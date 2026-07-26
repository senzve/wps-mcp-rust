use std::path::PathBuf;
use thiserror::Error;

pub type DocsResult<T> = Result<T, DocsError>;

#[derive(Debug, Error)]
pub enum DocsError {
    #[error("路径不存在: {0}")]
    PathNotFound(PathBuf),
    #[error("权限不足: {0}")]
    PermissionDenied(PathBuf),
    #[error("不支持的格式: {0}")]
    UnsupportedFormat(String),
    #[error("解析失败: {0}")]
    ParseError(String),
    #[error("内嵌对象无法抽取: {0}")]
    EmbeddingNotExtractable(String),
    #[error("参数无效: {0}")]
    InvalidArgument(String),
    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

impl DocsError {
    pub fn to_public_message(&self) -> String {
        self.to_string()
    }
}
