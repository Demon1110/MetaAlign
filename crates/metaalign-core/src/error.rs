//! Error types shared by the whole engine.

use std::path::PathBuf;

/// Every fallible operation in the engine returns this error type.
#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("文件读写失败：{0}")]
    Io(#[from] std::io::Error),

    #[error("压缩包处理失败：{0}")]
    Zip(String),

    #[error("文档格式不支持：{0}")]
    Unsupported(String),

    #[error("元数据解析失败：{0}")]
    Parse(String),

    #[error("当前平台不支持该操作：{0}")]
    Platform(String),

    #[error("无效路径：{0}")]
    InvalidPath(PathBuf),
}

impl From<zip::result::ZipError> for MetaError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::Zip(value.to_string())
    }
}

impl MetaError {
    /// Short, user facing category used by the UI.
    pub fn category(&self) -> &'static str {
        match self {
            Self::Io(_) => "IO",
            Self::Zip(_) => "ZIP",
            Self::Unsupported(_) => "UNSUPPORTED",
            Self::Parse(_) => "PARSE",
            Self::Platform(_) => "PLATFORM",
            Self::InvalidPath(_) => "PATH",
        }
    }
}

pub type Result<T> = std::result::Result<T, MetaError>;
