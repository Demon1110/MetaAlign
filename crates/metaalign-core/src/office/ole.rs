//! Legacy Office (`.doc`/`.xls`/`.ppt`) metadata through OLE property sets.
//!
//! These formats store their document properties in the OLE compound file
//! streams `\x05SummaryInformation` (author, title, timestamps …) and
//! `\x05DocumentSummaryInformation` (company, manager, category …). Each stream
//! is a "property set" made of sections, each section a list of typed values.
//! MetaAlign parses the structures, swaps the values it was asked to change and
//! writes a structurally identical stream back, so every untouched property is
//! preserved byte for byte.

use std::path::Path;

use chrono::{DateTime, Utc};
use std::io::{Read, Write};

use super::{OfficeProps, OfficeUpdate};
use crate::error::{MetaError, Result};
use crate::fsmeta::{filetime_ticks_to_unix, unix_to_filetime_ticks};

const SI_STREAM: &str = "\u{5}SummaryInformation";
const DSI_STREAM: &str = "\u{5}DocumentSummaryInformation";

/// FMTID of the SummaryInformation property set.
const FMTID_SUMMARY: [u8; 16] = [
    0xE0, 0x85, 0x9F, 0xF2, 0x9B, 0x4F, 0x68, 0x10, 0xAB, 0x91, 0x08, 0x00, 0x2B, 0x27, 0xB3, 0xD9,
];
/// FMTID of the DocumentSummaryInformation property set.
const FMTID_DOCUMENT: [u8; 16] = [
    0x02, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C, 0xF9, 0xAE,
];

const VT_I2: u32 = 2;
const VT_LPSTR: u32 = 30;
const VT_LPWSTR: u32 = 31;
const VT_FILETIME: u32 = 64;

#[derive(Clone, Debug)]
struct Property {
    id: u32,
    /// The complete value: 4 byte type tag followed by the payload (4-byte padded).
    raw: Vec<u8>,
}

impl Property {
    fn type_id(&self) -> u32 {
        read_u32(&self.raw, 0).unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
struct Section {
    fmtid: [u8; 16],
    props: Vec<Property>,
    codepage: u16,
}

#[derive(Clone, Debug)]
struct PropSet {
    header: Vec<u8>,
    sections: Vec<Section>,
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn pad4(bytes: &mut Vec<u8>) {
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
}

fn parse_propset(bytes: &[u8]) -> Result<PropSet> {
    if bytes.len() < 28 {
        return Err(MetaError::Parse("属性集长度不足".to_string()));
    }
    if read_u16(bytes, 0) != Some(0xFFFE) {
        return Err(MetaError::Parse("属性集字节序标记无效".to_string()));
    }
    let count = read_u32(bytes, 24).unwrap_or(0) as usize;
    if count == 0 || count > 64 {
        return Err(MetaError::Parse(format!("属性集数量异常：{count}")));
    }

    let mut sections = Vec::with_capacity(count);
    for index in 0..count {
        let entry = 28 + index * 20;
        let Some(offset) = read_u32(bytes, entry + 16) else {
            break;
        };
        let mut fmtid = [0u8; 16];
        if let Some(guid) = bytes.get(entry..entry + 16) {
            fmtid.copy_from_slice(guid);
        }
        sections.push(parse_section(bytes, offset as usize, fmtid)?);
    }

    if sections.is_empty() {
        return Err(MetaError::Parse("属性集中没有有效分区".to_string()));
    }

    Ok(PropSet {
        header: bytes[..28].to_vec(),
        sections,
    })
}

fn parse_section(bytes: &[u8], offset: usize, fmtid: [u8; 16]) -> Result<Section> {
    let size = read_u32(bytes, offset).unwrap_or(0) as usize;
    let count = read_u32(bytes, offset + 4).unwrap_or(0) as usize;
    if count > 4096 {
        return Err(MetaError::Parse(format!("属性数量异常：{count}")));
    }
    let end = (offset + size).min(bytes.len());

    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let base = offset + 8 + index * 8;
        let Some(id) = read_u32(bytes, base) else {
            break;
        };
        let Some(relative) = read_u32(bytes, base + 4) else {
            break;
        };
        entries.push((id, relative as usize));
    }
    entries.sort_by_key(|(_, relative)| *relative);

    let mut props = Vec::with_capacity(entries.len());
    for (position, (id, relative)) in entries.iter().enumerate() {
        let start = offset + relative;
        let stop = entries
            .get(position + 1)
            .map(|(_, next)| offset + next)
            .unwrap_or(end)
            .min(bytes.len());
        if start >= stop {
            continue;
        }
        let raw = bytes[start..stop].to_vec();
        if raw.len() < 4 {
            continue;
        }
        props.push(Property { id: *id, raw });
    }

    let codepage = props
        .iter()
        .find(|prop| prop.id == 1 && prop.type_id() == VT_I2)
        .and_then(|prop| read_u16(&prop.raw, 4))
        .unwrap_or(1252);

    Ok(Section {
        fmtid,
        props,
        codepage,
    })
}

fn decode_ansi(bytes: &[u8], codepage: u16) -> String {
    match codepage {
        936 | 54936 => encoding_rs::GBK.decode(bytes).0.into_owned(),
        950 => encoding_rs::BIG5.decode(bytes).0.into_owned(),
        65001 => String::from_utf8_lossy(bytes).into_owned(),
        _ => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

fn encode_ansi(value: &str, codepage: u16) -> Vec<u8> {
    match codepage {
        936 | 54936 => encoding_rs::GBK.encode(value).0.into_owned(),
        950 => encoding_rs::BIG5.encode(value).0.into_owned(),
        65001 => value.as_bytes().to_vec(),
        _ => encoding_rs::WINDOWS_1252.encode(value).0.into_owned(),
    }
}

fn prop_string(prop: &Property, codepage: u16) -> Option<String> {
    match prop.type_id() {
        VT_LPSTR => {
            let length = read_u32(&prop.raw, 4)? as usize;
            let payload = prop.raw.get(8..).unwrap_or_default();
            let end = length.min(payload.len());
            let slice = &payload[..end];
            let trimmed: Vec<u8> = slice
                .iter()
                .copied()
                .take_while(|byte| *byte != 0)
                .collect();
            Some(decode_ansi(&trimmed, codepage))
        }
        VT_LPWSTR => {
            let length = read_u32(&prop.raw, 4)? as usize;
            let payload = prop.raw.get(8..).unwrap_or_default();
            let bytes = &payload[..(length * 2).min(payload.len())];
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .take_while(|unit| *unit != 0)
                .collect();
            Some(String::from_utf16_lossy(&units))
        }
        _ => None,
    }
}

fn prop_time(prop: &Property) -> Option<DateTime<Utc>> {
    if prop.type_id() != VT_FILETIME {
        return None;
    }
    let slice = prop.raw.get(4..12)?;
    let ticks = u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]);
    filetime_ticks_to_unix(ticks)
}

fn make_string(value: &str, codepage: u16) -> Vec<u8> {
    let mut payload = encode_ansi(value, codepage);
    payload.push(0);
    let mut raw = Vec::with_capacity(payload.len() + 8);
    raw.extend_from_slice(&VT_LPSTR.to_le_bytes());
    raw.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    raw.extend_from_slice(&payload);
    pad4(&mut raw);
    raw
}

fn make_time(value: DateTime<Utc>) -> Vec<u8> {
    let mut raw = Vec::with_capacity(12);
    raw.extend_from_slice(&VT_FILETIME.to_le_bytes());
    raw.extend_from_slice(&unix_to_filetime_ticks(value).to_le_bytes());
    pad4(&mut raw);
    raw
}

fn make_i2(value: i16) -> Vec<u8> {
    let mut raw = Vec::with_capacity(8);
    raw.extend_from_slice(&VT_I2.to_le_bytes());
    raw.extend_from_slice(&value.to_le_bytes());
    pad4(&mut raw);
    raw
}

fn serialize_section(section: &Section) -> Vec<u8> {
    let count = section.props.len();
    let header_size = 8 + count * 8;
    let mut values: Vec<u8> = Vec::new();
    let mut offsets = Vec::with_capacity(count);

    for prop in &section.props {
        offsets.push((header_size + values.len()) as u32);
        values.extend_from_slice(&prop.raw);
        pad4(&mut values);
    }

    let size = (header_size + values.len()) as u32;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&(count as u32).to_le_bytes());
    for (prop, offset) in section.props.iter().zip(offsets) {
        out.extend_from_slice(&prop.id.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
    }
    out.extend_from_slice(&values);
    out
}

fn build_propset(set: &PropSet, section_bytes: &[Vec<u8>]) -> Vec<u8> {
    let count = section_bytes.len();
    let mut header = set.header.clone();
    header.resize(28, 0);
    header[24..28].copy_from_slice(&(count as u32).to_le_bytes());

    let mut out = Vec::with_capacity(
        header.len() + count * 20 + section_bytes.iter().map(Vec::len).sum::<usize>(),
    );
    out.extend_from_slice(&header);
    let mut offset = 28 + count * 20;
    for (section, bytes) in set.sections.iter().zip(section_bytes) {
        out.extend_from_slice(&section.fmtid);
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += bytes.len();
    }
    for bytes in section_bytes {
        out.extend_from_slice(bytes);
    }
    out
}

fn new_propset(fmtid: [u8; 16], codepage: u16, mut values: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    values.sort_by_key(|(id, _)| *id);
    if !values.iter().any(|(id, _)| *id == 1) {
        values.insert(0, (1, make_i2(codepage as i16)));
    }
    let section = Section {
        fmtid,
        props: values
            .into_iter()
            .map(|(id, raw)| Property { id, raw })
            .collect(),
        codepage,
    };
    let set = PropSet {
        header: {
            let mut header = vec![0u8; 28];
            header[0..2].copy_from_slice(&0xFFFEu16.to_le_bytes());
            header[2..4].copy_from_slice(&2u16.to_le_bytes());
            header[24..28].copy_from_slice(&1u32.to_le_bytes());
            header
        },
        sections: vec![section.clone()],
    };
    build_propset(&set, &[serialize_section(&section)])
}

fn read_stream(path: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    let mut file = match cfb::open(path) {
        Ok(file) => file,
        Err(error) => return Err(MetaError::Io(error)),
    };
    if !file.exists(name) {
        return Ok(None);
    }
    let mut stream = file.open_stream(name)?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

fn write_stream(path: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let mut file = cfb::open_rw(path)?;
    {
        let mut stream = file.create_stream(name)?;
        stream.write_all(bytes)?;
    }
    file.flush()?;
    Ok(())
}

fn apply_values(
    set: &PropSet,
    section_index: usize,
    updates: &[(u32, Vec<u8>)],
) -> (Vec<u8>, Vec<u32>) {
    let mut sections = set.sections.clone();
    let mut applied = Vec::new();
    if let Some(section) = sections.get_mut(section_index) {
        for (id, raw) in updates {
            match section.props.iter_mut().find(|prop| prop.id == *id) {
                Some(prop) => prop.raw = raw.clone(),
                None => section.props.push(Property {
                    id: *id,
                    raw: raw.clone(),
                }),
            }
            applied.push(*id);
        }
        section.props.sort_by_key(|prop| prop.id);
    }
    let bytes: Vec<Vec<u8>> = sections.iter().map(serialize_section).collect();
    let mut rebuilt = set.clone();
    rebuilt.sections = sections;
    (build_propset(&rebuilt, &bytes), applied)
}

fn summary_updates(update: &OfficeUpdate, codepage: u16) -> Vec<(u32, Vec<u8>)> {
    let mut values: Vec<(u32, Vec<u8>)> = Vec::new();
    if let Some(value) = &update.title {
        values.push((2, make_string(value, codepage)));
    }
    if let Some(value) = &update.subject {
        values.push((3, make_string(value, codepage)));
    }
    if let Some(value) = &update.author {
        values.push((4, make_string(value, codepage)));
    }
    if let Some(value) = &update.keywords {
        values.push((5, make_string(value, codepage)));
    }
    if let Some(value) = &update.comments {
        values.push((6, make_string(value, codepage)));
    }
    if let Some(value) = &update.last_modified_by {
        values.push((8, make_string(value, codepage)));
    }
    if let Some(value) = &update.revision {
        values.push((9, make_string(value, codepage)));
    }
    if let Some(value) = update.created {
        values.push((12, make_time(value)));
    }
    if let Some(value) = update.modified {
        values.push((13, make_time(value)));
    }
    values
}

fn document_updates(update: &OfficeUpdate, codepage: u16) -> Vec<(u32, Vec<u8>)> {
    let mut values: Vec<(u32, Vec<u8>)> = Vec::new();
    if let Some(value) = &update.category {
        values.push((2, make_string(value, codepage)));
    }
    if let Some(value) = &update.manager {
        values.push((14, make_string(value, codepage)));
    }
    if let Some(value) = &update.company {
        values.push((15, make_string(value, codepage)));
    }
    values
}

/// Read the metadata of a legacy Office file.
pub fn read_file(path: &Path) -> Result<OfficeProps> {
    let mut props = OfficeProps::default();

    if let Some(bytes) = read_stream(path, SI_STREAM)? {
        let set = parse_propset(&bytes)?;
        if let Some(section) = set.sections.first() {
            for prop in &section.props {
                match prop.id {
                    1 => {}
                    2 => props.title = prop_string(prop, section.codepage),
                    3 => props.subject = prop_string(prop, section.codepage),
                    4 => props.author = prop_string(prop, section.codepage),
                    5 => props.keywords = prop_string(prop, section.codepage),
                    6 => props.comments = prop_string(prop, section.codepage),
                    8 => props.last_modified_by = prop_string(prop, section.codepage),
                    9 => props.revision = prop_string(prop, section.codepage),
                    12 => props.created = prop_time(prop),
                    13 => props.modified = prop_time(prop),
                    18 => props.app_name = prop_string(prop, section.codepage),
                    _ => {}
                }
            }
        }
    }

    if let Some(bytes) = read_stream(path, DSI_STREAM)? {
        let set = parse_propset(&bytes)?;
        if let Some(section) = set.sections.first() {
            for prop in &section.props {
                match prop.id {
                    2 => props.category = prop_string(prop, section.codepage),
                    14 => props.manager = prop_string(prop, section.codepage),
                    15 => props.company = prop_string(prop, section.codepage),
                    _ => {}
                }
            }
        }
    }

    Ok(props)
}

/// Rewrite the metadata of a legacy Office file.
pub fn write_file(path: &Path, update: &OfficeUpdate) -> Result<Vec<String>> {
    let mut labels: Vec<String> = Vec::new();

    let summary_values = summary_updates(update, 936);
    if !summary_values.is_empty() {
        match read_stream(path, SI_STREAM)? {
            Some(bytes) => {
                let set = parse_propset(&bytes)?;
                let codepage = set.sections.first().map(|s| s.codepage).unwrap_or(1252);
                let values = summary_updates(update, codepage);
                let (rebuilt, applied) = apply_values(&set, 0, &values);
                write_stream(path, SI_STREAM, &rebuilt)?;
                labels.extend(summary_labels(&applied));
            }
            None => {
                let values = summary_updates(update, 936);
                let rebuilt = new_propset(FMTID_SUMMARY, 936, values.clone());
                write_stream(path, SI_STREAM, &rebuilt)?;
                labels.extend(summary_labels(
                    &values.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                ));
            }
        }
    }

    let document_values = document_updates(update, 936);
    if !document_values.is_empty() {
        match read_stream(path, DSI_STREAM)? {
            Some(bytes) => {
                let set = parse_propset(&bytes)?;
                let codepage = set.sections.first().map(|s| s.codepage).unwrap_or(1252);
                let values = document_updates(update, codepage);
                let (rebuilt, applied) = apply_values(&set, 0, &values);
                write_stream(path, DSI_STREAM, &rebuilt)?;
                labels.extend(document_labels(&applied));
            }
            None => {
                let values = document_updates(update, 936);
                let rebuilt = new_propset(FMTID_DOCUMENT, 936, values.clone());
                write_stream(path, DSI_STREAM, &rebuilt)?;
                labels.extend(document_labels(
                    &values.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                ));
            }
        }
    }

    Ok(labels)
}

fn summary_labels(ids: &[u32]) -> Vec<String> {
    ids.iter()
        .filter_map(|id| {
            Some(
                match id {
                    2 => "标题",
                    3 => "主题",
                    4 => "作者",
                    5 => "关键词",
                    6 => "备注",
                    8 => "最后修改人",
                    9 => "修订号",
                    12 => "文档创建时间",
                    13 => "文档修改时间",
                    _ => return None,
                }
                .to_string(),
            )
        })
        .collect()
}

fn document_labels(ids: &[u32]) -> Vec<String> {
    ids.iter()
        .filter_map(|id| {
            Some(
                match id {
                    2 => "分类",
                    14 => "经理",
                    15 => "公司",
                    _ => return None,
                }
                .to_string(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_set() -> PropSet {
        let values = vec![
            (1u32, make_i2(936)),
            (2, make_string("旧标题", 936)),
            (4, make_string("旧作者", 936)),
            (
                12,
                make_time(DateTime::from_timestamp(1_500_000_000, 0).unwrap()),
            ),
        ];
        let bytes = new_propset(FMTID_SUMMARY, 936, values);
        parse_propset(&bytes).unwrap()
    }

    #[test]
    fn propset_roundtrip_preserves_values() {
        let set = sample_set();
        let bytes = build_propset(
            &set,
            &set.sections
                .iter()
                .map(serialize_section)
                .collect::<Vec<_>>(),
        );
        let parsed = parse_propset(&bytes).unwrap();
        let section = &parsed.sections[0];
        assert_eq!(
            section
                .props
                .iter()
                .find(|prop| prop.id == 2)
                .and_then(|prop| prop_string(prop, section.codepage)),
            Some("旧标题".to_string())
        );
    }

    #[test]
    fn updates_replace_and_append() {
        let set = sample_set();
        let updates = vec![
            (4u32, make_string("新作者", 936)),
            (8, make_string("审核员", 936)),
        ];
        let (bytes, applied) = apply_values(&set, 0, &updates);
        assert_eq!(applied, vec![4, 8]);
        let parsed = parse_propset(&bytes).unwrap();
        let section = &parsed.sections[0];
        assert_eq!(
            section
                .props
                .iter()
                .find(|prop| prop.id == 4)
                .and_then(|prop| prop_string(prop, section.codepage)),
            Some("新作者".to_string())
        );
        assert_eq!(
            section
                .props
                .iter()
                .find(|prop| prop.id == 8)
                .and_then(|prop| prop_string(prop, section.codepage)),
            Some("审核员".to_string())
        );
    }

    #[test]
    fn filenames_do_not_break_the_date_math() {
        let ticks = unix_to_filetime_ticks(DateTime::from_timestamp(1_700_000_000, 0).unwrap());
        assert_eq!(
            filetime_ticks_to_unix(ticks).map(|value| value.timestamp()),
            Some(1_700_000_000)
        );
    }
}
