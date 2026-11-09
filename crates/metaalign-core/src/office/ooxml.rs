//! OOXML (`docx`/`xlsx`/`pptx`) metadata reader and writer.
//!
//! An OOXML file is a ZIP container; the metadata lives in two small XML parts:
//! `docProps/core.xml` (Dublin Core) and `docProps/app.xml` (extended
//! properties). Everything else is copied through untouched.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

use super::OfficeUpdate;
use super::xmlpatch::{self, Replacement};
use crate::error::{MetaError, Result};

const CORE_PATH: &str = "docProps/core.xml";
const APP_PATH: &str = "docProps/app.xml";
const CONTENT_TYPES: &str = "[Content_Types].xml";
const ROOT_RELS: &str = "_rels/.rels";

const EMPTY_CORE: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n",
    "<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" ",
    "xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" ",
    "xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
    "</cp:coreProperties>"
);

const EMPTY_APP: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n",
    "<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" ",
    "xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"></Properties>"
);

/// One entry of the ZIP container.
struct Entry {
    name: String,
    compression: zip::CompressionMethod,
    data: Vec<u8>,
}

fn read_entries(bytes: &[u8]) -> Result<Vec<Entry>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let name = file
            .name()
            .map_err(|error| MetaError::Zip(error.to_string()))?
            .into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        entries.push(Entry {
            name,
            compression: file.compression(),
            data,
        });
    }
    Ok(entries)
}

fn write_entries(entries: &[Entry]) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for entry in entries {
        let options =
            zip::write::SimpleFileOptions::default().compression_method(entry.compression);
        writer.start_file(entry.name.as_str(), options)?;
        writer.write_all(&entry.data)?;
    }
    let cursor = writer.finish()?;
    Ok(cursor.into_inner())
}

fn parse_w3cdtf(raw: &str) -> Option<DateTime<Utc>> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Some(parsed.with_timezone(&Utc));
    }
    for format in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, format) {
            return Some(parsed.and_utc());
        }
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|naive| naive.and_utc())
}

fn format_w3cdtf(instant: DateTime<Utc>) -> String {
    instant.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn text_of(map: &HashMap<String, String>, key: &str) -> Option<String> {
    map.get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn props_from_parts(core: Option<&str>, app: Option<&str>) -> Result<super::OfficeProps> {
    let mut props = super::OfficeProps::default();
    if let Some(core) = core {
        let map: HashMap<String, String> = xmlpatch::collect_children(core)?.into_iter().collect();
        props.author = text_of(&map, "creator");
        props.last_modified_by = text_of(&map, "lastModifiedBy");
        props.title = text_of(&map, "title");
        props.subject = text_of(&map, "subject");
        props.keywords = text_of(&map, "keywords");
        props.category = text_of(&map, "category");
        props.comments = text_of(&map, "description");
        props.revision = text_of(&map, "revision");
        props.created = map.get("created").and_then(|value| parse_w3cdtf(value));
        props.modified = map.get("modified").and_then(|value| parse_w3cdtf(value));
    }
    if let Some(app) = app {
        let map: HashMap<String, String> = xmlpatch::collect_children(app)?.into_iter().collect();
        props.company = text_of(&map, "Company");
        props.manager = text_of(&map, "Manager");
        props.app_name = text_of(&map, "Application");
    }
    Ok(props)
}

/// Read metadata straight from the bytes of an OOXML container.
pub fn read_bytes(bytes: &[u8]) -> Result<super::OfficeProps> {
    let entries = read_entries(bytes)?;
    let core = entries
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(CORE_PATH))
        .map(|entry| String::from_utf8_lossy(&entry.data).into_owned());
    let app = entries
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(APP_PATH))
        .map(|entry| String::from_utf8_lossy(&entry.data).into_owned());
    props_from_parts(core.as_deref(), app.as_deref())
}

/// Read metadata from an OOXML file on disk.
pub fn read_file(path: &Path) -> Result<super::OfficeProps> {
    let bytes = std::fs::read(path)?;
    read_bytes(&bytes)
}

fn core_replacements(update: &OfficeUpdate) -> Vec<(Replacement, &'static str)> {
    let mut out = Vec::new();
    let mut push =
        |local: &'static str, prefix: &'static str, value: &Option<String>, label: &'static str| {
            if let Some(value) = value {
                out.push((
                    Replacement {
                        local,
                        prefix,
                        value: value.clone(),
                        w3cdtf: false,
                    },
                    label,
                ));
            }
        };
    push("creator", "dc", &update.author, "作者");
    push(
        "lastModifiedBy",
        "cp",
        &update.last_modified_by,
        "最后修改人",
    );
    push("title", "dc", &update.title, "标题");
    push("subject", "dc", &update.subject, "主题");
    push("keywords", "cp", &update.keywords, "关键词");
    push("category", "cp", &update.category, "分类");
    push("description", "dc", &update.comments, "备注");
    push("revision", "cp", &update.revision, "修订号");
    if let Some(created) = update.created {
        out.push((
            Replacement {
                local: "created",
                prefix: "dcterms",
                value: format_w3cdtf(created),
                w3cdtf: true,
            },
            "文档创建时间",
        ));
    }
    if let Some(modified) = update.modified {
        out.push((
            Replacement {
                local: "modified",
                prefix: "dcterms",
                value: format_w3cdtf(modified),
                w3cdtf: true,
            },
            "文档修改时间",
        ));
    }
    out
}

fn app_replacements(update: &OfficeUpdate) -> Vec<(Replacement, &'static str)> {
    let mut out = Vec::new();
    if let Some(company) = &update.company {
        out.push((
            Replacement {
                local: "Company",
                prefix: "",
                value: company.clone(),
                w3cdtf: false,
            },
            "公司",
        ));
    }
    if let Some(manager) = &update.manager {
        out.push((
            Replacement {
                local: "Manager",
                prefix: "",
                value: manager.clone(),
                w3cdtf: false,
            },
            "经理",
        ));
    }
    out
}

fn apply(xml: &str, replacements: &[(Replacement, &'static str)]) -> Result<(String, Vec<String>)> {
    let plain: Vec<Replacement> = replacements
        .iter()
        .map(|(replacement, _)| replacement.clone())
        .collect();
    let (output, applied) = xmlpatch::patch_children(xml, &plain)?;
    let labels = applied
        .iter()
        .filter_map(|local| {
            replacements
                .iter()
                .find(|(replacement, _)| replacement.local == local)
                .map(|(_, label)| label.to_string())
        })
        .collect();
    Ok((output, labels))
}

fn ensure_content_type(xml: &str, part: &str, content_type: &str) -> String {
    if xml.contains(&format!("PartName=\"/{part}\"")) {
        return xml.to_string();
    }
    let override_tag = format!("<Override PartName=\"/{part}\" ContentType=\"{content_type}\"/>");
    match xml.rfind("</Types>") {
        Some(index) => {
            let mut out = String::with_capacity(xml.len() + override_tag.len());
            out.push_str(&xml[..index]);
            out.push_str(&override_tag);
            out.push_str(&xml[index..]);
            out
        }
        None => xml.to_string(),
    }
}

fn ensure_root_relationship(xml: &str, part: &str, relationship_type: &str, id: &str) -> String {
    if xml.contains(&format!("Target=\"{part}\"")) {
        return xml.to_string();
    }
    let tag = format!("<Relationship Id=\"{id}\" Type=\"{relationship_type}\" Target=\"{part}\"/>");
    match xml.rfind("</Relationships>") {
        Some(index) => {
            let mut out = String::with_capacity(xml.len() + tag.len());
            out.push_str(&xml[..index]);
            out.push_str(&tag);
            out.push_str(&xml[index..]);
            out
        }
        None => xml.to_string(),
    }
}

/// Rewrite an OOXML container held in memory.
///
/// Returns the new bytes and the list of changed field labels.
pub fn write_bytes(bytes: &[u8], update: &OfficeUpdate) -> Result<(Vec<u8>, Vec<String>)> {
    let mut entries = read_entries(bytes)?;
    let mut labels: Vec<String> = Vec::new();

    let core_replacements = core_replacements(update);
    if !core_replacements.is_empty() {
        let index = entries
            .iter()
            .position(|entry| entry.name.eq_ignore_ascii_case(CORE_PATH));
        let existing = index
            .map(|index| String::from_utf8_lossy(&entries[index].data).into_owned())
            .unwrap_or_else(|| EMPTY_CORE.to_string());
        let (patched, changed) = apply(&existing, &core_replacements)?;
        labels.extend(changed);
        match index {
            Some(index) => entries[index].data = patched.into_bytes(),
            None => entries.push(Entry {
                name: CORE_PATH.to_string(),
                compression: zip::CompressionMethod::Deflated,
                data: patched.into_bytes(),
            }),
        }
    }

    let app_replacements = app_replacements(update);
    if !app_replacements.is_empty() {
        let index = entries
            .iter()
            .position(|entry| entry.name.eq_ignore_ascii_case(APP_PATH));
        let existing = index
            .map(|index| String::from_utf8_lossy(&entries[index].data).into_owned())
            .unwrap_or_else(|| EMPTY_APP.to_string());
        let (patched, changed) = apply(&existing, &app_replacements)?;
        labels.extend(changed);
        match index {
            Some(index) => entries[index].data = patched.into_bytes(),
            None => entries.push(Entry {
                name: APP_PATH.to_string(),
                compression: zip::CompressionMethod::Deflated,
                data: patched.into_bytes(),
            }),
        }
    }

    if labels.is_empty() {
        return Ok((bytes.to_vec(), labels));
    }

    // Make sure newly created parts are declared for the consumer application.
    if let Some(index) = entries
        .iter()
        .position(|entry| entry.name.eq_ignore_ascii_case(CONTENT_TYPES))
    {
        let mut xml = String::from_utf8_lossy(&entries[index].data).into_owned();
        xml = ensure_content_type(
            &xml,
            CORE_PATH,
            "application/vnd.openxmlformats-package.core-properties+xml",
        );
        xml = ensure_content_type(
            &xml,
            APP_PATH,
            "application/vnd.openxmlformats-officedocument.extended-properties+xml",
        );
        entries[index].data = xml.into_bytes();
    }

    if let Some(index) = entries
        .iter()
        .position(|entry| entry.name.eq_ignore_ascii_case(ROOT_RELS))
    {
        let mut xml = String::from_utf8_lossy(&entries[index].data).into_owned();
        xml = ensure_root_relationship(
            &xml,
            CORE_PATH,
            "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties",
            "rIdMetaAlignCore",
        );
        xml = ensure_root_relationship(
            &xml,
            APP_PATH,
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties",
            "rIdMetaAlignApp",
        );
        entries[index].data = xml.into_bytes();
    }

    let rebuilt = write_entries(&entries)?;
    Ok((rebuilt, labels))
}

/// Rewrite the metadata of an OOXML file in place.
pub fn write_file(path: &Path, update: &OfficeUpdate) -> Result<Vec<String>> {
    let bytes = std::fs::read(path)?;
    let (new_bytes, labels) = write_bytes(&bytes, update)?;
    if labels.is_empty() {
        return Ok(labels);
    }
    // Write to a sibling temp file first so a failure never leaves a half written document.
    let temp = path.with_extension(format!(
        "{}.metaalign.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("tmp")
    ));
    std::fs::write(&temp, &new_bytes)?;
    if let Err(error) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(MetaError::Io(error));
    }
    Ok(labels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_container(core: Option<&str>, app: Option<&str>) -> Vec<u8> {
        let mut entries = vec![Entry {
            name: CONTENT_TYPES.to_string(),
            compression: zip::CompressionMethod::Deflated,
            data: br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#.to_vec(),
        }];
        if let Some(core) = core {
            entries.push(Entry {
                name: CORE_PATH.to_string(),
                compression: zip::CompressionMethod::Deflated,
                data: core.as_bytes().to_vec(),
            });
        }
        if let Some(app) = app {
            entries.push(Entry {
                name: APP_PATH.to_string(),
                compression: zip::CompressionMethod::Deflated,
                data: app.as_bytes().to_vec(),
            });
        }
        write_entries(&entries).unwrap()
    }

    #[test]
    fn roundtrip_props() {
        let bytes = build_container(Some(EMPTY_CORE), Some(EMPTY_APP));
        let mut update = OfficeUpdate {
            author: Some("王小明".to_string()),
            last_modified_by: Some("李雷".to_string()),
            company: Some("MetaAlign 工作室".to_string()),
            revision: Some("12".to_string()),
            ..Default::default()
        };
        update.created = DateTime::from_timestamp(1_700_000_000, 0);
        update.modified = DateTime::from_timestamp(1_700_100_000, 0);

        let (new_bytes, labels) = write_bytes(&bytes, &update).unwrap();
        assert!(labels.contains(&"作者".to_string()));
        assert!(labels.contains(&"公司".to_string()));

        let props = read_bytes(&new_bytes).unwrap();
        assert_eq!(props.author.as_deref(), Some("王小明"));
        assert_eq!(props.last_modified_by.as_deref(), Some("李雷"));
        assert_eq!(props.company.as_deref(), Some("MetaAlign 工作室"));
        assert_eq!(props.revision.as_deref(), Some("12"));
        assert_eq!(props.created, update.created);
        assert_eq!(props.modified, update.modified);
    }

    #[test]
    fn creates_missing_core_part() {
        let bytes = build_container(None, None);
        let update = OfficeUpdate {
            author: Some("新作者".to_string()),
            ..Default::default()
        };
        let (new_bytes, labels) = write_bytes(&bytes, &update).unwrap();
        assert!(labels.contains(&"作者".to_string()));
        let props = read_bytes(&new_bytes).unwrap();
        assert_eq!(props.author.as_deref(), Some("新作者"));
    }
}
