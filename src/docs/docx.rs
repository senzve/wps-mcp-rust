use crate::docs::error::{DocsError, DocsResult};
use crate::docs::pathutil::{paths_equal, require_existing_file, resolve_output_path};
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};
use serde::Serialize;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Serialize)]
pub struct DocxTextResult {
    pub ok: bool,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct DocxTablesResult {
    pub ok: bool,
    pub tables: Vec<Vec<Vec<String>>>,
}

#[derive(Debug, Serialize)]
pub struct DocxMarkdownResult {
    pub ok: bool,
    pub markdown: String,
    pub output_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DocxWriteResult {
    pub ok: bool,
    pub output_path: String,
}

pub fn read_text(path: impl AsRef<Path>) -> DocsResult<DocxTextResult> {
    let xml = read_document_xml(path)?;
    let (text, _) = extract_text_and_tables(&xml)?;
    Ok(DocxTextResult { ok: true, text })
}

pub fn read_tables(path: impl AsRef<Path>) -> DocsResult<DocxTablesResult> {
    let xml = read_document_xml(path)?;
    let (_, tables) = extract_text_and_tables(&xml)?;
    Ok(DocxTablesResult { ok: true, tables })
}

pub fn to_markdown(
    path: impl AsRef<Path>,
    output_path: Option<&Path>,
) -> DocsResult<DocxMarkdownResult> {
    let xml = read_document_xml(&path)?;
    let (text, tables) = extract_text_and_tables(&xml)?;
    let mut md = text.trim().to_string();
    for table in tables {
        if table.is_empty() {
            continue;
        }
        md.push_str("\n\n");
        md.push('|');
        for c in &table[0] {
            md.push_str(&format!(" {c} |"));
        }
        md.push('\n');
        md.push('|');
        for _ in &table[0] {
            md.push_str(" --- |");
        }
        md.push('\n');
        for row in table.iter().skip(1) {
            md.push('|');
            for c in row {
                md.push_str(&format!(" {c} |"));
            }
            md.push('\n');
        }
    }
    let mut out_path = None;
    if let Some(p) = output_path {
        let out = resolve_output_path(None, Some(p), false)?;
        std::fs::write(&out, &md)?;
        out_path = Some(out.display().to_string());
    }
    Ok(DocxMarkdownResult {
        ok: true,
        markdown: md,
        output_path: out_path,
    })
}

pub fn create(path: impl AsRef<Path>, content: &str, format: &str) -> DocsResult<DocxWriteResult> {
    // 新建文件：目标已存在时拒绝覆盖（符合写操作默认不覆盖约定）
    let out = resolve_output_path(None, Some(path.as_ref()), false)?;
    let paragraphs: Vec<String> = match format {
        "text" | "markdown" => content
            .lines()
            .map(|l| l.trim_start_matches('#').trim().to_string())
            .filter(|l| !l.is_empty() || format == "text")
            .collect(),
        other => {
            return Err(DocsError::InvalidArgument(format!(
                "不支持的 format: {other}（支持 text|markdown）"
            )))
        }
    };
    let paragraphs = if paragraphs.is_empty() {
        vec![String::new()]
    } else {
        paragraphs
    };
    let document_xml = build_document_xml(&paragraphs);
    write_minimal_docx(&out, &document_xml)?;
    Ok(DocxWriteResult {
        ok: true,
        output_path: out.display().to_string(),
    })
}

pub fn replace_text(
    path: impl AsRef<Path>,
    replacements: &[(String, String)],
    output_path: Option<&Path>,
) -> DocsResult<DocxWriteResult> {
    let src = require_existing_file(path)?;
    // 省略 output_path 时写回源文件；指定新路径时默认不覆盖已存在目标
    let overwrite = output_path.map(|p| paths_equal(&src, p)).unwrap_or(true);
    let out = resolve_output_path(Some(&src), output_path, overwrite)?;

    let file = std::fs::File::open(&src)?;
    let mut archive = ZipArchive::new(file).map_err(|e| DocsError::ParseError(e.to_string()))?;

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| DocsError::ParseError(e.to_string()))?;
            let name = entry.name().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;

            if name == "word/document.xml" {
                let xml = String::from_utf8(data)
                    .map_err(|e| DocsError::ParseError(format!("document.xml 非 UTF-8: {e}")))?;
                let rewritten = replace_in_document_xml(&xml, replacements)?;
                writer
                    .start_file(name, options)
                    .map_err(|e| DocsError::Other(e.to_string()))?;
                writer
                    .write_all(rewritten.as_bytes())
                    .map_err(|e| DocsError::Other(e.to_string()))?;
            } else {
                writer
                    .start_file(name, options)
                    .map_err(|e| DocsError::Other(e.to_string()))?;
                writer
                    .write_all(&data)
                    .map_err(|e| DocsError::Other(e.to_string()))?;
            }
        }
        writer
            .finish()
            .map_err(|e| DocsError::Other(e.to_string()))?;
    }

    std::fs::write(&out, buffer.into_inner())?;
    Ok(DocxWriteResult {
        ok: true,
        output_path: out.display().to_string(),
    })
}

fn read_document_xml(path: impl AsRef<Path>) -> DocsResult<String> {
    let path = require_existing_file(path)?;
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("docx"))
        != Some(true)
    {
        return Err(DocsError::UnsupportedFormat("仅支持 .docx".into()));
    }
    let file = std::fs::File::open(&path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| DocsError::ParseError(e.to_string()))?;
    let mut doc = zip
        .by_name("word/document.xml")
        .map_err(|e| DocsError::ParseError(format!("缺少 word/document.xml: {e}")))?;
    let mut xml = String::new();
    doc.read_to_string(&mut xml)?;
    Ok(xml)
}

fn extract_text_and_tables(xml: &str) -> DocsResult<(String, Vec<Vec<Vec<String>>>)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut text = String::new();
    let mut tables = Vec::new();
    let mut in_table = false;
    let mut current_table: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();
    let mut in_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match local.as_str() {
                    "tbl" => {
                        in_table = true;
                        current_table = Vec::new();
                    }
                    "tr" if in_table => current_row = Vec::new(),
                    "tc" if in_table => current_cell = String::new(),
                    "t" => in_t = true,
                    "tab" => {
                        if in_table {
                            current_cell.push('\t');
                        } else {
                            text.push('\t');
                        }
                    }
                    "br" | "cr" => {
                        if in_table {
                            current_cell.push('\n');
                        } else {
                            text.push('\n');
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match local.as_str() {
                    "t" => in_t = false,
                    "p" => {
                        if in_table {
                            if !current_cell.is_empty() && !current_cell.ends_with(' ') {
                                current_cell.push(' ');
                            }
                        } else {
                            text.push('\n');
                        }
                    }
                    "tc" if in_table => current_row.push(current_cell.trim().to_string()),
                    "tr" if in_table => current_table.push(std::mem::take(&mut current_row)),
                    "tbl" => {
                        in_table = false;
                        tables.push(std::mem::take(&mut current_table));
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) if in_t => {
                let decoded = t.unescape().unwrap_or_default();
                if in_table {
                    current_cell.push_str(&decoded);
                } else {
                    text.push_str(&decoded);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocsError::ParseError(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok((text, tables))
}

fn replace_in_document_xml(xml: &str, replacements: &[(String, String)]) -> DocsResult<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut buf = Vec::new();
    let mut in_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if local == "t" {
                    in_t = true;
                }
                writer
                    .write_event(Event::Start(e.into_owned()))
                    .map_err(|e| DocsError::Other(e.to_string()))?;
            }
            Ok(Event::End(e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if local == "t" {
                    in_t = false;
                }
                writer
                    .write_event(Event::End(e.into_owned()))
                    .map_err(|e| DocsError::Other(e.to_string()))?;
            }
            Ok(Event::Empty(e)) => {
                writer
                    .write_event(Event::Empty(e.into_owned()))
                    .map_err(|e| DocsError::Other(e.to_string()))?;
            }
            Ok(Event::Text(t)) => {
                if in_t {
                    let mut s = t.unescape().unwrap_or_default().to_string();
                    for (from, to) in replacements {
                        s = s.replace(from, to);
                    }
                    writer
                        .write_event(Event::Text(BytesText::new(&s)))
                        .map_err(|e| DocsError::Other(e.to_string()))?;
                } else {
                    writer
                        .write_event(Event::Text(t))
                        .map_err(|e| DocsError::Other(e.to_string()))?;
                }
            }
            Ok(Event::Eof) => break,
            Ok(e) => {
                writer
                    .write_event(e)
                    .map_err(|e| DocsError::Other(e.to_string()))?;
            }
            Err(e) => return Err(DocsError::ParseError(e.to_string())),
        }
        buf.clear();
    }

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).map_err(|e| DocsError::ParseError(e.to_string()))
}

fn build_document_xml(paragraphs: &[String]) -> String {
    let mut body = String::new();
    for p in paragraphs {
        let escaped = xml_escape(p);
        body.push_str(&format!(
            "<w:p><w:r><w:t xml:space=\"preserve\">{escaped}</w:t></w:r></w:p>"
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    {body}
  </w:body>
</w:document>"#
    )
}

fn write_minimal_docx(path: &Path, document_xml: &str) -> DocsResult<()> {
    let file = std::fs::File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

    for (name, data) in [
        ("[Content_Types].xml", content_types),
        ("_rels/.rels", rels),
        ("word/document.xml", document_xml),
    ] {
        zip.start_file(name, options)
            .map_err(|e| DocsError::Other(e.to_string()))?;
        zip.write_all(data.as_bytes())
            .map_err(|e| DocsError::Other(e.to_string()))?;
    }
    zip.finish().map_err(|e| DocsError::Other(e.to_string()))?;
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
