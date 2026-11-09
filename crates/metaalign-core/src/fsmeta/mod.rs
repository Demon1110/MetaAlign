//! Filesystem attribute access (timestamps + DOS attribute bits).
//!
//! The Windows implementation talks to `kernel32` directly so that *creation*
//! time can be written; other platforms fall back to `filetime` and simply
//! report the unsupported bits.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as platform;

#[cfg(all(not(windows), not(target_arch = "wasm32")))]
mod fallback;
#[cfg(all(not(windows), not(target_arch = "wasm32")))]
use fallback as platform;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
use web as platform;

/// Whether the current platform can *write* a file's creation time.
///
/// Only the Win32 API exposes this; Unix systems have no creation time that can
/// be set after the fact, so the engine drops the request and reports it instead
/// of pretending it succeeded.
pub const fn supports_created_time() -> bool {
    cfg!(windows)
}

/// Whether the DOS attribute bits (hidden / system / archive) mean anything here.
pub const fn supports_attribute_bits() -> bool {
    cfg!(windows)
}

/// The current state of a file on disk.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FsSnapshot {
    pub size: u64,
    pub readonly: bool,
    pub hidden: bool,
    pub system: bool,
    pub archive: bool,
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub accessed: Option<DateTime<Utc>>,
}

impl FsSnapshot {
    /// Egui-friendly one-line summary of the attribute bits.
    pub fn attribute_summary(&self) -> String {
        let mut parts = Vec::new();
        if self.readonly {
            parts.push("只读");
        }
        if self.hidden {
            parts.push("隐藏");
        }
        if self.system {
            parts.push("系统");
        }
        if self.archive {
            parts.push("存档");
        }
        if parts.is_empty() {
            "常规".to_string()
        } else {
            parts.join(" / ")
        }
    }
}

/// What to change. `None` means "leave untouched".
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FsPatch {
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub accessed: Option<DateTime<Utc>>,
    pub readonly: Option<bool>,
    pub hidden: Option<bool>,
    pub system: Option<bool>,
    pub archive: Option<bool>,
}

impl FsPatch {
    pub fn wants_times(&self) -> bool {
        self.created.is_some() || self.modified.is_some() || self.accessed.is_some()
    }

    pub fn wants_attributes(&self) -> bool {
        self.readonly.is_some()
            || self.hidden.is_some()
            || self.system.is_some()
            || self.archive.is_some()
    }

    pub fn is_empty(&self) -> bool {
        !self.wants_times() && !self.wants_attributes()
    }
}

/// Read size, timestamps and attribute bits for `path`.
pub fn read_snapshot(path: &Path) -> Result<FsSnapshot> {
    platform::read_snapshot(path)
}

/// Apply the patch to `path`.
pub fn apply_patch(path: &Path, patch: &FsPatch) -> Result<()> {
    platform::apply_patch(path, patch)
}

/// Temporarily remove the read-only bit so the file can be rewritten.
///
/// Returns `true` when the bit was actually cleared and should be restored.
pub fn clear_readonly_for_write(path: &Path) -> Result<bool> {
    platform::clear_readonly_for_write(path)
}

/// Restore the read-only bit after a write.
pub fn restore_readonly(path: &Path, readonly: bool) -> Result<()> {
    platform::set_readonly(path, readonly)
}

/// Convert a UTC instant into a Windows `FILETIME` (100 ns ticks since 1601).
pub(crate) fn unix_to_filetime_ticks(dt: DateTime<Utc>) -> u64 {
    const TICKS_1601_TO_1970: i128 = 116_444_736_000_000_000;
    let nanos = dt.timestamp_nanos_opt().unwrap_or_default() as i128;
    let ticks = nanos / 100 + TICKS_1601_TO_1970;
    ticks.max(0) as u64
}

/// Inverse of [`unix_to_filetime_ticks`].
pub(crate) fn filetime_ticks_to_unix(ticks: u64) -> Option<DateTime<Utc>> {
    const TICKS_1601_TO_1970: i128 = 116_444_736_000_000_000;
    let nanos = (ticks as i128 - TICKS_1601_TO_1970) * 100;
    let secs = (nanos / 1_000_000_000) as i64;
    let sub = (nanos % 1_000_000_000) as u32;
    DateTime::from_timestamp(secs, sub)
}

/// Best-effort conversion of `SystemTime` into UTC.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) fn system_time_to_utc(
    time: std::io::Result<std::time::SystemTime>,
) -> Option<DateTime<Utc>> {
    let time = time.ok()?;
    let since_epoch = time.duration_since(std::time::UNIX_EPOCH).ok()?;
    DateTime::from_timestamp(since_epoch.as_secs() as i64, since_epoch.subsec_nanos())
}
