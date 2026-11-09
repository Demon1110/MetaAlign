//! Browser stub: the engine never touches a real filesystem under WASM.
//!
//! Only the in-memory Office metadata pipeline is available there; the GUI
//! hides every filesystem control when compiled for `wasm32`.

use std::path::Path;

use super::{FsPatch, FsSnapshot};
use crate::error::{MetaError, Result};

fn unsupported(what: &str) -> MetaError {
    MetaError::Platform(format!("浏览器环境不支持 {what}，请在桌面版中执行"))
}

pub fn read_snapshot(path: &Path) -> Result<FsSnapshot> {
    log::debug!("read_snapshot({path:?}) ignored in the browser build");
    Err(unsupported("读取文件系统时间"))
}

pub fn apply_patch(_path: &Path, _patch: &FsPatch) -> Result<()> {
    Err(unsupported("写入文件系统属性"))
}

pub fn clear_readonly_for_write(_path: &Path) -> Result<bool> {
    Ok(false)
}

pub fn set_readonly(_path: &Path, _readonly: bool) -> Result<()> {
    Err(unsupported("修改只读属性"))
}
