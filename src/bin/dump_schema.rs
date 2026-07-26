use rmcp::schemars;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
pub struct XlsxWriteParams {
    pub output_path: String,
    pub sheet: String,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
pub struct CellUpdateParam {
    pub cell: String,
    pub value: Value,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
pub struct XlsxUpdateParams {
    pub path: String,
    pub sheet: String,
    pub cells: Vec<CellUpdateParam>,
    pub output_path: Option<String>,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
pub struct DocxReplaceParams {
    pub path: String,
    pub replacements: Vec<Replacement>,
    pub output_path: Option<String>,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
pub struct Replacement {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
pub struct Composite {
    pub rows: Vec<Vec<Value>>,
    pub cells: Vec<CellUpdateParam>,
    pub replacements: Vec<Replacement>,
    pub value: Value,
    pub extra: Option<serde_json::Value>,
}

fn main() {
    let schema = rmcp::schemars::schema_for!(Composite);
    let json = serde_json::to_string_pretty(&schema).unwrap();
    println!("{}", json);
}