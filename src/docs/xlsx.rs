use crate::docs::error::{DocsError, DocsResult};
use crate::docs::pathutil::{require_existing_file, resolve_output_path};
use calamine::{open_workbook, Data, Reader, Xlsx};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use umya_spreadsheet::{self, writer};

pub const DEFAULT_ROW_LIMIT: usize = 5000;

#[derive(Debug, Clone, Default)]
pub struct ReadSheetOptions {
    pub range: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ListSheetsResult {
    pub ok: bool,
    pub sheets: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReadSheetResult {
    pub ok: bool,
    pub rows: Vec<Vec<Value>>,
    pub truncated: bool,
    pub total_rows: usize,
}

#[derive(Debug, Serialize)]
pub struct CsvResult {
    pub ok: bool,
    pub csv: String,
    pub output_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct XlsxWriteResult {
    pub ok: bool,
    pub output_path: String,
}

#[derive(Debug, Clone)]
pub struct CellUpdate {
    pub cell: String,
    pub value: Value,
}

pub fn list_sheets(path: impl AsRef<Path>) -> DocsResult<ListSheetsResult> {
    let path = require_xlsx(path)?;
    let workbook: Xlsx<_> =
        open_workbook(&path).map_err(|e| DocsError::ParseError(format!("{e}")))?;
    Ok(ListSheetsResult {
        ok: true,
        sheets: workbook.sheet_names().to_vec(),
    })
}

pub fn read_sheet(
    path: impl AsRef<Path>,
    sheet: &str,
    opts: ReadSheetOptions,
) -> DocsResult<ReadSheetResult> {
    let path = require_xlsx(path)?;
    let mut workbook: Xlsx<_> =
        open_workbook(&path).map_err(|e| DocsError::ParseError(format!("{e}")))?;

    if let Some(r) = &opts.range {
        // v1: range 参数预留，当前读取整表后由调用方自行切片；非法标记仍校验非空
        if r.trim().is_empty() {
            return Err(DocsError::InvalidArgument("range 不能为空".into()));
        }
    }

    let range = workbook
        .worksheet_range(sheet)
        .map_err(|e| DocsError::ParseError(format!("读取工作表失败: {e}")))?;

    let all_rows: Vec<Vec<Value>> = range
        .rows()
        .map(|row| row.iter().map(data_to_value).collect())
        .collect();

    let total_rows = all_rows.len();
    let offset = opts.offset.unwrap_or(0);
    let limit = opts.limit.unwrap_or(DEFAULT_ROW_LIMIT);
    if offset > total_rows {
        return Err(DocsError::InvalidArgument(format!(
            "offset {offset} 超出总行数 {total_rows}"
        )));
    }
    let end = (offset + limit).min(total_rows);
    let rows = all_rows[offset..end].to_vec();
    let truncated = end < total_rows;

    Ok(ReadSheetResult {
        ok: true,
        rows,
        truncated,
        total_rows,
    })
}

pub fn to_csv(
    path: impl AsRef<Path>,
    sheet: &str,
    output_path: Option<&Path>,
) -> DocsResult<CsvResult> {
    let result = read_sheet(
        path,
        sheet,
        ReadSheetOptions {
            range: None,
            limit: None,
            offset: None,
        },
    )?;
    let mut csv = String::new();
    for row in &result.rows {
        let line = row
            .iter()
            .map(value_to_csv_field)
            .collect::<Vec<_>>()
            .join(",");
        csv.push_str(&line);
        csv.push('\n');
    }
    let mut out_path = None;
    if let Some(p) = output_path {
        std::fs::write(p, &csv)?;
        out_path = Some(p.display().to_string());
    }
    Ok(CsvResult {
        ok: true,
        csv,
        output_path: out_path,
    })
}

pub fn write(
    path: impl AsRef<Path>,
    sheet: &str,
    rows: &[Vec<Value>],
) -> DocsResult<XlsxWriteResult> {
    let out = resolve_output_path(None, Some(path.as_ref()), true)?;
    let mut book = umya_spreadsheet::new_file();
    {
        let ws = book
            .get_sheet_mut(&0)
            .ok_or_else(|| DocsError::Other("无法创建默认工作表".into()))?;
        ws.set_name(sheet);
        for (r_idx, row) in rows.iter().enumerate() {
            for (c_idx, cell) in row.iter().enumerate() {
                let addr = cell_address(r_idx + 1, c_idx + 1);
                set_cell_value(ws.get_cell_mut(addr.as_str()), cell);
            }
        }
    }
    writer::xlsx::write(&book, &out).map_err(|e| DocsError::Other(e.to_string()))?;
    Ok(XlsxWriteResult {
        ok: true,
        output_path: out.display().to_string(),
    })
}

pub fn update_cells(
    path: impl AsRef<Path>,
    sheet: &str,
    cells: &[CellUpdate],
    output_path: Option<&Path>,
) -> DocsResult<XlsxWriteResult> {
    let src = require_xlsx(path)?;
    let out = resolve_output_path(Some(&src), output_path, output_path.is_none())?;
    let mut book = umya_spreadsheet::reader::xlsx::read(&src)
        .map_err(|e| DocsError::ParseError(e.to_string()))?;
    {
        let ws = book
            .get_sheet_by_name_mut(sheet)
            .ok_or_else(|| DocsError::InvalidArgument(format!("工作表不存在: {sheet}")))?;
        for upd in cells {
            set_cell_value(ws.get_cell_mut(upd.cell.as_str()), &upd.value);
        }
    }
    writer::xlsx::write(&book, &out).map_err(|e| DocsError::Other(e.to_string()))?;
    Ok(XlsxWriteResult {
        ok: true,
        output_path: out.display().to_string(),
    })
}

fn require_xlsx(path: impl AsRef<Path>) -> DocsResult<std::path::PathBuf> {
    let path = require_existing_file(path)?;
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("xlsx"))
        != Some(true)
    {
        return Err(DocsError::UnsupportedFormat("仅支持 .xlsx".into()));
    }
    Ok(path)
}

fn data_to_value(d: &Data) -> Value {
    match d {
        Data::Empty => Value::Null,
        Data::String(s) => Value::String(s.clone()),
        Data::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::String(f.to_string())),
        Data::Int(i) => Value::Number((*i).into()),
        Data::Bool(b) => Value::Bool(*b),
        Data::DateTime(dt) => Value::String(dt.to_string()),
        Data::DateTimeIso(s) => Value::String(s.clone()),
        Data::DurationIso(s) => Value::String(s.clone()),
        Data::Error(e) => Value::String(format!("#ERROR:{e:?}")),
    }
}

fn value_to_csv_field(v: &Value) -> String {
    let s = match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

fn set_cell_value(cell: &mut umya_spreadsheet::structs::Cell, value: &Value) {
    match value {
        Value::Null => {
            cell.set_value("");
        }
        Value::Bool(b) => {
            cell.set_value_bool(*b);
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                cell.set_value_number(i as f64);
            } else if let Some(f) = n.as_f64() {
                cell.set_value_number(f);
            } else {
                cell.set_value(n.to_string());
            }
        }
        Value::String(s) => {
            cell.set_value(s);
        }
        other => {
            cell.set_value(other.to_string());
        }
    }
}

fn cell_address(row: usize, col: usize) -> String {
    format!("{}{}", col_to_letters(col), row)
}

fn col_to_letters(mut col: usize) -> String {
    let mut s = String::new();
    while col > 0 {
        let rem = (col - 1) % 26;
        s.insert(0, (b'A' + rem as u8) as char);
        col = (col - 1) / 26;
    }
    s
}
