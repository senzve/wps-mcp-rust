use crate::docs::xlsx::{self, CellUpdate, ReadSheetOptions};
use crate::tools::{err_result, json_ok};
use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct XlsxPathParams {
    /// 本地 Excel 文件路径（.xlsx）。优先绝对路径，也可为相对当前工作目录的相对路径。例如 `/data/report.xlsx`。必填。不支持 .xls。
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct XlsxReadParams {
    /// 本地 Excel 文件路径（.xlsx）。优先绝对路径，也可为相对路径。必填。
    pub path: String,
    /// 工作表名称（与 xlsx_list_sheets 返回的名称一致）。必填。
    pub sheet: String,
    /// 可选单元格区域（预留参数；当前实现读取整表后再分页）。
    pub range: Option<String>,
    /// 最多返回的行数（分页）；省略则使用服务端默认上限。
    pub limit: Option<usize>,
    /// 跳过的起始行偏移（分页，从 0 起）。
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct XlsxCsvParams {
    /// 本地 Excel 文件路径（.xlsx）。绝对或相对路径。必填。
    pub path: String,
    /// 要导出的工作表名称。必填。
    pub sheet: String,
    /// 可选 CSV 输出路径；省略则在结果中返回 CSV 文本。
    pub output_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct XlsxWriteParams {
    /// 新建 .xlsx 的本地输出路径（绝对或相对）。必填。
    pub output_path: String,
    /// 工作表名称。必填。
    pub sheet: String,
    /// 二维行数据；每个内层数组为一行，单元格可为字符串/数字/布尔/null。
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CellUpdateParam {
    /// 单元格地址，如 `A1`、`B12`。
    pub cell: String,
    /// 写入值（字符串/数字/布尔/null 等 JSON 值）。
    pub value: Value,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct XlsxUpdateParams {
    /// 已有 .xlsx 本地路径（绝对或相对）。必填。
    pub path: String,
    /// 目标工作表名称。必填。
    pub sheet: String,
    /// 要更新的单元格列表。必填。
    pub cells: Vec<CellUpdateParam>,
    /// 可选输出路径；省略则按实现规则写回。
    pub output_path: Option<String>,
}

pub fn xlsx_list_sheets(params: XlsxPathParams) -> CallToolResult {
    match xlsx::list_sheets(&params.path) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn xlsx_read(params: XlsxReadParams) -> CallToolResult {
    match xlsx::read_sheet(
        &params.path,
        &params.sheet,
        ReadSheetOptions {
            range: params.range,
            limit: params.limit,
            offset: params.offset,
        },
    ) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn xlsx_to_csv(params: XlsxCsvParams) -> CallToolResult {
    let out = params.output_path.as_deref().map(std::path::Path::new);
    match xlsx::to_csv(&params.path, &params.sheet, out) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn xlsx_write(params: XlsxWriteParams) -> CallToolResult {
    match xlsx::write(&params.output_path, &params.sheet, &params.rows) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn xlsx_update_cells(params: XlsxUpdateParams) -> CallToolResult {
    let cells: Vec<CellUpdate> = params
        .cells
        .into_iter()
        .map(|c| CellUpdate {
            cell: c.cell,
            value: c.value,
        })
        .collect();
    let out = params.output_path.as_deref().map(std::path::Path::new);
    match xlsx::update_cells(&params.path, &params.sheet, &cells, out) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}
