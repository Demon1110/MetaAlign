//! Unix fallback: timestamps through `filetime`, creation time unsupported.

use std::path::Path;

use chrono::{DateTime, Utc};
use filetime::FileTime;

use super::{FsPatch, FsSnapshot, system_time_to_utc};
use crate::error::{MetaError, Result};

pub fn read_snapshot(path: &Path) -> Result<FsSnapshot> {
    let metadata = std::fs::metadata(path)?;
    Ok(FsSnapshot {
        size: metadata.len(),
        readonly: metadata.permissions().readonly(),
        hidden: false,
        system: false,
        archive: false,
        created: system_time_to_utc(metadata.created()),
        modified: system_time_to_utc(metadata.modified()),
        accessed: system_time_to_utc(metadata.accessed()),
    })
}

fn to_file_time(value: DateTime<Utc>) -> FileTime {
    FileTime::from_unix_time(value.timestamp(), value.timestamp_subsec_nanos())
}

pub fn apply_patch(path: &Path, patch: &FsPatch) -> Result<()> {
    if patch.created.is_some() {
        log::warn!("{path:?}: 创建时间在当前平台不可写，已跳过");
    }
    if patch.accessed.is_some() || patch.modified.is_some() {
        let metadata = std::fs::metadata(path)?;
        let current_atime = metadata
            .accessed()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let current_mtime = metadata
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let atime = patch
            .accessed
            .map(to_file_time)
            .unwrap_or_else(|| FileTime::from_system_time(current_atime));
        let mtime = patch
            .modified
            .map(to_file_time)
            .unwrap_or_else(|| FileTime::from_system_time(current_mtime));
        filetime::set_file_times(path, atime, mtime)?;
    }
    if patch.wants_attributes() {
        log::warn!("{path:?}: 隐藏/系统/存档属性仅 Windows 支持，已跳过");
        if let Some(readonly) = patch.readonly {
            let mut permissions = std::fs::metadata(path)?.permissions();
            permissions.set_readonly(readonly);
            std::fs::set_permissions(path, permissions)?;
        }
    }
    Ok(())
}

pub fn clear_readonly_for_write(path: &Path) -> Result<bool> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.permissions().readonly() {
        return Ok(false);
    }
    let mut permissions = metadata.permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions)?;
    Ok(true)
}

pub fn set_readonly(path: &Path, readonly: bool) -> Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_readonly(readonly);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[allow(dead_code)]
pub fn unsupported_creation_time() -> MetaError {
    MetaError::Platform("当前平台不支持写入创建时间".to_string())
}
