# Gemini MCP 工具 Schema 兼容性修复

## 问题现象

连接当前项目的 MCP 服务后，使用 Gemini 系列模型（gemini-2.0-flash 等）调用工具时返回 400 错误，模型不会调用任何 MCP 工具：

```
Error: Retry failed after 1 attempts: 400
{"error":{"type":"400","message":"Invalid value at '.tools[0].function_declarations[5]..properties[1]...items' (..com/....Schema), true","type":"error"}}
```

## 根因

### 1. schemars 为 `serde_json::Value` 生成 boolean `true`

`schemars` 1.x 对 `serde_json::Value` 的 `JsonSchema` 实现输出 `Schema::Bool(true)`：

```rust
// schemars-1.2.1/src/json_schema_impls/serdejson.rs
impl JsonSchema for Value {
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        true.into()  // → 序列化为 JSON 值 true
    }
}
```

这导致包含 `Value` 的参数结构体生成不符合预期的 schema：

```json
{
  "rows": {
    "type": "array",
    "items": {
      "type": "array",
      "items": true       ← boolean，不是 schema object
    }
  },
  "value": true,          ← boolean，不是 schema object
  "extra": true           ← boolean，Option<Value> 也是 true
}
```

### 2. Google AI SDK 的 `Schema` 接口只接受 object 形式

`@google/genai` SDK 的 `Schema` 类型定义（`dist/genai.d.ts`）：

```typescript
export declare interface Schema {
    anyOf?: Schema[];
    description?: string;
    enum?: string[];
    format?: string;
    items?: Schema;                      // ← 必须是 Schema 对象，不是 boolean
    properties?: Record<string, Schema>; // ← 每个属性值必须是 Schema 对象，不是 boolean
    required?: string[];
    type?: Type;
    // ... 没有 additionalProperties 字段
}
```

`Schema` 是一个**严格的对象接口**，不是 `Schema | boolean` 联合类型。所以：
- `items: true` 不合法（`items` 必须是 `Schema` 对象）
- `"value": true` 不合法（属性值必须是 `Schema` 对象）
- `additionalProperties` 完全不支持

### 3. 错误消息本身也指向了这一点

```
Invalid value at '...items' (..com/....Schema), true
```

- `(..com/....Schema)` = Google 内部校验规则，期望 `Schema` 对象
- `true` = 实际传入的 boolean 值

### 总结

| 层级 | 问题 |
|------|------|
| schemars 生成 | `serde_json::Value` → `Schema::Bool(true)` → JSON `true` |
| Google API 期望 | `items`/`properties` 值必须是 `Schema` 对象 |
| 冲突 | 传入 `true` (boolean) 但期望 `Schema` (object) |
| 影响 | `Vec<Vec<Value>>` → `items: true`；`Value` → `true` |

## 受影响的字段

| 结构体 | 字段 | 产生无效 schema |
|--------|------|-----------------|
| `XlsxWriteParams` | `rows: Vec<Vec<Value>>` | 内层 items = `true` |
| `CellUpdateParam` | `value: Value` | 属性值 = `true` |
| `XlsxUpdateParams` | 通过 `cells: Vec<CellUpdateParam>` 间接包含 | 同上 |

## 修复方案

创建一个包装类型 `JsonValue`，用自定义 `JsonSchema` 实现返回 `{}`（空 schema object）而非 `true`。

### 步骤 1：在 `src/tools/mod.rs` 中添加 `JsonValue` 类型

```rust
use rmcp::schemars;
use rmcp::schemars::schema::{Schema, SchemaObject};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 包装 `serde_json::Value`，提供 Gemini 兼容的 JSON Schema 输出。
///
/// schemars 1.x 对 `serde_json::Value` 生成 `Schema::Bool(true)`（即 JSON `true`），
/// 但 Google AI SDK 的 `Schema` 接口只接受 object 形式，不接受 boolean。
/// 此包装类型改为输出 `{}`（空 schema object），表示"任意类型"。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonValue(pub Value);

impl schemars::JsonSchema for JsonValue {
    fn schema_name() -> String {
        "JsonValue".into()
    }
    fn json_schema(_: &mut schemars::gen::SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject::default())  // 输出 {} 而非 true
    }
}
```

### 步骤 2：在 `src/tools/xlsx.rs` 中替换 `Value` 为 `JsonValue`

将参数结构体中的 `Value` 替换为 `JsonValue`：

```rust
// 修改前
pub rows: Vec<Vec<Value>>,
pub value: Value,

// 修改后
pub rows: Vec<Vec<JsonValue>>,
pub value: JsonValue,
```

### 步骤 3：在工具函数中解包 `JsonValue` 为 `Value`

```rust
// xlsx_write: 传递 rows 时解包
match xlsx::write(&params.output_path, &params.sheet, &params.rows) {
```
改为：
```rust
let rows: Vec<Vec<Value>> = params.rows
    .into_iter()
    .map(|row| row.into_iter().map(|v| v.0).collect())
    .collect();
match xlsx::write(&params.output_path, &params.sheet, &rows) {
```

```rust
// xlsx_update_cells: 解包 value
.map(|c| CellUpdate {
    cell: c.cell,
    value: c.value,  // 改为 c.value.0
})
```

### 注意事项

- `docs/xlsx.rs` 层**不需要修改**，它继续使用 `serde_json::Value` 内部处理
- 只有 `src/tools/xlsx.rs` 的参数结构体层需要修改
- 修改后需 `cargo test` 确保所有测试通过
- 修改后 dump schema 验证：`"value": true` 应变为 `"value": {}`