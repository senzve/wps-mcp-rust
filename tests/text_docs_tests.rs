use wps_mcp_rust::docs::text::{read_text, text_info, write_text, ReadTextOptions};

#[test]
fn read_utf8_text() {
    let r = read_text("tests/fixtures/sample_utf8.txt", ReadTextOptions::default()).unwrap();
    assert!(r.text.contains("你好"));
    assert_eq!(r.encoding, "utf-8");
    assert!(!r.truncated);
}

#[test]
fn read_gbk_fallback() {
    let r = read_text("tests/fixtures/sample_gbk.txt", ReadTextOptions::default()).unwrap();
    assert!(r.text.contains("中文") || r.text.contains("GBK"));
    assert!(r.encoding == "gbk" || r.encoding == "gb18030" || r.encoding.starts_with("gb"));
}

#[test]
fn read_with_limit_truncates() {
    let r = read_text(
        "tests/fixtures/sample_utf8.txt",
        ReadTextOptions {
            encoding: None,
            limit: Some(1),
            offset: Some(0),
        },
    )
    .unwrap();
    assert_eq!(r.total_lines, 3);
    assert!(r.truncated);
    assert_eq!(r.text.lines().count(), 1);
}

#[test]
fn write_and_read_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.txt");
    write_text(&path, "abc\n", None, true).unwrap();
    let r = read_text(&path, ReadTextOptions::default()).unwrap();
    assert_eq!(r.text, "abc\n");
    let info = text_info(&path).unwrap();
    assert!(info.line_count >= 1);
}

#[test]
fn missing_path_returns_not_found() {
    let err = read_text(
        "tests/fixtures/definitely-missing-xyz.txt",
        ReadTextOptions::default(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("不存在"));
}

#[test]
fn write_refuses_overwrite_without_flag() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("exists.txt");
    write_text(&path, "one", None, true).unwrap();
    let err = write_text(&path, "two", None, false).unwrap_err();
    assert!(err.to_string().contains("覆盖") || err.to_string().contains("已存在"));
}
