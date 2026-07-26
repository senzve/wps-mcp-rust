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
