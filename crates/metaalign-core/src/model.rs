//! Declarative description of everything MetaAlign can rewrite.
//!
//! A [`BatchPlan`] is pure data: the GUI edits it, the engine executes it and
//! the WASM bindings accept it as JSON.

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::office::OfficeFormat;

/// Fallback instant used when a [`TimeField`] has no explicit value yet.
fn epoch_default() -> DateTime<Utc> {
    DateTime::from_timestamp(1_600_000_000, 0).unwrap_or_else(Utc::now)
}

/// How a single field should be produced for each file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldMode {
    /// Leave whatever the file currently contains.
    #[default]
    Keep,
    /// Write one fixed value for every file.
    Fixed,
    /// Generate a fresh value per file.
    Random,
}

impl FieldMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Keep => "保留",
            Self::Fixed => "指定",
            Self::Random => "随机",
        }
    }
}

/// A time value together with the mode describing how to use it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeField {
    pub mode: FieldMode,
    pub fixed: DateTime<Utc>,
}

impl Default for TimeField {
    fn default() -> Self {
        Self {
            mode: FieldMode::Keep,
            fixed: epoch_default(),
        }
    }
}

impl TimeField {
    pub fn keep() -> Self {
        Self::default()
    }

    pub fn fixed(value: DateTime<Utc>) -> Self {
        Self {
            mode: FieldMode::Fixed,
            fixed: value,
        }
    }

    pub fn random() -> Self {
        Self {
            mode: FieldMode::Random,
            fixed: epoch_default(),
        }
    }

    pub fn is_keep(&self) -> bool {
        self.mode == FieldMode::Keep
    }
}

/// A textual value together with the mode describing how to use it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextField {
    pub mode: FieldMode,
    pub value: String,
}

impl TextField {
    pub fn keep() -> Self {
        Self::default()
    }

    pub fn fixed(value: impl Into<String>) -> Self {
        Self {
            mode: FieldMode::Fixed,
            value: value.into(),
        }
    }

    pub fn random() -> Self {
        Self {
            mode: FieldMode::Random,
            value: String::new(),
        }
    }

    pub fn is_keep(&self) -> bool {
        self.mode == FieldMode::Keep
    }
}

/// Inclusive time window used for random generation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl Default for TimeRange {
    fn default() -> Self {
        let end = Utc::now();
        Self {
            start: end - TimeDelta::days(365),
            end,
        }
    }
}

impl TimeRange {
    /// Returns the range with `start <= end`, so callers never have to care
    /// about the user typing the bounds in reverse order.
    pub fn normalized(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    /// The window length in milliseconds, at least one millisecond.
    pub fn span_millis(&self) -> i64 {
        let (start, end) = self.normalized();
        (end.timestamp_millis() - start.timestamp_millis()).max(1)
    }

    pub fn is_valid(&self) -> bool {
        self.start < self.end
    }
}

/// How random timestamps are distributed inside the range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceMode {
    /// Every file gets an unrelated instant.
    #[default]
    Independent,
    /// Files are ordered by name and spread from the range start to its end.
    Ascending,
    /// Files are ordered by name and spread from the range end to its start.
    Descending,
}

impl SequenceMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Independent => "独立随机",
            Self::Ascending => "顺序递增",
            Self::Descending => "顺序递减",
        }
    }
}

/// Rules for the three filesystem timestamps.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeRule {
    pub created: TimeField,
    pub modified: TimeField,
    pub accessed: TimeField,
    pub range: TimeRange,
    pub sequence: SequenceMode,
    /// Keep `created <= modified <= accessed`, so generated sets look plausible.
    pub respect_order: bool,
}

impl Default for TimeRule {
    fn default() -> Self {
        Self {
            created: TimeField::random(),
            modified: TimeField::random(),
            accessed: TimeField::random(),
            range: TimeRange::default(),
            sequence: SequenceMode::Independent,
            respect_order: true,
        }
    }
}

impl TimeRule {
    pub fn any_time_needed(&self) -> bool {
        !self.created.is_keep() || !self.modified.is_keep() || !self.accessed.is_keep()
    }
}

/// Tri-state switch for Windows DOS attributes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlagMode {
    #[default]
    Keep,
    On,
    Off,
}

impl FlagMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Keep => "保留",
            Self::On => "开启",
            Self::Off => "关闭",
        }
    }

    /// `Some(true)` = enable, `Some(false)` = disable, `None` = untouched.
    pub fn as_patch(self) -> Option<bool> {
        match self {
            Self::Keep => None,
            Self::On => Some(true),
            Self::Off => Some(false),
        }
    }
}

/// Rules for the Windows DOS attribute bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributeRule {
    pub readonly: FlagMode,
    pub hidden: FlagMode,
    pub system: FlagMode,
    pub archive: FlagMode,
}

impl AttributeRule {
    pub fn any_change(&self) -> bool {
        self.readonly != FlagMode::Keep
            || self.hidden != FlagMode::Keep
            || self.system != FlagMode::Keep
            || self.archive != FlagMode::Keep
    }
}

/// Name flavour used when an author-ish field is set to random.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NameStyle {
    #[default]
    Chinese,
    English,
    Mixed,
}

impl NameStyle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Chinese => "中文姓名",
            Self::English => "英文姓名",
            Self::Mixed => "中英混合",
        }
    }
}

/// Source of random names.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NameRule {
    pub style: NameStyle,
    /// When non-empty (and `use_custom` is true) names are drawn from here.
    pub use_custom: bool,
    pub custom: Vec<String>,
}

impl Default for NameRule {
    fn default() -> Self {
        Self {
            style: NameStyle::Chinese,
            use_custom: false,
            custom: Vec::new(),
        }
    }
}

/// Rules for the metadata stored *inside* Office documents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OfficeRule {
    pub author: TextField,
    pub last_modified_by: TextField,
    pub title: TextField,
    pub subject: TextField,
    pub keywords: TextField,
    pub category: TextField,
    pub comments: TextField,
    pub company: TextField,
    pub manager: TextField,
    pub revision: TextField,
    pub created: TimeField,
    pub modified: TimeField,
    /// Reuse the per-file values generated for the filesystem timestamps.
    pub follow_file_time: bool,
}

impl Default for OfficeRule {
    fn default() -> Self {
        Self {
            author: TextField::random(),
            last_modified_by: TextField::random(),
            title: TextField::keep(),
            subject: TextField::keep(),
            keywords: TextField::keep(),
            category: TextField::keep(),
            comments: TextField::keep(),
            company: TextField::keep(),
            manager: TextField::keep(),
            revision: TextField::keep(),
            created: TimeField::keep(),
            modified: TimeField::keep(),
            follow_file_time: true,
        }
    }
}

impl OfficeRule {
    pub fn any_text_change(&self) -> bool {
        [
            &self.author,
            &self.last_modified_by,
            &self.title,
            &self.subject,
            &self.keywords,
            &self.category,
            &self.comments,
            &self.company,
            &self.manager,
            &self.revision,
        ]
        .iter()
        .any(|field| !field.is_keep())
    }

    pub fn any_time_change(&self) -> bool {
        !self.created.is_keep() || !self.modified.is_keep()
    }

    pub fn any_change(&self) -> bool {
        self.any_text_change() || self.any_time_change()
    }
}

/// Where a safety copy is written before a file is modified.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackupMode {
    /// No copy at all.
    None,
    /// `<file>.<ext>.metaalign.bak` next to the original.
    #[default]
    Sidecar,
    /// `metaalign_backup/` folder next to the original.
    Folder,
}

impl BackupMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "不备份",
            Self::Sidecar => "同名 .bak",
            Self::Folder => "backup 目录",
        }
    }
}

/// Runtime switches that are not per-field rules.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunOptions {
    pub backup: BackupMode,
    pub dry_run: bool,
    /// Only touch files that actually support Office metadata.
    pub only_office_files: bool,
    /// Report files that need no change as skipped instead of success.
    pub skip_unchanged: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            backup: BackupMode::Sidecar,
            dry_run: false,
            only_office_files: false,
            skip_unchanged: true,
        }
    }
}

/// Everything the engine needs for one run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BatchPlan {
    pub time: TimeRule,
    pub attrs: AttributeRule,
    pub office: OfficeRule,
    pub names: NameRule,
    pub options: RunOptions,
}

impl BatchPlan {
    /// True when the plan would leave every file untouched.
    pub fn is_noop(&self) -> bool {
        !self.time.any_time_needed()
            && !self.attrs.any_change()
            && !self.office.any_change()
    }
}

/// Resolved per-file timestamps (`None` means "keep the current value").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimePatch {
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub accessed: Option<DateTime<Utc>>,
}

impl TimePatch {
    pub fn is_empty(&self) -> bool {
        self.created.is_none() && self.modified.is_none() && self.accessed.is_none()
    }
}

/// The file formats MetaAlign knows how to inspect.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileFormat {
    #[default]
    Other,
    Office(OfficeFormat),
}

impl FileFormat {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Other => "普通文件",
            Self::Office(format) => format.label(),
        }
    }

    pub fn is_office(&self) -> bool {
        matches!(self, Self::Office(format) if format.supports_office_meta())
    }
}
