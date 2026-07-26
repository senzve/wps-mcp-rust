use crate::docs::error::{DocsError, DocsResult};
use std::path::{Path, PathBuf};

pub fn require_existing_file(path: impl AsRef<Path>) -> DocsResult<PathBuf> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(DocsError::PathNotFound(path.to_path_buf()));
    }
    if !path.is_file() {
        return Err(DocsError::InvalidArgument(format!(
            "不是文件: {}",
            path.display()
        )));
    }
    Ok(path.to_path_buf())
}

pub fn ensure_parent_dir(path: impl AsRef<Path>) -> DocsResult<()> {
    if let Some(parent) = path.as_ref().parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

pub fn resolve_output_path(
    source: Option<&Path>,
    output_path: Option<&Path>,
    overwrite: bool,
) -> DocsResult<PathBuf> {
    let out = match (output_path, source) {
        (Some(p), _) => p.to_path_buf(),
        (None, Some(s)) => s.to_path_buf(),
        (None, None) => return Err(DocsError::InvalidArgument("必须提供 output_path".into())),
    };

    if out.exists() {
        let same_as_source = source.map(|s| s == out.as_path()).unwrap_or(false);
        if !overwrite && !same_as_source {
            return Err(DocsError::InvalidArgument(format!(
                "目标已存在且未允许覆盖: {}",
                out.display()
            )));
        }
    }
    ensure_parent_dir(&out)?;
    Ok(out)
}
