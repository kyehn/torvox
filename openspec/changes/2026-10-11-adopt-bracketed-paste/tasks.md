# Tasks

## 1. 变更文档

- [ ] 1.1 写 proposal/specs delta 并 `openspec validate 2026-10-11-adopt-bracketed-paste --type change` 通过

## 2. native 侧粘贴编码查询

- [ ] 2.1 `commands.rs` 新增 `Query::EncodePaste`（数据、响应通道）；`internal.rs` 按 `mode_get(2004, 0)` 取 bracketed 标志后调 `paste::encode`
- [ ] 2.2 `paste::encode` 返回 `OutOfSpace { required }` 时按 `required` 精确扩容重试，不截断；`public_api.rs` 新增 `encode_paste`，`ffi.rs` 导出 `encodePaste`
- [ ] 2.3 cargo 测试：非 bracketed 下 `a` LF `b` 得 `a` CR `b`；bracketed 下得 `ESC[200~` 包裹且换行为 LF；NUL/ESC/DEL 中化为空格；`ESC[201~` 在 bracketed 下被中化；简体中文与 emoji 逐字节保留；缓冲区不足按 `required` 重试

## 3. Kotlin 侧删除自实现编码

- [ ] 3.1 `PasteChunks.kt` 删换行归一（`\r\n`→`\r`、`\n`→`r` 两条 replace），函数改为对给定字节流切块，不再解释内容
- [ ] 3.2 切块点仍落在多字节字符边界；截断到 `MAX_PASTE_CHARS` 的行为与告警保持不变
- [ ] 3.3 `executePaste` 改调 `encodePaste`，包裹序列只出现于首块之前与末块之后，不被切块拆开
- [ ] 3.4 `testDebugUnitTest` 与 `detekt` 通过

## 4. 规范对齐

- [ ] 4.1 `upstream-alignment` 的 `Terminal::paste` 条目从「待办」改为「不采用」并写明 `PROHIBITED` 依据；新增 `paste::encode` 采用与 `paste::is_safe` 不采用两行
- [ ] 4.2 `modifier-bar-sticky-encoding` 的 bracketed 缺口声明精确化：粘贴路径已实现，输入法提交路径不受影响
- [ ] 4.3 归档后核对 `openspec/specs/` 确实含新条目数
