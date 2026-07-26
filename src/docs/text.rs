use crate::docs::error::{DocsError, DocsResult};
use crate::docs::pathutil::{require_existing_file, resolve_output_path};
use encoding_rs::{Encoding, GB18030, GBK, UTF_8};
use serde::Serialize;
use std::path::Path;

pub const DEFAULT_LINE_LIMIT: usize = 5000;
pub const DEFAULT_BYTE_LIMIT: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct ReadTextOptions {
    pub encoding: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadTextResult {
    pub ok: bool,
    pub text: String,
    pub encoding: String,
    pub truncated: bool,
    pub total_lines: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextInfo {
    pub ok: bool,
    pub path: String,
    pub size: u64,
    pub encoding: String,
    pub line_count: usize,
}

pub fn read_text(path: impl AsRef<Path>, opts: ReadTextOptions) -> DocsResult<ReadTextResult> {
    let path = require_existing_file(path)?;
    let bytes = std::fs::read(&path)?;
    let (text, encoding) = decode_bytes(&bytes, opts.encoding.as_deref())?;

    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let total_lines = lines.len();
    let offset = opts.offset.unwrap_or(0);
    let limit = opts.limit.unwrap_or(DEFAULT_LINE_LIMIT);

    if offset > total_lines {
        return Err(DocsError::InvalidArgument(format!(
            "offset {offset} 超出总行数 {total_lines}"
        )));
    }

    let end = (offset + limit).min(total_lines);
    let slice = &lines[offset..end];
    let mut out = slice.concat();
    let mut truncated = end < total_lines;

    if out.len() > DEFAULT_BYTE_LIMIT {
        let mut cut = DEFAULT_BYTE_LIMIT;
        while cut > 0 && !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        truncated = true;
    }

    Ok(ReadTextResult {
        ok: true,
        text: out,
        encoding,
        truncated,
        total_lines,
    })
}

pub fn write_text(
    path: impl AsRef<Path>,
    content: &str,
    encoding: Option<&str>,
    overwrite: bool,
) -> DocsResult<String> {
    let out = resolve_output_path(None, Some(path.as_ref()), overwrite)?;
    let bytes = encode_string(content, encoding)?;
    std::fs::write(&out, bytes)?;
    Ok(out.display().to_string())
}

pub fn text_info(path: impl AsRef<Path>) -> DocsResult<TextInfo> {
    let path = require_existing_file(path)?;
    let meta = std::fs::metadata(&path)?;
    let bytes = std::fs::read(&path)?;
    let (text, encoding) = decode_bytes(&bytes, None)?;
    let line_count = if text.is_empty() {
        0
    } else {
        text.lines().count()
    };
    Ok(TextInfo {
        ok: true,
        path: path.display().to_string(),
        size: meta.len(),
        encoding,
        line_count,
    })
}

fn encoding_from_label(label: &str) -> DocsResult<&'static Encoding> {
    Encoding::for_label(label.as_bytes())
        .ok_or_else(|| DocsError::InvalidArgument(format!("未知编码: {label}")))
}

fn decode_bytes(bytes: &[u8], forced: Option<&str>) -> DocsResult<(String, String)> {
    if let Some(label) = forced {
        let enc = encoding_from_label(label)?;
        let (cow, _, _) = enc.decode(bytes);
        return Ok((cow.into_owned(), label.to_string()));
    }

    if let Ok(s) = std::str::from_utf8(bytes) {
        return Ok((s.to_string(), "utf-8".into()));
    }

    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (cow, _, _) = UTF_8.decode(bytes);
        return Ok((cow.into_owned(), "utf-8".into()));
    }

    for (enc, name) in [(GBK, "gbk"), (GB18030, "gb18030")] {
        let (cow, _, had_errors) = enc.decode(bytes);
        if !had_errors {
            return Ok((cow.into_owned(), name.into()));
        }
    }

    let (cow, _, _) = UTF_8.decode(bytes);
    Ok((cow.into_owned(), "utf-8-lossy".into()))
}

fn encode_string(content: &str, encoding: Option<&str>) -> DocsResult<Vec<u8>> {
    match encoding {
        None | Some("utf-8") | Some("UTF-8") => Ok(content.as_bytes().to_vec()),
        Some(label) => {
            let enc = encoding_from_label(label)?;
            let (cow, _, unmatched) = enc.encode(content);
            if unmatched {
                return Err(DocsError::InvalidArgument(format!(
                    "内容无法用编码 {label} 完整表示"
                )));
            }
            Ok(cow.into_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossy_fallback_does_not_panic() {
        let bytes = b"\xff\xfe not valid utf8 \x80\x81";
        let (text, enc) = decode_bytes(bytes, None).unwrap();
        assert!(!text.is_empty() || enc == "utf-8-lossy" || enc.starts_with("gb"));
    }
}
