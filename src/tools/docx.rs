use crate::docs::docx;
use crate::tools::{err_result, json_ok};
use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DocxPathParams {
    /// 本地 Word 文档路径（.docx）。优先绝对路径，也可为相对当前工作目录的相对路径。例如 `/home/user/docs/spec.docx` 或 `./docs/spec.docx`。必填。不支持 .doc / .wps。
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DocxToMarkdownParams {
    /// 本地 Word 文档路径（.docx）。优先绝对路径，也可为相对路径。必填。
    pub path: String,
    /// 可选：Markdown 输出文件路径（绝对或相对）。省略则只在工具结果中返回内容。
    pub output_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DocxCreateParams {
    /// 新建 .docx 的本地输出路径（绝对或相对）。必填。
    pub output_path: String,
    /// 要写入文档的正文内容。
    pub content: String,
    /// 内容格式：`text`（默认）或 `markdown`。
    pub format: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct Replacement {
    /// 查找原文（精确子串匹配）。
    pub from: String,
    /// 替换为的目标文本。
    pub to: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DocxReplaceParams {
    /// 源 .docx 本地路径（绝对或相对）。必填。
    pub path: String,
    /// 替换规则列表，每项含 from / to。
    pub replacements: Vec<Replacement>,
    /// 可选输出路径；省略则覆盖源文件（实现以 resolve 规则为准）。
    pub output_path: Option<String>,
}

pub fn docx_read_text(params: DocxPathParams) -> CallToolResult {
    match docx::read_text(&params.path) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn docx_read_tables(params: DocxPathParams) -> CallToolResult {
    match docx::read_tables(&params.path) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn docx_to_markdown(params: DocxToMarkdownParams) -> CallToolResult {
    let out = params.output_path.as_deref().map(std::path::Path::new);
    match docx::to_markdown(&params.path, out) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn docx_create(params: DocxCreateParams) -> CallToolResult {
    let format = params.format.as_deref().unwrap_or("text");
    match docx::create(&params.output_path, &params.content, format) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn docx_replace_text(params: DocxReplaceParams) -> CallToolResult {
    let reps: Vec<(String, String)> = params
        .replacements
        .into_iter()
        .map(|r| (r.from, r.to))
        .collect();
    let out = params.output_path.as_deref().map(std::path::Path::new);
    match docx::replace_text(&params.path, &reps, out) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}
