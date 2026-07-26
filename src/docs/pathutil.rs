use crate::docs::error::{DocsError, DocsResult};
use std::path::{Path, PathBuf};

pub fn require_existing_file(path: impl AsRef<Path>) -> DocsResult<PathBuf> {
    let path = path.as_ref();
    match std::fs::metadata(path) {
        Ok(meta) => {
            if !meta.is_file() {
                return Err(DocsError::InvalidArgument(format!(
                    "不是文件: {}",
                    path.display()
                )));
            }
            Ok(path.to_path_buf())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(DocsError::PathNotFound(path.to_path_buf()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(DocsError::PermissionDenied(path.to_path_buf()))
        }
        Err(e) => Err(DocsError::IoError(e)),
    }
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
        let same_as_source = source
            .map(|s| paths_equal(s, out.as_path()))
            .unwrap_or(false);
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

pub fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    // 尽力规范化后再比，避免相对/绝对路径误判为不同文件
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuse_overwrite_when_exists() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.txt");
        std::fs::write(&p, b"x").unwrap();
        let err = resolve_output_path(None, Some(&p), false).unwrap_err();
        assert!(err.to_string().contains("覆盖") || err.to_string().contains("已存在"));
    }

    #[test]
    fn allow_overwrite_same_source() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.txt");
        std::fs::write(&p, b"x").unwrap();
        let out = resolve_output_path(Some(&p), None, false).unwrap();
        assert_eq!(out, p);
    }
}
