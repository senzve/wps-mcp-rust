use crate::docs::error::{DocsError, DocsResult};
use crate::docs::pathutil::{paths_equal, require_existing_file, resolve_output_path};
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

    let sheet_range = workbook.worksheet_range(sheet).map_err(|e| {
        let msg = e.to_string();
        if msg.to_ascii_lowercase().contains("not found")
            || msg.contains("找不到")
            || msg.contains("Sheet")
        {
            DocsError::InvalidArgument(format!("工作表不存在: {sheet}"))
        } else {
            DocsError::ParseError(format!("读取工作表失败: {e}"))
        }
    })?;

    // calamine 使用 0-based 绝对坐标，(0,0)=A1
    let view = if let Some(r) = &opts.range {
        let (start, end) = parse_a1_range(r)?;
        sheet_range.range(start, end)
    } else {
        sheet_range
    };

    let total_rows = view.height();
    let offset = opts.offset.unwrap_or(0);
    // limit=None 表示不截断（供 to_csv 等内部全量读取）；工具层会注入默认 limit
    let limit = opts.limit.unwrap_or(usize::MAX);

    if offset > total_rows {
        return Err(DocsError::InvalidArgument(format!(
            "offset {offset} 超出总行数 {total_rows}"
        )));
    }

    let rows: Vec<Vec<Value>> = view
        .rows()
        .skip(offset)
        .take(limit)
        .map(|row| row.iter().map(data_to_value).collect())
        .collect();

    let end = offset.saturating_add(limit).min(total_rows);
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
            limit: None, // 全量导出，不受 xlsx_read 默认 5000 行限制
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
        let out = resolve_output_path(None, Some(p), false)?;
        std::fs::write(&out, &csv)?;
        out_path = Some(out.display().to_string());
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
    // 新建文件：目标已存在时拒绝覆盖（符合写操作默认不覆盖约定）
    let out = resolve_output_path(None, Some(path.as_ref()), false)?;
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
    // 省略 output_path 时写回源文件；指定新路径时默认不覆盖已存在目标
    let overwrite = output_path.map(|p| paths_equal(&src, p)).unwrap_or(true);
    let out = resolve_output_path(Some(&src), output_path, overwrite)?;
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

/// 解析 A1 风格区域：`A1`、`A1:B2`（大小写不敏感，允许空白）。
/// 返回 calamine 0-based 绝对坐标 `(row, col)`。
fn parse_a1_range(input: &str) -> DocsResult<((u32, u32), (u32, u32))> {
    let s = input.trim();
    if s.is_empty() {
        return Err(DocsError::InvalidArgument("range 不能为空".into()));
    }
    let (left, right) = match s.split_once(':') {
        Some((a, b)) => (a.trim(), b.trim()),
        None => (s, s),
    };
    if left.is_empty() || right.is_empty() {
        return Err(DocsError::InvalidArgument(format!(
            "非法 range 区域: {input}"
        )));
    }
    let start = parse_a1_cell(left)?;
    let end = parse_a1_cell(right)?;
    let (r1, c1) = start;
    let (r2, c2) = end;
    Ok(((r1.min(r2), c1.min(c2)), (r1.max(r2), c1.max(c2))))
}

fn parse_a1_cell(cell: &str) -> DocsResult<(u32, u32)> {
    let bytes = cell.as_bytes();
    if bytes.is_empty() {
        return Err(DocsError::InvalidArgument(format!(
            "非法单元格地址: {cell}"
        )));
    }
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == 0 || i == bytes.len() {
        return Err(DocsError::InvalidArgument(format!(
            "非法单元格地址: {cell}（期望如 A1）"
        )));
    }
    let col_part = &cell[..i];
    let row_part = &cell[i..];
    if !row_part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DocsError::InvalidArgument(format!(
            "非法单元格地址: {cell}"
        )));
    }
    let row_num: u32 = row_part
        .parse()
        .map_err(|_| DocsError::InvalidArgument(format!("非法行号: {row_part}")))?;
    if row_num == 0 {
        return Err(DocsError::InvalidArgument("行号必须从 1 开始".into()));
    }
    let col_num = letters_to_col(col_part)?;
    // 转为 0-based
    Ok((row_num - 1, col_num - 1))
}

fn letters_to_col(letters: &str) -> DocsResult<u32> {
    let mut col: u32 = 0;
    for ch in letters.chars() {
        let c = ch.to_ascii_uppercase();
        if !c.is_ascii_uppercase() {
            return Err(DocsError::InvalidArgument(format!("非法列标: {letters}")));
        }
        col = col
            .checked_mul(26)
            .and_then(|v| v.checked_add((c as u32) - ('A' as u32) + 1))
            .ok_or_else(|| DocsError::InvalidArgument(format!("列标过大: {letters}")))?;
    }
    if col == 0 {
        return Err(DocsError::InvalidArgument(format!("非法列标: {letters}")));
    }
    Ok(col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_range_a1_b2() {
        let (s, e) = parse_a1_range("A1:B2").unwrap();
        assert_eq!(s, (0, 0));
        assert_eq!(e, (1, 1));
    }

    #[test]
    fn parse_single_cell() {
        let (s, e) = parse_a1_range("C3").unwrap();
        assert_eq!(s, (2, 2));
        assert_eq!(e, (2, 2));
    }

    #[test]
    fn parse_invalid_range() {
        assert!(parse_a1_range("not-a-range").is_err());
        assert!(parse_a1_range("").is_err());
        assert!(parse_a1_range("A0").is_err());
    }
}
