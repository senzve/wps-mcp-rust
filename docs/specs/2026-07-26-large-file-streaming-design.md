# 大文件流式与增量读取优化设计文档

- **日期**：2026-07-26
- **设计目标**：解决项目在处理 50MB~500MB 大文件（文本、Excel、Docx）时的内存溢出（OOM）风险与性能瓶颈，实现 $O(1)$ 常数级内存占用与毫秒级增量读取响应。
- **状态**：已批准

---

## 1. 背景与现状分析

项目当前在读取和导出大文件时存在严重的全量装载内存问题：

1. **文本处理 (`src/docs/text.rs`)**：
   - `read_text` 和 `text_info` 均使用 `std::fs::read` 将整个文件读入内存数组，然后再进行 UTF-8/GBK 全量解码。
   - `read_text` 使用 `.split_inclusive('\n')` 将整文本切分为 `Vec<&str>` 内存数组后才执行 `offset`/`limit` 切片。读取 500MB 日志文件即使只要前 10 行也会导致几百兆内存分配。

2. **Excel 处理 (`src/docs/xlsx.rs`)**：
   - `read_sheet` 会将目标工作表中的所有单元格全量解析为 `Vec<Vec<Data>>` 并在内存中转为 `Vec<Vec<Value>>`。
   - `to_csv` 先在内存中构造全量 `Vec<Vec<Value>>` 再拼接为包含所有 CSV 内容的巨型 `String`，极大消耗内存。

3. **Word 处理 (`src/docs/docx.rs`)**：
   - `read_document_xml` 将 Zip 包内的 `word/document.xml` 一次性读取为内存字符串 `String`，再交由 XML Reader 解析。

---

## 2. 优化方案设计

### 2.1 整体架构与目标

- **内存控制**：内存开销与文件总体大小脱钩，在处理 50MB~500MB 大文件时物理内存峰值控制在 **< 20MB**（常数级 $O(1)$ 空间复杂度）。
- **读取延迟**：对大文件请求局部增量数据（如 `offset: 0, limit: 100`）时响应时间控制在 **< 10ms**。
- **接口兼容**：完全向下兼容现有的 MCP 工具参数与 JSON 结果格式。

---

### 2.2 模块详细设计

#### 2.2.1 文本文件流式处理 (`src/docs/text.rs`)

1. **流式解码与按行跳过**：
   - 使用 `File::open` 配合 `std::io::BufReader` 打开文件。
   - **预检编码**：读取头部最多 4KB 字节识别 UTF-8 / UTF-8 BOM / GBK / GB18030 编码，避免全量文件扫描。
   - **增量跳过 `offset`**：使用 `BufReader::read_until(b'\n', &mut buf)` 流式按行读取。在行数小于 `offset` 时，只累加 `line_count`，无需进行 UTF-8 字符串解码和保存。
   - **收集 `limit` 行**：当行数到达 `offset` 后开始逐行解码并追加至输出 `out: String`。当收集行数达到 `limit` 或输出字节数达到 `DEFAULT_BYTE_LIMIT` (10MB) 时，设置 `truncated = true` 并立即退出循环。
   - **单行安全上限**：单个 `read_until` 缓冲区设置 2MB 上限，防止超长无换行文件导致 OOM。

2. **高效行数统计与 `text_info`**：
   - 快速获取文件元数据 `metadata.len()`。
   - 编码识别仅分析头部 4KB。
   - `line_count` 统计采用流式分块扫描字节 `b'\n'`，无需将文件转为 `String`。

#### 2.2.2 Excel 文件流式处理 (`src/docs/xlsx.rs`)

1. **行迭代器增量读取 (`read_sheet`)**：
   - 结合 `calamine::Xlsx::worksheet_range` 获取区域迭代器。
   - 对行迭代器施加 `.skip(offset).take(limit)` 增量处理：
     - 被 `skip` 的行不触发 `Data -> Value` 的序列化转换。
     - 仅对属于 `[offset, offset + limit)` 区间的单元格执行 `data_to_value` 转换。
   - 达到 `limit` 后立刻中断迭代。

2. **流式 CSV 导出 (`to_csv`)**：
   - 当指定 `output_path` 时，直接创建 `std::io::BufWriter<File>`。
   - 遍历 `sheet` 行数据时，逐行格式化为 CSV 文本并立即写入 `BufWriter`，避免在内存中拼接整表 CSV 字符串。

#### 2.2.3 Docx 文件流式解析 (`src/docs/docx.rs`)

1. **Zip 解压流与 XML 解析直接绑定**：
   - 不再将 `word/document.xml` 先读取为 `String`。
   - 直接取得 Zip 文件条目 `zip.by_name("word/document.xml")` 句柄，通过 `quick_xml::Reader::from_reader(BufReader::new(entry))` 构造 Sax 流式解析器。
   - XML Event 循环直接从 Zip 压缩流中边解压边解析。

---

## 3. 错误处理与边界防御策略

1. **多字节字符截断保护**：
   - 文本截断达到 `DEFAULT_BYTE_LIMIT` 时，使用 `out.is_char_boundary(cut)` 确保截断点在合法 UTF-8 字符边界上。
2. **非法输入与超长单行保护**：
   - 设置单行 2MB 缓冲区硬上限，如果单行大小超过 2MB 仍未见换行符，强制截断该行并返回。
3. **输出流安全**：
   - `xlsx_to_csv` 未指定输出路径时，限制内存字符串最大长度，防止过度消耗内存和 stdio 管道带宽。

---

## 4. 验证与测试计划

1. **单元测试与边界测试**：
   - 扩充 `src/docs/text.rs` 测试用例，验证大行数流式 `offset`/`limit` 切片的准确性与 GBK/UTF-8 编解码正确性。
   - 扩充 `src/docs/xlsx.rs` 测试用例，验证 `to_csv` 流式写入文件与直接返回内存字符串的结果一致性。
   - 扩充 `src/docs/docx.rs` 测试用例，验证从 Zip 流直接解析 XML 与原字符串解析逻辑的等价性。
2. **性能与内存基准测试**：
   - 生成 50MB~100MB 规模的测试文件，测试 `read_text` 局部读取响应耗时小于 10ms，物理内存占用小于 20MB。
