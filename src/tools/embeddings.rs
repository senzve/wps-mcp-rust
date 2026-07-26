use crate::docs::embeddings;
use crate::tools::{err_result, json_ok};
use rmcp::model::CallToolResult;
use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListEmbeddingsParams {
    /// 宿主文档本地路径（.docx 或 .xlsx）。优先绝对路径，也可为相对路径。必填。
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExtractEmbeddingParams {
    /// 宿主文档本地路径（.docx 或 .xlsx）。绝对或相对路径。必填。
    pub path: String,
    /// 内嵌对象 id（与 doc_list_embeddings 返回一致）；与 name 至少提供一个。
    pub id: Option<String>,
    /// 内嵌对象名称；与 id 至少提供一个。
    pub name: Option<String>,
    /// 抽取后的完整输出文件路径（绝对或相对）。与 output_dir 二选一。
    pub output_path: Option<String>,
    /// 抽取输出目录；文件名由实现推导。与 output_path 二选一。
    pub output_dir: Option<String>,
}

pub fn doc_list_embeddings(params: ListEmbeddingsParams) -> CallToolResult {
    match embeddings::list(&params.path) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}

pub fn doc_extract_embedding(params: ExtractEmbeddingParams) -> CallToolResult {
    let out = params.output_path.as_deref().map(std::path::Path::new);
    let dir = params.output_dir.as_deref().map(std::path::Path::new);
    match embeddings::extract(
        &params.path,
        params.id.as_deref(),
        params.name.as_deref(),
        out,
        dir,
    ) {
        Ok(r) => json_ok(r),
        Err(e) => err_result(e.to_public_message()),
    }
}
