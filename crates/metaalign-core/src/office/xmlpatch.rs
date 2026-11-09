//! Minimal, structure preserving XML patching for OOXML `docProps` parts.
//!
//! Instead of re-serialising a whole document we locate the direct children of
//! the root element, replace only the text of the fields we care about and copy
//! everything else verbatim. Unknown namespaces, attributes and formatting are
//! therefore preserved byte for byte.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{MetaError, Result};

/// Namespace declarations MetaAlign may have to add when a field is missing.
pub const NAMESPACES: &[(&str, &str)] = &[
    (
        "cp",
        "http://schemas.openxmlformats.org/package/2006/metadata/core-properties",
    ),
    ("dc", "http://purl.org/dc/elements/1.1/"),
    ("dcterms", "http://purl.org/dc/terms/"),
    ("dcmitype", "http://purl.org/dc/dcmitype/"),
    ("xsi", "http://www.w3.org/2001/XMLSchema-instance"),
];

/// One field to write, addressed by its local element name.
#[derive(Clone, Debug)]
pub struct Replacement {
    pub local: &'static str,
    pub prefix: &'static str,
    pub value: String,
    /// Add `xsi:type="dcterms:W3CDTF"` when the element has no type attribute.
    pub w3cdtf: bool,
}

#[derive(Debug)]
struct ChildSpan {
    local: String,
    raw_name: String,
    start: usize,
    end: usize,
    is_empty: bool,
}

#[derive(Debug)]
struct Document {
    root_start_tag: String,
    root_start_end: usize,
    root_end_start: usize,
    children: Vec<ChildSpan>,
}

fn parse_document(xml: &str) -> Result<Document> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut last_pos = 0usize;
    let mut root_start_tag = String::new();
    let mut root_start_end = 0usize;
    let mut root_end_start = xml.len();
    let mut children: Vec<ChildSpan> = Vec::new();
    let mut pending: Option<(String, String, usize, bool)> = None;

    loop {
        let start = last_pos;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let local = event.local_name().as_ref().to_string();
                let raw = event.name().as_ref().to_string();
                depth += 1;
                if depth == 1 {
                    root_start_tag = xml[start..reader.buffer_position() as usize].to_string();
                    root_start_end = reader.buffer_position() as usize;
                } else if depth == 2 {
                    pending = Some((local, raw, start, false));
                }
            }
            Ok(Event::Empty(event)) => {
                if depth == 1 {
                    children.push(ChildSpan {
                        local: event.local_name().as_ref().to_string(),
                        raw_name: event.name().as_ref().to_string(),
                        start,
                        end: reader.buffer_position() as usize,
                        is_empty: true,
                    });
                }
            }
            Ok(Event::End(_)) => {
                if depth == 2
                    && let Some((local, raw_name, child_start, _)) = pending.take()
                {
                    children.push(ChildSpan {
                        local,
                        raw_name,
                        start: child_start,
                        end: reader.buffer_position() as usize,
                        is_empty: false,
                    });
                }
                if depth == 1 {
                    root_end_start = start;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(MetaError::Parse(format!("XML 结构解析失败：{error}")));
            }
        }
        last_pos = reader.buffer_position() as usize;
    }

    if root_start_end == 0 || root_end_start >= xml.len() {
        return Err(MetaError::Parse("缺少根元素".to_string()));
    }

    Ok(Document {
        root_start_tag,
        root_start_end,
        root_end_start,
        children,
    })
}

fn ensure_namespaces(root_tag: &str, prefixes: &[&str]) -> String {
    let mut additions = String::new();
    for prefix in prefixes {
        let declared = format!("xmlns:{prefix}=");
        if root_tag.contains(&declared) {
            continue;
        }
        if let Some((_, uri)) = NAMESPACES.iter().find(|(name, _)| name == prefix) {
            additions.push_str(&format!(" xmlns:{prefix}=\"{uri}\""));
        }
    }
    if additions.is_empty() {
        return root_tag.to_string();
    }
    match root_tag.rfind('>') {
        Some(index) => {
            let mut out = String::with_capacity(root_tag.len() + additions.len());
            out.push_str(&root_tag[..index]);
            out.push_str(&additions);
            out.push_str(&root_tag[index..]);
            out
        }
        None => root_tag.to_string(),
    }
}

fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
        .collect()
}

fn rebuilt_child(raw: &str, child: &ChildSpan, replacement: &Replacement) -> String {
    let sanitized = sanitize(&replacement.value);
    let text = quick_xml::escape::escape(&sanitized);
    if child.is_empty {
        let trimmed = raw.trim_end();
        let open = trimmed
            .strip_suffix("/>")
            .map(|head| head.trim_end().to_string())
            .unwrap_or_else(|| trimmed.to_string());
        return format!("{open}>{text}</{}>", child.raw_name);
    }

    let start_tag_end = raw.find('>').map(|index| index + 1).unwrap_or(raw.len());
    let mut open_tag = raw[..start_tag_end].to_string();
    if replacement.w3cdtf && !open_tag.contains("xsi:type") {
        if let Some(index) = open_tag.rfind('>') {
            open_tag.insert_str(index, " xsi:type=\"dcterms:W3CDTF\"");
        }
    }
    format!("{open_tag}{text}</{}>", child.raw_name)
}

fn new_element(replacement: &Replacement) -> String {
    let sanitized = sanitize(&replacement.value);
    let text = quick_xml::escape::escape(&sanitized);
    let name = if replacement.prefix.is_empty() {
        replacement.local.to_string()
    } else {
        format!("{}:{}", replacement.prefix, replacement.local)
    };
    if replacement.w3cdtf {
        format!("<{name} xsi:type=\"dcterms:W3CDTF\">{text}</{name}>")
    } else {
        format!("<{name}>{text}</{name}>")
    }
}

/// Apply the replacements to the direct children of the root element.
///
/// Returns the new document plus the list of local names that were written.
pub fn patch_children(xml: &str, replacements: &[Replacement]) -> Result<(String, Vec<String>)> {
    let document = parse_document(xml)?;
    let prefixes: Vec<&str> = replacements
        .iter()
        .map(|replacement| replacement.prefix)
        .collect();
    let root_tag = ensure_namespaces(&document.root_start_tag, &prefixes);

    let mut out = String::with_capacity(xml.len() + 256);
    out.push_str(&root_tag);
    let mut cursor = document.root_start_end;
    let mut applied: Vec<String> = Vec::new();

    for child in &document.children {
        if cursor < child.start {
            out.push_str(&xml[cursor..child.start]);
        }
        match replacements
            .iter()
            .find(|replacement| replacement.local == child.local)
        {
            Some(replacement) => {
                out.push_str(&rebuilt_child(&xml[child.start..child.end], child, replacement));
                applied.push(child.local.clone());
            }
            None => out.push_str(&xml[child.start..child.end]),
        }
        cursor = child.end;
    }

    let mut missing = String::new();
    for replacement in replacements {
        if !applied.iter().any(|name| name == replacement.local) {
            missing.push_str(&new_element(replacement));
            applied.push(replacement.local.to_string());
        }
    }

    if cursor < document.root_end_start {
        out.push_str(&xml[cursor..document.root_end_start]);
    }
    out.push_str(&missing);
    out.push_str(&xml[document.root_end_start..]);
    Ok((out, applied))
}

/// Collect the text of every direct child of the root element.
pub fn collect_children(xml: &str) -> Result<Vec<(String, String)>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut current: Option<String> = None;
    let mut text = String::new();
    let mut out = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                depth += 1;
                if depth == 2 {
                    current =
                        Some(event.local_name().as_ref().to_string());
                    text.clear();
                }
            }
            Ok(Event::Empty(event)) => {
                if depth == 1 {
                    out.push((
                        event.local_name().as_ref().to_string(),
                        String::new(),
                    ));
                }
            }
            Ok(Event::Text(event)) => {
                if depth == 2 && current.is_some() {
                    text.push_str(event.as_ref());
                }
            }
            Ok(Event::CData(event)) => {
                if depth == 2 && current.is_some() {
                    text.push_str(event.as_ref());
                }
            }
            Ok(Event::End(_)) => {
                if depth == 2 && let Some(name) = current.take() {
                    let value = quick_xml::escape::unescape(&text)
                        .map(|value| value.into_owned())
                        .unwrap_or(text.clone());
                    out.push((name, value));
                    text.clear();
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(MetaError::Parse(format!("XML 结构解析失败：{error}")));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:creator>旧作者</dc:creator>
  <cp:lastModifiedBy>旧作者</cp:lastModifiedBy>
  <cp:revision>3</cp:revision>
  <dcterms:created xsi:type="dcterms:W3CDTF">2020-01-01T00:00:00Z</dcterms:created>
  <dcterms:modified xsi:type="dcterms:W3CDTF">2020-01-02T00:00:00Z</dcterms:modified>
</cp:coreProperties>"#;

    #[test]
    fn replaces_existing_and_adds_missing() {
        let replacements = vec![
            Replacement {
                local: "creator",
                prefix: "dc",
                value: "新的作者".to_string(),
                w3cdtf: false,
            },
            Replacement {
                local: "keywords",
                prefix: "cp",
                value: "标签".to_string(),
                w3cdtf: false,
            },
        ];
        let (output, applied) = patch_children(CORE, &replacements).unwrap();
        assert!(output.contains("<dc:creator>新的作者</dc:creator>"));
        assert!(output.contains("<cp:keywords>标签</cp:keywords>"));
        assert!(output.contains("<cp:revision>3</cp:revision>"));
        assert_eq!(applied.len(), 2);

        let children = collect_children(&output).unwrap();
        assert_eq!(
            children
                .iter()
                .find(|(name, _)| name == "creator")
                .map(|(_, value)| value.as_str()),
            Some("新的作者")
        );
    }

    #[test]
    fn writes_w3cdtf_with_type() {
        let replacements = vec![Replacement {
            local: "created",
            prefix: "dcterms",
            value: "2024-05-06T07:08:09Z".to_string(),
            w3cdtf: true,
        }];
        let (output, _) = patch_children(CORE, &replacements).unwrap();
        assert!(output.contains("2024-05-06T07:08:09Z"));
        assert!(output.contains("xsi:type=\"dcterms:W3CDTF\""));
    }

    #[test]
    fn self_closing_fields_are_expanded() {
        let xml = r#"<cp:coreProperties xmlns:cp="c" xmlns:dc="d"><dc:title/></cp:coreProperties>"#;
        let replacements = vec![Replacement {
            local: "title",
            prefix: "dc",
            value: "新标题".to_string(),
            w3cdtf: false,
        }];
        let (output, _) = patch_children(xml, &replacements).unwrap();
        assert!(output.contains("<dc:title>新标题</dc:title>"), "{output}");
    }
}
