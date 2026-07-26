use crate::docs::text::{self, ReadTextOptions};
use crate::tools::{err_result, json_ok};
use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TextReadParams {
    /// 本地文本文件路径。优先绝对路径，也可为相对当前工作目录的相对路径。例如 `/home/user/notes.txt`。必填。
    pub path: String,
    /// 可选强制编码名（如 `utf-8`、`gbk`）。省略则自动探测 UTF-8/GBK/GB18030。
    pub encoding: Option<String>,
    /// 最多返回的行数（分页）；省略则使用服务端默认上限。
    pub limit: Option<usize>,
    /// 跳过的起始行偏移（分页，从 0 起）。
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TextWriteParams {
    /// 写入目标路径（与 output_path 二选一，优先 output_path）。绝对或相对本地路径。
    pub path: Option<String>,
    /// 写入目标路径（推荐）。绝对或相对本地路径。
    pub output_path: Option<String>,
    /// 要写入的完整文本内容。必填。
    pub content: String,
    /// 可选输出编码（默认 UTF-8）。
    pub encoding: Option<String>,
    /// 目标已存在时是否覆盖；默认 false。
    pub overwrite: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TextInfoParams {
    /// 本地文本文件路径（绝对或相对）。必填。
    pub path: String,
}

pub fn text_read(params: TextReadParams) -> CallToolResult {
    match text::read_text(
        &params.path,
        ReadTextOptions {
            encoding: params.encoding,
            limit: params.limit,
            offset: params.offset,
        },
    ) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn text_write(params: TextWriteParams) -> CallToolResult {
    let path = params.output_path.or(params.path).unwrap_or_default();
    if path.is_empty() {
        return err_result("必须提供 path 或 output_path");
    }
    match text::write_text(
        &path,
        &params.content,
        params.encoding.as_deref(),
        params.overwrite.unwrap_or(false),
    ) {
        Ok(output_path) => json_ok(serde_json::json!({ "ok": true, "output_path": output_path })),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn text_info(params: TextInfoParams) -> CallToolResult {
    match text::text_info(&params.path) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}
