# 12 Sunbr99/TerminalOS

上游：<https://github.com/Sunbr99/TerminalOS>
本地：/tmp/competitor-study/Sunbr99__TerminalOS
版本：主线 2026-10-04 取样。仓库仅 LICENSE、README、示例脚本三文件，README 所列 Go 源码树（main.go、core、commands、library）在克隆中不存在，无代码可审。本报告仅记录声称。

## 声称要点

Go 自研 shell 解释器，面向 Termux 与桌面 Unix，bash 风格解析执行、内建命令、管道重定向、插件注册、安全层。非 Android 应用，无 Activity、无 PTY、无 VT、无渲染。

## 与本项目对照

无 Android 终端代码，无对照价值。自研 shell 与本项目启动真实 shell（依次找 bash、login、system sh）方向相反，不参考。安全模型声称与本项目低安全限制不设计权限管理的立场不兼容，不参考。

## 可参考点

- 无。仅记录在案。

## 禁止项勿抄

自研 shell 解释器、内建命令集、插件注册表。源码缺失，任何细节不得引用。

证据：仓库仅三文件，`README.md:154` 为全部依据。
