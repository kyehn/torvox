# 17 Autonomous-Agentic-Workflows/termux-app

上游：<https://github.com/Autonomous-Agentic-Workflows/termux-app>
本地：/tmp/competitor-study/Autonomous-Agentic-Workflows__termux-app
版本：主线 2026-09-18 取样，345 文件。旧基线 termux fork，核心被删减。`diff -rq` 全模块比对已做。

## 血缘与回退

相对上游：终端模拟器删图像类（`AndroidUtils/ITermImage/TerminalBitmap/TerminalSixel` 缺失），`TerminalBuffer/Row/Emulator/TextStyle` 有改动，`termux.c` 有改动，`TerminalRenderer/View` 有改动，应用层小幅改动。两处真实回退：渲染器删 Sixel 位图绘制分支（上游 `TerminalRenderer.java:105-124` 整段缺失）；`drawTextRun` 去 API23 版本守卫改无条件调用（上游 `259-263` 五Asia改一Asia），低版本崩溃风险。另有文档注释大面积剥离。

## 与本项目对照

旧基线加删减，无新增能力。Sixel 剥离与本项目 kitty 图像方向相反。无版本守卫属质量回退，反面教材。沿用 02 上游行为参考即可，本仓无新增可参考点。

## 可参考点

- 无。反面价值：基线 fork 删减图像与守卫即退化，论证跟 master 与保留上游能力的价值。

## 禁止项勿抄

删减后的模拟器与渲染器、任何回退改动。

证据：三方 `diff -rq` 结果、`TerminalRenderer.java:105-124,259-263` diff。
