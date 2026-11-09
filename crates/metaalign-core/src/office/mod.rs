//! Office document metadata (OOXML `docProps` + legacy OLE property sets).

pub mod ole;
pub mod ooxml;
pub mod xmlpatch;

use std::io::Read;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{MetaError, Result};

/// The document formats MetaAlign recognises.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfficeFormat {
    #[default]
    Other,
    /// `.docx` and friends (Word, OOXML).
    Docx,
    /// `.xlsx` and friends (Excel, OOXML).
    Xlsx,
    /// `.pptx` and friends (PowerPoint, OOXML).
    Pptx,
    /// OOXML container with an unusual extension.
    Ooxml,
    /// Legacy Word `.doc`.
    Doc,
    /// Legacy Excel `.xls`.
    Xls,
    /// Legacy PowerPoint `.ppt`.
    Ppt,
    /// OLE container with an unusual extension.
    Ole,
    /// PDF is recognised but not yet rewritten.
    Pdf,
}

impl OfficeFormat {
    pub fn from_extension(extension: &str) -> Self {
        match extension.to_ascii_lowercase().as_str() {
            "docx" | "docm" | "dotx" | "dotm" => Self::Docx,
            "xlsx" | "xlsm" | "xltx" | "xltm" | "xlsb" => Self::Xlsx,
            "pptx" | "pptm" | "potx" | "potm" | "ppsx" => Self::Pptx,
            "doc" | "dot" | "wps" => Self::Doc,
            "xls" | "xlt" | "et" => Self::Xls,
            "ppt" | "pps" | "dps" => Self::Ppt,
            "pdf" => Self::Pdf,
            _ => Self::Other,
        }
    }

    pub fn is_ooxml(self) -> bool {
        matches!(self, Self::Docx | Self::Xlsx | Self::Pptx | Self::Ooxml)
    }

    pub fn is_ole(self) -> bool {
        matches!(self, Self::Doc | Self::Xls | Self::Ppt | Self::Ole)
    }

    /// Files whose internal metadata can be read and rewritten.
    pub fn supports_office_meta(self) -> bool {
        self.is_ooxml() || self.is_ole()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Docx => "Word 文档",
            Self::Xlsx => "Excel 工作簿",
            Self::Pptx => "PowerPoint 演示",
            Self::Ooxml => "OOXML 文档",
            Self::Doc => "Word 97-2003",
            Self::Xls => "Excel 97-2003",
            Self::Ppt => "PowerPoint 97-2003",
            Self::Ole => "OLE 复合文档",
            Self::Pdf => "PDF 文档",
            Self::Other => "普通文件",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Docx | Self::Ooxml => "DOCX",
            Self::Xlsx => "XLSX",
            Self::Pptx => "PPTX",
            Self::Doc => "DOC",
            Self::Xls => "XLS",
            Self::Ppt => "PPT",
            Self::Ole => "OLE",
            Self::Pdf => "PDF",
            Self::Other => "FILE",
        }
    }
}

/// Extensions offered by the folder scanner by default.
pub fn supported_extensions() -> &'static [&'static str] {
    &[
        "docx", "docm", "dotx", "xlsx", "xlsm", "xltx", "pptx", "pptm", "potx", "doc", "xls",
        "ppt", "wps", "et", "dps",
    ]
}

/// Metadata read from a document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OfficeProps {
    pub title: Option<String>,
    pub subject: Option<String>,
    pub author: Option<String>,
    pub last_modified_by: Option<String>,
    pub keywords: Option<String>,
    pub category: Option<String>,
    pub comments: Option<String>,
    pub company: Option<String>,
    pub manager: Option<String>,
    pub revision: Option<String>,
    pub app_name: Option<String>,
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
}

impl OfficeProps {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.subject.is_none()
            && self.author.is_none()
            && self.last_modified_by.is_none()
            && self.keywords.is_none()
            && self.category.is_none()
            && self.comments.is_none()
            && self.company.is_none()
            && self.manager.is_none()
            && self.revision.is_none()
            && self.created.is_none()
            && self.modified.is_none()
    }
}

/// Fields to write into a document; `None` means "keep".
#[derive(Clone, Debug, Default)]
pub struct OfficeUpdate {
    pub author: Option<String>,
    pub last_modified_by: Option<String>,
    pub title: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub category: Option<String>,
    pub comments: Option<String>,
    pub company: Option<String>,
    pub manager: Option<String>,
    pub revision: Option<String>,
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
}

impl OfficeUpdate {
    pub fn is_empty(&self) -> bool {
        self.field_count() == 0
    }

    pub fn field_count(&self) -> usize {
        [
            self.author.is_some(),
            self.last_modified_by.is_some(),
            self.title.is_some(),
            self.subject.is_some(),
            self.keywords.is_some(),
            self.category.is_some(),
            self.comments.is_some(),
            self.company.is_some(),
            self.manager.is_some(),
            self.revision.is_some(),
            self.created.is_some(),
            self.modified.is_some(),
        ]
        .iter()
        .filter(|present| **present)
        .count()
    }
}

/// Detect a format from the extension first and the magic bytes second.
pub fn detect_format(path: &Path) -> OfficeFormat {
    let by_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(OfficeFormat::from_extension)
        .unwrap_or_default();
    if by_extension != OfficeFormat::Other {
        return by_extension;
    }
    sniff_magic(path).unwrap_or(OfficeFormat::Other)
}

/// Inspect the first bytes of a file to tell OOXML from OLE.
pub fn sniff_magic(path: &Path) -> Option<OfficeFormat> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut magic = [0u8; 8];
    let read = file.read(&mut magic).ok()?;
    if read < 4 {
        return None;
    }
    if magic[..4] == [0x50, 0x4B, 0x03, 0x04] {
        return Some(OfficeFormat::Ooxml);
    }
    if read >= 8 && magic == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1] {
        return Some(OfficeFormat::Ole);
    }
    if magic[..4] == *b"%PDF" {
        return Some(OfficeFormat::Pdf);
    }
    None
}

/// Read the metadata of `path`, when the format is supported.
pub fn read_props(path: &Path) -> Result<Option<OfficeProps>> {
    let format = detect_format(path);
    if format.is_ooxml() {
        return ooxml::read_file(path).map(Some);
    }
    if format.is_ole() {
        return ole::read_file(path).map(Some);
    }
    Ok(None)
}

/// Rewrite the metadata of `path`; returns the list of changed fields.
pub fn update_props(path: &Path, update: &OfficeUpdate) -> Result<Vec<String>> {
    if update.is_empty() {
        return Ok(Vec::new());
    }
    let format = detect_format(path);
    if format.is_ooxml() {
        return ooxml::write_file(path, update);
    }
    if format.is_ole() {
        return ole::write_file(path, update);
    }
    if format == OfficeFormat::Pdf {
        return Err(MetaError::Unsupported(
            "PDF 元数据写入尚未实现（可通过 PDF 工具链后续扩展）".to_string(),
        ));
    }
    Err(MetaError::Unsupported(format!(
        "{} 不支持内部元数据写入",
        path.display()
    )))
}
