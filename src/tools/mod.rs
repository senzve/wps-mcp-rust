pub mod docx;
pub mod embeddings;
pub mod text;
pub mod xlsx;

use rmcp::model::{CallToolResult, Content};
use serde::Serialize;

pub fn json_ok<T: Serialize>(value: T) -> CallToolResult {
    match serde_json::to_string_pretty(&value) {
        Ok(s) => CallToolResult::success(vec![Content::text(s)]),
        Err(e) => CallToolResult::error(vec![Content::text(format!("序列化失败: {e}"))]),
    }
}

pub fn err_result(msg: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![Content::text(msg.into())])
}
