use crate::docs::error::{DocsError, DocsResult};
use crate::docs::pathutil::{require_existing_file, resolve_output_path};
use encoding_rs::{Encoding, GB18030, GBK, UTF_8};
use serde::Serialize;
use std::io::{BufRead, BufReader, Read};
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

fn detect_encoding(sample: &[u8], forced: Option<&str>) -> DocsResult<(&'static Encoding, String)> {
    if let Some(label) = forced {
        let enc = encoding_from_label(label)?;
        return Ok((enc, label.to_string()));
    }

    if sample.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok((UTF_8, "utf-8".into()));
    }

    let is_utf8 = match std::str::from_utf8(sample) {
        Ok(_) => true,
        Err(e) => e.error_len().is_none() && e.valid_up_to() > 0,
    };
    if is_utf8 {
        return Ok((UTF_8, "utf-8".into()));
    }

    let (_, _, gbk_errors) = GBK.decode(sample);
    if !gbk_errors {
        return Ok((GBK, "gbk".into()));
    }

    let (_, _, gb18030_errors) = GB18030.decode(sample);
    if !gb18030_errors {
        return Ok((GB18030, "gb18030".into()));
    }

    Ok((UTF_8, "utf-8-lossy".into()))
}

fn decode_bytes_with_enc(bytes: &[u8], enc: &'static Encoding, encoding_name: &str) -> String {
    if encoding_name == "utf-8" {
        if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            let (cow, _, _) = UTF_8.decode(bytes);
            return cow.into_owned();
        }
        if let Ok(s) = std::str::from_utf8(bytes) {
            return s.to_string();
        }
    }
    let (cow, _, _) = enc.decode(bytes);
    cow.into_owned()
}

fn count_lines_in_reader<R: Read>(mut reader: R) -> std::io::Result<usize> {
    let mut buf = [0u8; 16384];
    let mut line_count = 0;
    let mut total_bytes = 0u64;
    let mut last_byte = None;

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total_bytes += n as u64;
        last_byte = Some(buf[n - 1]);
        line_count += buf[..n].iter().filter(|&&b| b == b'\n').count();
    }

    if total_bytes > 0 && last_byte != Some(b'\n') {
        line_count += 1;
    }

    Ok(line_count)
}

pub fn read_text(path: impl AsRef<Path>, opts: ReadTextOptions) -> DocsResult<ReadTextResult> {
    let path = require_existing_file(path)?;
    let file = std::fs::File::open(&path)?;
    let mut reader = BufReader::new(file);

    let sample_buf = reader.fill_buf()?;
    let sample_len = sample_buf.len().min(4096);
    let sample = &sample_buf[..sample_len];
    let (enc, encoding) = detect_encoding(sample, opts.encoding.as_deref())?;

    let offset = opts.offset.unwrap_or(0);
    let limit = opts.limit.unwrap_or(DEFAULT_LINE_LIMIT);
    const MAX_LINE_BYTES: usize = 2 * 1024 * 1024;

    let mut current_line = 0;
    let mut buf = Vec::new();

    // 1. Skip lines before offset
    while current_line < offset {
        buf.clear();
        let bytes_read = reader
            .by_ref()
            .take(MAX_LINE_BYTES as u64)
            .read_until(b'\n', &mut buf)?;
        if bytes_read == 0 {
            break;
        }
        current_line += 1;
    }

    if current_line < offset {
        return Err(DocsError::InvalidArgument(format!(
            "offset {offset} 超出总行数 {current_line}"
        )));
    }

    // 2. Collect lines up to limit
    let mut out_bytes = Vec::new();
    let mut lines_collected = 0;
    let mut truncated_by_byte_limit = false;
    let mut hit_eof = false;
    let mut last_line_had_newline = true;

    while lines_collected < limit {
        buf.clear();
        let bytes_read = reader
            .by_ref()
            .take(MAX_LINE_BYTES as u64)
            .read_until(b'\n', &mut buf)?;
        if bytes_read == 0 {
            hit_eof = true;
            break;
        }
        current_line += 1;
        lines_collected += 1;
        last_line_had_newline = buf.ends_with(b"\n");

        if out_bytes.len() + buf.len() > DEFAULT_BYTE_LIMIT {
            let remaining = DEFAULT_BYTE_LIMIT.saturating_sub(out_bytes.len());
            out_bytes.extend_from_slice(&buf[..remaining]);
            truncated_by_byte_limit = true;
            break;
        } else {
            out_bytes.extend_from_slice(&buf);
        }
    }

    // 3. Calculate total lines
    let total_lines = if hit_eof {
        current_line
    } else {
        if !last_line_had_newline {
            let mut dummy = Vec::new();
            reader.read_until(b'\n', &mut dummy)?;
        }
        let remaining_lines = count_lines_in_reader(reader)?;
        current_line + remaining_lines
    };

    let mut text = decode_bytes_with_enc(&out_bytes, enc, &encoding);
    let mut truncated = (offset + lines_collected < total_lines) || truncated_by_byte_limit;

    if text.len() > DEFAULT_BYTE_LIMIT {
        let mut cut = DEFAULT_BYTE_LIMIT;
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        truncated = true;
    }

    Ok(ReadTextResult {
        ok: true,
        text,
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
    let file = std::fs::File::open(&path)?;
    let mut reader = BufReader::new(file);

    let sample_buf = reader.fill_buf()?;
    let sample_len = sample_buf.len().min(4096);
    let sample = &sample_buf[..sample_len];
    let (_, encoding) = detect_encoding(sample, None)?;

    let line_count = count_lines_in_reader(reader)?;
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
        let (enc, name) = detect_encoding(bytes, None).unwrap();
        assert!(!name.is_empty());
        let _ = decode_bytes_with_enc(bytes, enc, &name);
    }

    #[test]
    fn test_read_text_streaming_large_offset() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("large_log.txt");
        let mut content = String::new();
        for i in 1..=10000 {
            content.push_str(&format!("Line {i}: hello streaming world\n"));
        }
        std::fs::write(&file_path, &content).unwrap();

        let opts = ReadTextOptions {
            encoding: None,
            limit: Some(10),
            offset: Some(5000),
        };
        let res = read_text(&file_path, opts).unwrap();
        assert!(res.ok);
        assert_eq!(res.total_lines, 10000);
        assert!(res.truncated);
        assert!(res.text.starts_with("Line 5001:"));
    }
}
