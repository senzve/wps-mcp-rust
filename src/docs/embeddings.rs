use crate::docs::error::{DocsError, DocsResult};
use crate::docs::pathutil::{ensure_parent_dir, require_existing_file};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub extractable: bool,
    pub reason: Option<String>,
    pub zip_path: String,
}

#[derive(Debug, Serialize)]
pub struct ListEmbeddingsResult {
    pub ok: bool,
    pub embeddings: Vec<EmbeddingInfo>,
}

#[derive(Debug, Serialize)]
pub struct ExtractEmbeddingResult {
    pub ok: bool,
    pub output_path: String,
    pub kind: String,
}

pub fn list(path: impl AsRef<Path>) -> DocsResult<ListEmbeddingsResult> {
    let path = require_existing_file(path)?;
    let file = std::fs::File::open(&path)?;
    let mut zip = ZipArchive::new(file).map_err(|e| DocsError::ParseError(e.to_string()))?;

    let mut embeddings = Vec::new();
    let mut rel_map = load_relationship_names(&mut zip)?;

    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| DocsError::ParseError(e.to_string()))?;
        let name = entry.name().to_string();
        if !is_embedding_path(&name) {
            continue;
        }
        let file_name = Path::new(&name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&name)
            .to_string();
        let id = rel_map
            .remove(&normalize_target(&name))
            .unwrap_or_else(|| format!("emb_{}", embeddings.len() + 1));

        // Peek magic by reading later; classify by extension first
        let (kind, extractable, reason) = classify_by_name(&file_name);
        embeddings.push(EmbeddingInfo {
            id,
            name: file_name,
            kind,
            extractable,
            reason,
            zip_path: name,
        });
    }

    // Refine extractable using magic bytes
    for emb in &mut embeddings {
        if let Ok(mut entry) = zip.by_name(&emb.zip_path) {
            let mut header = [0u8; 8];
            let n = entry.read(&mut header).unwrap_or(0);
            let refined = classify_bytes(&header[..n], &emb.name);
            emb.kind = refined.0;
            emb.extractable = refined.1;
            emb.reason = refined.2;
        }
    }

    Ok(ListEmbeddingsResult {
        ok: true,
        embeddings,
    })
}

pub fn extract(
    path: impl AsRef<Path>,
    id: Option<&str>,
    name: Option<&str>,
    output_path: Option<&Path>,
    output_dir: Option<&Path>,
) -> DocsResult<ExtractEmbeddingResult> {
    let listed = list(&path)?;
    let emb = listed
        .embeddings
        .iter()
        .find(|e| {
            id.map(|i| e.id == i).unwrap_or(false) || name.map(|n| e.name == n).unwrap_or(false)
        })
        .ok_or_else(|| {
            DocsError::InvalidArgument("未找到匹配的内嵌对象（请提供 id 或 name）".into())
        })?
        .clone();

    if !emb.extractable {
        return Err(DocsError::EmbeddingNotExtractable(
            emb.reason
                .clone()
                .unwrap_or_else(|| "无法抽取为可用文件".into()),
        ));
    }

    let src = require_existing_file(path)?;
    let file = std::fs::File::open(&src)?;
    let mut zip = ZipArchive::new(file).map_err(|e| DocsError::ParseError(e.to_string()))?;
    let mut entry = zip
        .by_name(&emb.zip_path)
        .map_err(|e| DocsError::ParseError(e.to_string()))?;
    let mut data = Vec::new();
    entry.read_to_end(&mut data)?;

    let payload = if is_ole(&data) {
        extract_ooxml_from_ole(&data).ok_or_else(|| {
            DocsError::EmbeddingNotExtractable("OLE 包装中未找到 docx/xlsx 载荷".into())
        })?
    } else if is_zip_ooxml(&data) {
        data
    } else {
        return Err(DocsError::EmbeddingNotExtractable(
            "内容既不是 OOXML 也不是可解 OLE".into(),
        ));
    };

    let out = resolve_extract_path(&emb, output_path, output_dir)?;
    ensure_parent_dir(&out)?;
    std::fs::write(&out, payload)?;

    Ok(ExtractEmbeddingResult {
        ok: true,
        output_path: out.display().to_string(),
        kind: emb.kind,
    })
}

fn resolve_extract_path(
    emb: &EmbeddingInfo,
    output_path: Option<&Path>,
    output_dir: Option<&Path>,
) -> DocsResult<PathBuf> {
    if let Some(p) = output_path {
        return Ok(p.to_path_buf());
    }
    if let Some(dir) = output_dir {
        return Ok(dir.join(&emb.name));
    }
    Err(DocsError::InvalidArgument(
        "必须提供 output_path 或 output_dir".into(),
    ))
}

fn is_embedding_path(name: &str) -> bool {
    let n = name.replace('\\', "/");
    n.contains("/embeddings/")
        || n.starts_with("word/embeddings/")
        || n.starts_with("xl/embeddings/")
}

fn normalize_target(zip_path: &str) -> String {
    // relationship Target is often relative like embeddings/foo.xlsx under word/
    let n = zip_path.replace('\\', "/");
    if let Some(idx) = n.find("embeddings/") {
        return n[idx..].to_string();
    }
    n
}

fn load_relationship_names(
    zip: &mut ZipArchive<std::fs::File>,
) -> DocsResult<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    for rels_name in ["word/_rels/document.xml.rels", "xl/_rels/workbook.xml.rels"] {
        let Ok(mut entry) = zip.by_name(rels_name) else {
            continue;
        };
        let mut xml = String::new();
        entry.read_to_string(&mut xml)?;
        // very small parse: Relationship Id= Target=
        for part in xml.split("<Relationship") {
            if !part.contains("Id=") || !part.contains("Target=") {
                continue;
            }
            let id = attr_value(part, "Id").unwrap_or_default();
            let target = attr_value(part, "Target").unwrap_or_default();
            if target.contains("embeddings/") {
                let t = target.trim_start_matches("./").to_string();
                map.insert(t, id);
            }
        }
    }
    Ok(map)
}

fn attr_value(s: &str, key: &str) -> Option<String> {
    let pat = format!("{key}=\"");
    let start = s.find(&pat)? + pat.len();
    let end = s[start..].find('"')? + start;
    Some(s[start..end].to_string())
}

fn classify_by_name(name: &str) -> (String, bool, Option<String>) {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".docx") {
        ("docx".into(), true, None)
    } else if lower.ends_with(".xlsx") {
        ("xlsx".into(), true, None)
    } else if lower.ends_with(".bin") {
        ("ole".into(), true, Some("可能是 OLE，将尝试解包".into()))
    } else {
        ("unknown".into(), false, Some("未知扩展名".into()))
    }
}

fn classify_bytes(header: &[u8], name: &str) -> (String, bool, Option<String>) {
    if is_zip_ooxml(header) {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".docx") {
            return ("docx".into(), true, None);
        }
        if lower.ends_with(".xlsx") {
            return ("xlsx".into(), true, None);
        }
        return ("ooxml".into(), true, None);
    }
    if is_ole(header) {
        return (
            "ole".into(),
            true,
            Some("OLE 包装，抽取时将尝试提取 OOXML".into()),
        );
    }
    classify_by_name(name)
}

fn is_zip_ooxml(data: &[u8]) -> bool {
    data.len() >= 4 && data[0] == 0x50 && data[1] == 0x4B && data[2] == 0x03 && data[3] == 0x04
}

fn is_ole(data: &[u8]) -> bool {
    data.len() >= 8
        && data[0] == 0xD0
        && data[1] == 0xCF
        && data[2] == 0x11
        && data[3] == 0xE0
        && data[4] == 0xA1
        && data[5] == 0xB1
        && data[6] == 0x1A
        && data[7] == 0xE1
}

fn extract_ooxml_from_ole(data: &[u8]) -> Option<Vec<u8>> {
    // Best-effort: open CFB and scan streams for ZIP magic; also scan raw bytes.
    if let Ok(mut comp) = cfb::CompoundFile::open(std::io::Cursor::new(data)) {
        let paths: Vec<_> = comp.walk().map(|e| e.path().to_path_buf()).collect();
        for p in paths {
            if let Ok(mut stream) = comp.open_stream(&p) {
                let mut buf = Vec::new();
                if stream.read_to_end(&mut buf).is_ok() {
                    if let Some(payload) = find_zip_payload(&buf) {
                        return Some(payload);
                    }
                }
            }
        }
    }
    find_zip_payload(data)
}

fn find_zip_payload(data: &[u8]) -> Option<Vec<u8>> {
    data.windows(4)
        .position(|w| w == [0x50, 0x4B, 0x03, 0x04])
        .map(|idx| data[idx..].to_vec())
}

// keep Write in scope for potential future streaming
#[allow(dead_code)]
fn _w(w: &mut dyn Write) {
    let _ = w;
}
