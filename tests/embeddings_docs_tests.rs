use wps_mcp_rust::docs::embeddings::{extract, list};

#[test]
fn list_and_extract_embedded_xlsx() {
    let list = list("tests/fixtures/host_with_embed.docx").unwrap();
    assert!(!list.embeddings.is_empty());
    let emb = &list.embeddings[0];
    assert!(emb.extractable);

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
}
