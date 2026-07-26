use wps_mcp_rust::docs::docx::{create, read_tables, read_text, replace_text, to_markdown};

#[test]
fn docx_read_text_works() {
    let r = read_text("tests/fixtures/sample.docx").unwrap();
    assert!(r.text.contains("Hello Docx"));
    assert!(r.text.contains("第二段"));
}

#[test]
fn docx_read_tables_works() {
    let r = read_tables("tests/fixtures/sample.docx").unwrap();
    assert_eq!(r.tables.len(), 1);
    assert_eq!(r.tables[0][0][0], "A1");
}

#[test]
fn docx_to_markdown_works() {
    let r = to_markdown("tests/fixtures/sample.docx", None).unwrap();
    assert!(r.markdown.contains("Hello Docx"));
}

#[test]
fn docx_create_and_replace() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.docx");
    create(&path, "Hello FOO", "text").unwrap();
    let t = read_text(&path).unwrap();
    assert!(t.text.contains("Hello FOO"));

    let out = dir.path().join("replaced.docx");
    replace_text(&path, &[("FOO".into(), "BAR".into())], Some(&out)).unwrap();
    let t2 = read_text(&out).unwrap();
    assert!(t2.text.contains("Hello BAR"));
}

#[test]
fn docx_rejects_non_docx_extension() {
    let err = read_text("tests/fixtures/sample_utf8.txt").unwrap_err();
    assert!(err.to_string().contains("docx") || err.to_string().contains("不支持"));
}

#[test]
fn docx_to_markdown_can_write_file() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.md");
    let r = to_markdown("tests/fixtures/sample.docx", Some(&out)).unwrap();
    assert!(r.output_path.is_some());
    let content = std::fs::read_to_string(&out).unwrap();
    assert!(content.contains("Hello Docx"));
    assert_eq!(r.markdown, content);
}

#[test]
fn docx_create_refuses_overwrite_existing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("once.docx");
    create(&path, "first", "text").unwrap();
    let err = create(&path, "second", "text").unwrap_err();
    assert!(err.to_string().contains("覆盖") || err.to_string().contains("已存在"));
}

#[test]
fn test_read_docx_text_streaming() {
    let dir = tempfile::tempdir().unwrap();
    let docx_path = dir.path().join("test.docx");
    create(&docx_path, "Line 1\nLine 2", "text").unwrap();

    let res = read_text(&docx_path).unwrap();
    assert!(res.ok);
    assert!(res.text.contains("Line 1"));
    assert!(res.text.contains("Line 2"));
}

