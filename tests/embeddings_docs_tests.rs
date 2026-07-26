use wps_mcp_rust::docs::embeddings::{extract, list};

#[test]
fn list_and_extract_embedded_xlsx() {
    let list = list("tests/fixtures/host_with_embed.docx").unwrap();
    assert!(!list.embeddings.is_empty());
    let emb = &list.embeddings[0];
    assert!(emb.extractable);
    assert_eq!(emb.kind, "xlsx");
    assert!(emb.name.to_ascii_lowercase().contains("xlsx") || emb.name.contains("Excel"));

    // 公共 JSON 契约字段：id/name/kind/extractable/reason（不含内部 zip_path）
    let json = serde_json::to_value(&list).unwrap();
    let first = &json["embeddings"][0];
    assert!(first.get("id").is_some());
    assert!(first.get("name").is_some());
    assert!(first.get("kind").is_some());
    assert!(first.get("extractable").is_some());
    assert!(
        first.get("zip_path").is_none(),
        "zip_path 应为内部字段，不对外序列化"
    );

    let dir = tempfile::tempdir().unwrap();
    let extracted = extract(
        "tests/fixtures/host_with_embed.docx",
        Some(&emb.id),
        None,
        None,
        Some(dir.path()),
    )
    .unwrap();
    assert!(std::path::Path::new(&extracted.output_path).exists());
    assert_eq!(extracted.kind, "xlsx");

    // 抽取结果可继续用 xlsx 工具处理
    let sheets = wps_mcp_rust::docs::xlsx::list_sheets(&extracted.output_path).unwrap();
    assert!(!sheets.sheets.is_empty());
}

#[test]
fn extract_requires_id_or_name() {
    let dir = tempfile::tempdir().unwrap();
    let err = extract(
        "tests/fixtures/host_with_embed.docx",
        None,
        None,
        None,
        Some(dir.path()),
    )
    .unwrap_err();
    assert!(err.to_string().contains("id") || err.to_string().contains("name"));
}
