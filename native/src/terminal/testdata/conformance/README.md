# 跨引擎语料来源

`conformance/` 下的 `.seq` 取自 Alacritty 仓库 `alacritty_terminal/tests/ref/` 的
`alacritty.recording`，许可为 Apache-2.0（© Joe Wilm 与贡献者，见仓库根
`LICENSE-APACHE`）。录音是 vim、tmux、htop、less 等程序在真实 PTY 上的输出，
字节流本身与 Alacritty 的实现无关，可直接作为终端输入。

`.json` 是这些录音在本引擎下的期望可见屏，来自 Alacritty 序列化的网格参考
（`grid.json`）。解析参考网格有两个要点：`raw.inner` 是**最新行在前**（bottom-up），
可见屏恒为末尾 `screen_lines` 行；未写入单元在上游被序列化为空格，而本项目快照把
同一单元表示为码位 0，故期望行按「空白单元渲染成空格」取值。

## 未纳入的录音

45 份录音全部跑通后，31 份屏幕文本完全一致，其余 14 份存在引擎差异，逐份记录如下。
差异都在 libghostty 与 Alacritty 的 VT 行为上，不是本项目的缺陷，因此不纳入语料：
用「已知失败」掩盖这些差异会让未知回归失去对照。

- `decaln_reset`：DECALN 填充后的行内容不同。
- `deccolm_reset`：DECCOLM（132/80 列切换）不被 libghostty 支持，网格列数不随请求改变。
- `erase_in_line`：EL 擦除范围不同。
- `saved_cursor`、`saved_cursor_alt`：DEC 特殊图形字符集中 `_` 应映射为空白，
  libghostty 保留 `_`。
- `scroll_in_region_up_preserves_history`：滚动区域内的历史行归属不同。
- `selective_erasure`：DECSED/DECSEL 选择性擦除行为不同。
- `tab_rendering`：制表符填充单元在上游存 `\t`，libghostty 存空格。
- `vttest_cursor_movement_1`、`vttest_insert`、`vttest_origin_mode_1`、
  `vttest_origin_mode_2`、`vttest_scroll`、`vttest_tab_clear_set`：录音内含 DECCOLM
  列宽切换，libghostty 不响应缩放；`vttest_tab_clear_set` 另有制表符填充差异。
