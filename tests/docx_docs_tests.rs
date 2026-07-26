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
