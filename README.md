# wps-mcp-rust

本地 **stdio MCP** 服务：用纯 Rust 处理 `.docx` / `.xlsx` / 文本文件，并支持列出与抽取宿主文档中的内嵌 docx/xlsx。

> 名称含 WPS，实现策略是本地开放格式处理，**不**控制 WPS 客户端，**不**调用云端 API。

## 功能概览

| 类别 | 工具 |
|------|------|
| 文本 | `text_read`, `text_write`, `text_info` |
| Word | `docx_read_text`, `docx_read_tables`, `docx_to_markdown`, `docx_create`, `docx_replace_text` |
| Excel | `xlsx_list_sheets`, `xlsx_read`, `xlsx_to_csv`, `xlsx_write`, `xlsx_update_cells` |
| 内嵌 | `doc_list_embeddings`, `doc_extract_embedding` |
| 其它 | `ping` |

大结果读取支持 `limit` / `offset`（默认约 5000 行），返回 `truncated` 元数据。文本编码自动尝试 UTF-8 / GBK / GB18030，失败时 lossy 解码。

工具 `description` / `inputSchema` 与 server `instructions` 已面向 Agent 意图优化：包含「通读 / 解析 / 读取 / 分析 / 总结 / 提取」等触发词，并明确 `path` 需传本地绝对或相对路径。

## 构建

```bash
cargo build --release
```

二进制：`target/release/wps-mcp-rust`

## 接入 Cursor / Claude Desktop

```json
{
  "mcpServers": {
    "wps-mcp-rust": {
      "command": "/absolute/path/to/wps-mcp-rust/target/release/wps-mcp-rust",
      "args": []
    }
  }
}
```

日志默认走 **stderr**（`RUST_LOG=info`），避免污染 MCP 的 stdout 协议流。

## 开发

```bash
cargo test
cargo fmt
cargo clippy
```

可用 `cargo run --example make_fixtures` 重新生成 `tests/fixtures/sample.xlsx`。

## 限制（第一期）

- 不支持 PPT、加密文档、`.doc` / `.xls`
- 不做完整样式保真
- 内嵌对象只支持列出与抽取，**不写回**宿主
- `docx_replace_text` 在 run 被拆分的复杂文档上可能无法替换跨 run 文本
- `xlsx_read` 的 `range` 参数预留，当前读取整表后再分页

## License

MIT
