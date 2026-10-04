# 任务：修复宽字符的单元格↔字符映射错位

- [x] 原生 `Query::WideCharTailCols` 变体 + `wide_char_tail_cols_impl` +
      `GhosttyTerminal::wide_char_tail_cols`
- [x] JNI 导出 `wideCharTailCols`
- [x] 原生单测：尾格列标出、纯窄字符行为空、软换行续行 `SpacerHead` 不计入、行文本列映射不变式
- [x] Kotlin 侧 `TerminalQueryPort` / `NativeQueryPort` / `NativeBridge` / `Bridge` 接线
- [x] `TerminalSurface`：吸附改走查询；长按先吸附再判空白
- [x] 行文本缓存换为列号数组缓存，删除 TTL；拖动会话标志的置位/清除保持在四条拆卸路径
- [x] 删除 `charIndexAtCellColumn`、`snapColToWideChar`、`util/TextWidth.kt` 及其单测
- [x] 修正把错误模型钉死的 Kotlin 单测
- [x] `cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、
      `testDebugUnitTest` 通过（余 1 项为台账 R32-T1 既有的并行偶发红）

## 验证记录

- 探针实测 `read_line_text`：`vt_write("中文AB")` → `"中 文 AB"`（6 字符 / 6 列），
  证实「字符下标 == 列号」；该探针已转为常驻回归测试。
- 新增原生测试 3 项全绿；`NativeBridgeSmokeTest` 8 项在构建 `jniLibs` 后全绿，
  即新 JNI 导出可被 JVM 解析。
