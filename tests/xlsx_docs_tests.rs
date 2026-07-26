use wps_mcp_rust::docs::xlsx::{
    list_sheets, read_sheet, to_csv, update_cells, write, CellUpdate, ReadSheetOptions,
};

#[test]
fn list_sheets_works() {
    let r = list_sheets("tests/fixtures/sample.xlsx").unwrap();
    assert!(r.sheets.iter().any(|s| s == "Sheet1"));
}

#[test]
fn read_sheet_with_limit() {
    let r = read_sheet(
        "tests/fixtures/sample.xlsx",
        "Sheet1",
        ReadSheetOptions {
            range: None,
            limit: Some(2),
            offset: Some(0),
        },
    )
    .unwrap();
    assert_eq!(r.rows.len(), 2);
    assert!(r.truncated);
    assert!(r.total_rows >= 3);
}

#[test]
fn xlsx_to_csv_and_write_update() {
    let csv = to_csv("tests/fixtures/sample.xlsx", "Sheet1", None).unwrap();
    assert!(csv.csv.contains("Alice"));

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("w.xlsx");
    write(
        &path,
        "Data",
        &[
            vec![serde_json::json!("H1"), serde_json::json!("H2")],
            vec![serde_json::json!(1), serde_json::json!(2)],
        ],
    )
    .unwrap();

    let out = dir.path().join("u.xlsx");
    update_cells(
        &path,
        "Data",
        &[CellUpdate {
            cell: "A2".into(),
            value: serde_json::json!("changed"),
        }],
        Some(&out),
    )
    .unwrap();
    let r = read_sheet(&out, "Data", ReadSheetOptions::default()).unwrap();
    assert_eq!(r.rows[1][0], serde_json::json!("changed"));
}

#[test]
fn read_sheet_range_a1_style() {
    let r = read_sheet(
        "tests/fixtures/sample.xlsx",
        "Sheet1",
        ReadSheetOptions {
            range: Some("A1:A2".into()),
            limit: None,
            offset: None,
        },
    )
    .unwrap();
    assert_eq!(r.total_rows, 2);
    assert_eq!(r.rows.len(), 2);
    assert_eq!(r.rows[0].len(), 1);
    assert_eq!(r.rows[0][0], serde_json::json!("Name"));
    assert_eq!(r.rows[1][0], serde_json::json!("Alice"));
    assert!(!r.truncated);
}

#[test]
fn read_sheet_invalid_range_errors() {
    let err = read_sheet(
        "tests/fixtures/sample.xlsx",
        "Sheet1",
        ReadSheetOptions {
            range: Some("not-a-range".into()),
            limit: None,
            offset: None,
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("range") || err.to_string().contains("区域"));
}

#[test]
fn read_sheet_missing_sheet_errors() {
    let err = read_sheet(
        "tests/fixtures/sample.xlsx",
        "NoSuchSheet",
        ReadSheetOptions::default(),
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("工作表") || msg.contains("NoSuchSheet"));
}

#[test]
fn to_csv_exports_all_rows_without_default_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.xlsx");
    // 构造 > DEFAULT_ROW_LIMIT 的表，验证 CSV 不被 5000 行默认截断
    let mut rows = vec![vec![serde_json::json!("idx")]];
    for i in 0..5200 {
        rows.push(vec![serde_json::json!(i)]);
    }
    write(&path, "Big", &rows).unwrap();
    let csv = to_csv(&path, "Big", None).unwrap();
    let line_count = csv.csv.lines().count();
    assert_eq!(
        line_count, 5201,
        "csv should include header + 5200 data rows"
    );
}

#[test]
fn xlsx_write_refuses_overwrite_existing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("once.xlsx");
    write(&path, "S", &[vec![serde_json::json!("a")]]).unwrap();
    let err = write(&path, "S", &[vec![serde_json::json!("b")]]).unwrap_err();
    assert!(err.to_string().contains("覆盖") || err.to_string().contains("已存在"));
}
