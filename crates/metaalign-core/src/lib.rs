//! MetaAlign core engine.
//!
//! * [`model`] – declarative description of a batch run (serialisable, so the
//!   GUI and the WASM bindings share exactly the same vocabulary).
//! * [`fsmeta`] – creation/modification/access timestamps and DOS attributes.
//! * [`office`] – OOXML `docProps` and legacy OLE property sets.
//! * [`random`] / [`names`] – timestamp and author synthesis.
//! * [`engine`] – the batch runner.
//!
//! The crate is `std`-only but platform independent: Windows gets the full
//! feature set through `kernel32`, other targets degrade gracefully, and
//! `wasm32` only exposes the in-memory Office pipeline.

pub mod engine;
pub mod error;
pub mod fsmeta;
pub mod model;
pub mod names;
pub mod office;
pub mod random;
pub mod scan;

pub use error::{MetaError, Result};
pub use model::{
    AttributeRule, BackupMode, BatchPlan, FieldMode, FileFormat, FlagMode, NameRule, NameStyle,
    OfficeRule, RunOptions, SequenceMode, TextField, TimeField, TimePatch, TimeRange, TimeRule,
};
pub use office::{OfficeFormat, OfficeProps, OfficeUpdate};
pub use scan::ScanFilter;

/// Everything a caller usually needs.
pub mod prelude {
    pub use crate::engine::{
        BatchReport, ChangeRecord, ItemOutcome, ItemStatus, apply_one, format_time, run, validate,
    };
    pub use crate::model::{
        AttributeRule, BackupMode, BatchPlan, FieldMode, FlagMode, NameRule, NameStyle, OfficeRule,
        RunOptions, SequenceMode, TextField, TimeField, TimeRange, TimeRule,
    };
    pub use crate::office::{OfficeFormat, OfficeProps, OfficeUpdate};
    pub use crate::{MetaError, Result};
}

/// Crate version, surfaced in the UI footer.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
