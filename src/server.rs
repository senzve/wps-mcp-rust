use rmcp::{
    handler::server::router::tool::ToolRouter, handler::server::wrapper::Parameters, model::*,
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use serde::{Deserialize, Serialize};

use crate::tools;

#[derive(Clone)]
pub struct WpsMcpServer {
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PingParams {
    /// 可选探测消息；省略时返回 "pong"
    pub message: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct PingResult {
    pub ok: bool,
    pub message: String,
}

const SERVER_INSTRUCTIONS: &str = "\
本地文档处理 MCP（wps-mcp-rust）：解析/读取/分析/通读/总结/提取本地 Word(.docx)、Excel(.xlsx) 与文本文件。\
不控制 WPS 客户端，不调用云端 API。\n\n\
【何时必须优先调用本 MCP】\n\
- 用户要通读、解析、读取、分析、总结、整理、提取 Word/DOCX/XLSX/文本内容\n\
- 用户用 @ 引用了 .docx / .xlsx / .txt / .md / .csv 等本地文件并要求处理内容\n\
- 需要从文档生成功能矩阵、需求列表、表格摘要、Markdown 导出等\n\
不要猜测文档内容；先用工具读取再回答。\n\n\
【路径规范】\n\
- path / output_path 传本地文件系统路径：优先绝对路径（如 /home/user/docs/spec.docx），也可用相对当前工作目录的相对路径\n\
- 用户 @ 附件时，使用 Agent 解析出的真实本地路径，不要只传文件名（除非该文件就在 CWD）\n\
- 仅支持开放格式：.docx / .xlsx / 常见文本；不支持 .doc / .xls / .wps / 加密文档\n\n\
【工具选型】\n\
- 通读/分析 Word 正文 → docx_read_text 或 docx_to_markdown（结构化更好时用 markdown）\n\
- Word 表格 → docx_read_tables\n\
- Excel 通读/分析 → 先 xlsx_list_sheets，再 xlsx_read\n\
- 纯文本/日志/CSV 文本 → text_read / text_info\n\
- 内嵌 docx/xlsx 对象 → doc_list_embeddings → doc_extract_embedding\n\
";

#[tool_router]
impl WpsMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "ping",
        description = "健康检查 / 连通性探测。确认本 MCP 服务是否可用；与文档解析无关。"
    )]
    fn ping(&self, Parameters(params): Parameters<PingParams>) -> Result<CallToolResult, McpError> {
        let message = params.message.unwrap_or_else(|| "pong".into());
        let body = PingResult { ok: true, message };
        let text = serde_json::to_string(&body).unwrap_or_else(|_| r#"{"ok":true}"#.into());
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(
        name = "text_read",
        description = "读取/通读/解析本地文本文件内容（.txt/.md/.csv/.log/.json 等）。支持编码自动探测（UTF-8/GBK/GB18030）与 limit/offset 分页。适用于：分析、总结、提取文本；用户 @ 了文本文件并要求阅读时优先调用。path 为本地绝对或相对路径（必填）。"
    )]
    fn text_read(
        &self,
        Parameters(p): Parameters<tools::text::TextReadParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::text::text_read(p))
    }

    #[tool(
        name = "text_write",
        description = "写入/创建/覆盖本地文本文件。适用于：导出分析结果、保存总结、生成 Markdown/CSV 文本。需提供 path 或 output_path（本地绝对或相对路径）以及 content。"
    )]
    fn text_write(
        &self,
        Parameters(p): Parameters<tools::text::TextWriteParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::text::text_write(p))
    }

    #[tool(
        name = "text_info",
        description = "获取本地文本文件基本信息（路径、大小、编码探测等）。适用于：读取前探查文件；path 为本地绝对或相对路径（必填）。"
    )]
    fn text_info(
        &self,
        Parameters(p): Parameters<tools::text::TextInfoParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::text::text_info(p))
    }

    #[tool(
        name = "docx_read_text",
        description = "通读/解析/读取/提取 Word 文档(.docx)纯文本正文。适用于：分析文档、总结内容、整理功能矩阵/需求列表、通读说明书或方案。用户提到 Word、DOCX、.docx、通读文档、解析文档、分析文档、提取正文时优先调用。path 为本地 .docx 的绝对或相对路径（必填）。不支持 .doc/.wps。"
    )]
    fn docx_read_text(
        &self,
        Parameters(p): Parameters<tools::docx::DocxPathParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::docx::docx_read_text(p))
    }

    #[tool(
        name = "docx_read_tables",
        description = "提取/解析 Word 文档(.docx)中的表格数据。适用于：从 Word 表格整理功能矩阵、对照表、清单；分析/读取文档内表格。path 为本地 .docx 绝对或相对路径（必填）。"
    )]
    fn docx_read_tables(
        &self,
        Parameters(p): Parameters<tools::docx::DocxPathParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::docx::docx_read_tables(p))
    }

    #[tool(
        name = "docx_to_markdown",
        description = "将 Word(.docx) 解析并导出为 Markdown，便于通读、分析、总结与二次编辑。适用于：文档转 MD、结构化阅读、整理需求/功能说明。path 为本地 .docx 绝对或相对路径（必填）；可选 output_path 写到文件。"
    )]
    fn docx_to_markdown(
        &self,
        Parameters(p): Parameters<tools::docx::DocxToMarkdownParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::docx::docx_to_markdown(p))
    }

    #[tool(
        name = "docx_create",
        description = "从纯文本或 Markdown 创建/生成 Word 文档(.docx)。适用于：根据分析结果写回 Word、导出报告。output_path 为本地输出路径（必填）；format 为 text 或 markdown。"
    )]
    fn docx_create(
        &self,
        Parameters(p): Parameters<tools::docx::DocxCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::docx::docx_create(p))
    }

    #[tool(
        name = "docx_replace_text",
        description = "对 Word(.docx) 做简单全文查找替换。适用于：批量改词、模板填充。path 为源文件本地路径（必填）；replacements 为 from/to 列表。复杂跨 run 文本可能无法替换。"
    )]
    fn docx_replace_text(
        &self,
        Parameters(p): Parameters<tools::docx::DocxReplaceParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::docx::docx_replace_text(p))
    }

    #[tool(
        name = "xlsx_list_sheets",
        description = "列出 Excel(.xlsx) 工作簿中的工作表名称。通读/分析 Excel 前应先调用以确定 sheet。path 为本地 .xlsx 绝对或相对路径（必填）。不支持 .xls。"
    )]
    fn xlsx_list_sheets(
        &self,
        Parameters(p): Parameters<tools::xlsx::XlsxPathParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::xlsx::xlsx_list_sheets(p))
    }

    #[tool(
        name = "xlsx_read",
        description = "读取/通读/解析/分析 Excel 工作表(.xlsx)数据，支持 A1 区域（如 A1:C10）与 limit/offset 分页（默认 limit=5000）。适用于：提取表格、总结数据、整理功能矩阵/清单。path 与 sheet 必填（本地路径 + 工作表名）；可先 xlsx_list_sheets。"
    )]
    fn xlsx_read(
        &self,
        Parameters(p): Parameters<tools::xlsx::XlsxReadParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::xlsx::xlsx_read(p))
    }

    #[tool(
        name = "xlsx_to_csv",
        description = "将 Excel(.xlsx) 指定工作表导出为 CSV。适用于：转换格式、便于文本分析。path 与 sheet 必填；可选 output_path。"
    )]
    fn xlsx_to_csv(
        &self,
        Parameters(p): Parameters<tools::xlsx::XlsxCsvParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::xlsx::xlsx_to_csv(p))
    }

    #[tool(
        name = "xlsx_write",
        description = "创建新的 Excel(.xlsx) 工作表并写入行列数据。适用于：导出分析结果、生成矩阵表。output_path、sheet、rows 必填。"
    )]
    fn xlsx_write(
        &self,
        Parameters(p): Parameters<tools::xlsx::XlsxWriteParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::xlsx::xlsx_write(p))
    }

    #[tool(
        name = "xlsx_update_cells",
        description = "更新已有 Excel(.xlsx) 中指定单元格的值，尽量保留原有格式。path、sheet、cells 必填；cells 项含 cell 地址（如 A1）与 value。"
    )]
    fn xlsx_update_cells(
        &self,
        Parameters(p): Parameters<tools::xlsx::XlsxUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::xlsx::xlsx_update_cells(p))
    }

    #[tool(
        name = "doc_list_embeddings",
        description = "列出 Word/Excel 宿主文档中内嵌的 docx/xlsx 对象。适用于：文档含嵌入附件时先枚举再抽取。path 为本地宿主文件绝对或相对路径（必填）。"
    )]
    fn doc_list_embeddings(
        &self,
        Parameters(p): Parameters<tools::embeddings::ListEmbeddingsParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::embeddings::doc_list_embeddings(p))
    }

    #[tool(
        name = "doc_extract_embedding",
        description = "从宿主 Word/Excel 中抽取内嵌 docx/xlsx 到本地路径，便于后续通读/解析。需 path；并用 id 或 name 指定对象；提供 output_path 或 output_dir。"
    )]
    fn doc_extract_embedding(
        &self,
        Parameters(p): Parameters<tools::embeddings::ExtractEmbeddingParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(tools::embeddings::doc_extract_embedding(p))
    }
}

#[tool_handler]
impl ServerHandler for WpsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(SERVER_INSTRUCTIONS.into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

impl Default for WpsMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod description_tests {
    use super::*;

    #[test]
    fn tool_descriptions_include_intent_keywords() {
        let server = WpsMcpServer::new();
        let tools = server.tool_router.list_all();
        let docx = tools
            .iter()
            .find(|t| t.name == "docx_read_text")
            .expect("docx_read_text tool");
        let desc = docx.description.as_ref().expect("description");
        for kw in ["通读", "解析", "读取", "分析", "docx"] {
            assert!(
                desc.contains(kw),
                "description missing keyword: {kw}; got: {desc}"
            );
        }

        let schema = docx.input_schema.as_ref();
        let path = schema
            .get("properties")
            .and_then(|p| p.get("path"))
            .expect("path property");
        let path_desc = path
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");
        assert!(
            path_desc.contains("绝对"),
            "path schema should describe absolute path: {path_desc}"
        );
        let required = schema
            .get("required")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            required.iter().any(|v| v.as_str() == Some("path")),
            "path must be required in inputSchema"
        );
    }

    #[test]
    fn server_instructions_guide_document_analysis() {
        let info = WpsMcpServer::new().get_info();
        let ins = info.instructions.unwrap_or_default();
        for kw in ["通读", "解析", "docx", "path", "优先"] {
            assert!(ins.contains(kw), "instructions missing: {kw}");
        }
    }

    #[test]
    fn all_phase1_tools_are_registered() {
        let server = WpsMcpServer::new();
        let tools = server.tool_router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        let expected = [
            "ping",
            "text_read",
            "text_write",
            "text_info",
            "docx_read_text",
            "docx_read_tables",
            "docx_to_markdown",
            "docx_create",
            "docx_replace_text",
            "xlsx_list_sheets",
            "xlsx_read",
            "xlsx_to_csv",
            "xlsx_write",
            "xlsx_update_cells",
            "doc_list_embeddings",
            "doc_extract_embedding",
        ];
        for name in expected {
            assert!(
                names.contains(&name),
                "missing tool: {name}; have: {names:?}"
            );
        }
        assert_eq!(
            names.len(),
            expected.len(),
            "unexpected extra tools: {names:?}"
        );
    }
}
