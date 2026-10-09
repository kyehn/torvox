# 种子语料来源

`seeds/` 下的字节流取自 Ghostty 仓库 `test/fuzz-libghostty/corpus/` 的
`parser-initial` `stream-initial` `osc-initial` 三组手写种子，许可为 MIT
（© 2024 Mitchell Hashimoto，见 Ghostty 仓库根 `LICENSE`）。三组共 94 份。

三组种子分别对应 libghostty-vt 的三个 fuzz 入口：仅 VT 解析器、完整终端流、OSC 解析器。
本项目用同一个 `GhosttyTerminal` 驱动全部三组，因此解析器与 OSC 的入口差异在本项目侧合并，
按解析特性命名（如 `14-csi-sgr-256`、`26-osc-color`、`46-line-drawing`）仍可读。

导入时按「首字节是否为 harness 选择器」分别处理：

- `stream-initial` 与 `osc-initial` 的首字节是目标选择器，不是 VT 输入。`fuzz_stream.zig`
  取 `input[0]` 的奇偶在 `nextSlice` 与逐字节 `next` 之间切换，故该组首字节只有 `00` 与
  `01`；`fuzz_osc.zig` 取 `% 3` 选择 BEL / ST / 无终止符，故该组首字节只有 `00` `01` `02`。
  这两份语料去除了首字节：若保留，`stream-initial` 的 `00` 会成为终端的 NUL 控制字符，
  `osc-initial` 的终止符选择器会被当成 OSC 数字，语义都偏离上游意图。
- `parser-initial` 的首字节**不是**选择器——`fuzz_parser.zig` 全程不使用 `input[0]`，
  该组首字节是真实的 VT 输入（38 份以 `1b` 即 ESC 开头，是转义序列的引入符）。该组
  逐字节保持与上游一致，去掉首字节会让 `14-csi-sgr-256` 之类的种子失去 ESC，解析器根本
  收不到 CSI。

`-cmin` 语料（26MB）是 AFL++ minimizer 的中间产物，不导入。
