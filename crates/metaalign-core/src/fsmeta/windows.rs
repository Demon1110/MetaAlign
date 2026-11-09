//! Windows implementation backed by `kernel32` (`CreateFileW` + `SetFileTime`).

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_ARCHIVE, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_SYSTEM, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    FILE_WRITE_ATTRIBUTES, GetFileAttributesW, OPEN_EXISTING, SetFileAttributesW, SetFileTime,
};

use super::{
    FsPatch, FsSnapshot, filetime_ticks_to_unix, system_time_to_utc, unix_to_filetime_ticks,
};
use crate::error::{MetaError, Result};

const INVALID_ATTRIBUTES: u32 = u32::MAX;

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn read_attributes(path: &Path) -> io::Result<u32> {
    let wide = wide(path);
    let attributes = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attributes == INVALID_ATTRIBUTES {
        Err(io::Error::last_os_error())
    } else {
        Ok(attributes)
    }
}

fn write_attributes(path: &Path, attributes: u32) -> io::Result<()> {
    let wide = wide(path);
    let value = if attributes == 0 {
        FILE_ATTRIBUTE_NORMAL
    } else {
        attributes
    };
    let ok = unsafe { SetFileAttributesW(wide.as_ptr(), value) };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn open_for_attributes(path: &Path, access: u32) -> io::Result<HANDLE> {
    let wide = wide(path);
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(handle)
    }
}

pub fn read_snapshot(path: &Path) -> Result<FsSnapshot> {
    let metadata = std::fs::metadata(path)?;
    let attributes = read_attributes(path).unwrap_or(0);
    Ok(FsSnapshot {
        size: metadata.len(),
        readonly: attributes & FILE_ATTRIBUTE_READONLY != 0,
        hidden: attributes & FILE_ATTRIBUTE_HIDDEN != 0,
        system: attributes & FILE_ATTRIBUTE_SYSTEM != 0,
        archive: attributes & FILE_ATTRIBUTE_ARCHIVE != 0,
        created: system_time_to_utc(metadata.created()),
        modified: system_time_to_utc(metadata.modified()),
        accessed: system_time_to_utc(metadata.accessed()),
    })
}

pub fn apply_patch(path: &Path, patch: &FsPatch) -> Result<()> {
    if patch.wants_times() {
        write_times(path, patch)?;
    }
    if patch.wants_attributes() {
        write_attribute_bits(path, patch)?;
    }
    Ok(())
}

fn write_times(path: &Path, patch: &FsPatch) -> Result<()> {
    if !path.exists() {
        return Err(MetaError::InvalidPath(path.to_path_buf()));
    }
    let handle = open_for_attributes(path, FILE_WRITE_ATTRIBUTES | FILE_READ_ATTRIBUTES)?;
    let created = patch.created.map(unix_to_filetime_ticks).map(to_filetime);
    let accessed = patch.accessed.map(unix_to_filetime_ticks).map(to_filetime);
    let modified = patch.modified.map(unix_to_filetime_ticks).map(to_filetime);

    let created_ptr = created.as_ref().map_or(std::ptr::null(), |value| value);
    let accessed_ptr = accessed.as_ref().map_or(std::ptr::null(), |value| value);
    let modified_ptr = modified.as_ref().map_or(std::ptr::null(), |value| value);

    let ok = unsafe { SetFileTime(handle, created_ptr, accessed_ptr, modified_ptr) };
    let result = if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    };
    unsafe {
        CloseHandle(handle);
    }
    result.map_err(MetaError::from)
}

fn to_filetime(ticks: u64) -> FILETIME {
    FILETIME {
        dwLowDateTime: (ticks & 0xFFFF_FFFF) as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    }
}

fn write_attribute_bits(path: &Path, patch: &FsPatch) -> Result<()> {
    let mut attributes = read_attributes(path)?;
    for (enabled, bit) in [
        (patch.readonly, FILE_ATTRIBUTE_READONLY),
        (patch.hidden, FILE_ATTRIBUTE_HIDDEN),
        (patch.system, FILE_ATTRIBUTE_SYSTEM),
        (patch.archive, FILE_ATTRIBUTE_ARCHIVE),
    ] {
        if let Some(enable) = enabled {
            if enable {
                attributes |= bit;
            } else {
                attributes &= !bit;
            }
        }
    }
    write_attributes(path, attributes)?;
    Ok(())
}

pub fn clear_readonly_for_write(path: &Path) -> Result<bool> {
    let attributes = read_attributes(path)?;
    if attributes & FILE_ATTRIBUTE_READONLY == 0 {
        return Ok(false);
    }
    write_attributes(path, attributes & !FILE_ATTRIBUTE_READONLY)?;
    Ok(true)
}

pub fn set_readonly(path: &Path, readonly: bool) -> Result<()> {
    let mut attributes = read_attributes(path)?;
    if readonly {
        attributes |= FILE_ATTRIBUTE_READONLY;
    } else {
        attributes &= !FILE_ATTRIBUTE_READONLY;
    }
    write_attributes(path, attributes)?;
    Ok(())
}

/// Read the three timestamps through the Win32 API (used by tests).
#[allow(dead_code)]
pub fn read_filetime(path: &Path) -> Result<[Option<chrono::DateTime<chrono::Utc>>; 3]> {
    use windows_sys::Win32::Storage::FileSystem::GetFileTime;
    let handle = open_for_attributes(path, FILE_READ_ATTRIBUTES)?;
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut accessed = created;
    let mut modified = created;
    let ok = unsafe { GetFileTime(handle, &mut created, &mut accessed, &mut modified) };
    unsafe {
        CloseHandle(handle);
    }
    if ok == 0 {
        return Err(MetaError::from(io::Error::last_os_error()));
    }
    let convert = |ft: FILETIME| {
        filetime_ticks_to_unix(((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64)
    };
    Ok([convert(created), convert(accessed), convert(modified)])
}
