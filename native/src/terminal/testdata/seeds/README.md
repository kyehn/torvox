# 种子语料来源

`seeds/` 下的字节流取自 Ghostty 仓库 `test/fuzz-libghostty/corpus/` 的
`parser-initial` `stream-initial` `osc-initial` 三组手写种子，许可为 MIT
（© 2024 Mitchell Hashimoto，见 Ghostty 仓库根 `LICENSE`）。

三组种子分别对应 libghostty-vt 的三个 fuzz 入口：仅 VT 解析器、完整终端流、OSC 解析器。
本项目用同一个 `GhosttyTerminal` 驱动全部三组，因此解析器与 OSC 的入口差异在本项目侧合并，
按解析特性命名（如 `13-csi-sgr-256`、`26-osc-color`、`46-line-drawing`）仍可读。

各 fuzz 入口会用输入的**首字节**选择入口内部路径：`fuzz_stream.zig` 取奇偶在
`nextSlice` 与逐字节 `next` 之间切换，`fuzz_osc.zig` 取 `% 3` 选择 BEL / ST / 无终止符。
该字节属于上游 harness，不属于终端输入，故导入时已逐文件去除。若保留，`stream-initial`
中的 `00` 会成为终端的 NUL 控制字符，`osc-initial` 中的终止符选择器会被当成 OSC 数字，
两份语料的语义都会偏离上游意图。

`-cmin` 语料（26MB）是 AFL++ minimizer 的中间产物，不导入。
