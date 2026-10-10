# xterm 转义序列一致性语料

## 这是什么

`vttest`（Thomas Dickey）→ `vt100-parser`（Mark Lodato）→ `xterm.js` 逐代继承的
VT 一致性语料，本仓用它断言自己的 VT 行为。

- `tNNNN-名称.in`：喂给终端的字节流
- `tNNNN-名称.text`：**从 xterm 逐帧手抄**的 80×25 期望屏幕文本

## 为什么是外部真相

期望值不是任何一方的实现输出，而是从另一个独立实现（xterm）人工抄录的屏幕状态，
血统一直追到 DEC STD 070 与 ECMA-48 描述的 VT 语义。用它校验本仓属于**外部验证**，
而用它校验 xterm.js 自己才是自验证——两者方向相反，本仓只取前者。

## 血统一致性依赖 `^` 标记

`t*.text` 中 `^` 是 vttest 约定的**光标位置标记**，不是屏幕字符：光标所在格在屏幕
上本就是空白，标记只用来断言光标停在哪。少数用例同时又打印了字面 `^`，两种含义必须
逐例区分——本仓按「先剥离 `^`、再把剥离位置当作期望光标」处理，见
`native/tests/vt_conformance.rs`。

## 来源与许可

- 上游：<https://github.com/xtermjs/xterm.js>，提交 `c58ea3637f3968e0e6e79cd92cf9aace7ef89ee2`
- 目录：`test/fixtures/escape_sequence_files/`
- 许可：MIT（见 `LICENSE`），版权归 xterm.js 作者、SourceLair Private Company 与
  Christopher Jeffrey

**本目录是原样拷贝，未做任何修改。** 更新语料时必须同步复核
`native/tests/vt_conformance.rs` 里的判定表：新用例若既不在断言表也不在
「已判定不采纳」表里，用例计数断言会直接失败，逼使逐例给出结论而不是静默放过。

## 采纳范围

不是 76 例全部采纳。TESTING 与 `openspec/specs/upstream-alignment` 要求按「输入是否
格式合规」分流：参数个数不合规的序列（如 `CSI 2;3D`）标准未规定，快照记的是 xterm
的历史行为，采纳即等于让本仓与之对齐；快照成文时尚未实现的序列（REP、reverse-wrap）
同理。判定与理由逐例写在测试文件的表里。
