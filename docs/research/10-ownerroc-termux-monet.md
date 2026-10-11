# 10 ownerroc/termux-monet

上游：<https://github.com/ownerroc/termux-monet>
本地：/tmp/competitor-study/ownerroc__termux-monet
版本：主线 2026-10-06 取样。与 sohan-f 同源的 termux 换皮 fork，经三方 `diff -rq` 比对：相对 sohan-f 拿掉 `MonetTerminalColors` 取色引擎与单测及模糊布局桩，保留 `TermuxBackgroundManager` 背景管理并改名换作者；终端核心仍沿用上游。全部关键 diff 已通读。

## 血缘

相对 sohan-f 的差异集中在换皮与构建：应用名仍为 `Termux:Monet`，背景管理器保留，Monet 取色引擎删除，工作流与文档改名，终端模拟器与视图层小幅改动。相对上游的差异为背景管理、样式设置、换皮资源与构建改动，PTY 与 VT 未换。

## 与本项目对照

结论与 03 基本相同且更弱：连 Monet 取色门限思想的载体都已删除，仅剩背景图管理，而背景图透明模糊正是本项目明确禁止项。故本仓无新增可参考点，仅作同源换皮记录在案，防止未来误引。

## 可参考点

- 无新增。沿用 02 上游行为参考与 03 门限测试思路。
- P2：同源 fork 换皮差异可作为“换皮不碰终端核心”的反面组织方式参考，不引入代码。

## 禁止项勿抄

背景图管理整条链、换皮改名、模糊布局、自定义样式开关。Monet 引擎既已缺失，更不得反向补回。

证据：`TermuxBackgroundManager.java:12.1K`、`TermuxTerminalSessionActivityClient.java:78-302`、`strings.xml:5`、三方 `diff -rq` 结果。
