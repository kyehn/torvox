# 05 deathlegionteamlk/jolz

上游：<https://github.com/deathlegionteamlk/jolz>
本地：/tmp/competitor-study/deathlegionteamlk__jolz
版本：v1.3.0（2026-10-09）。仓库仅含 README 与 Release APK，源码私有，无代码可审。本报告仅记录 README 声称，不做代码级结论。

## 声称要点

纯 Java 原生 VT/ANSI 模拟器，自带 `pkg` 包管理器与 9 类插件，11 配色，24 内建命令，8 Activity，主屏小部件、文件管理器、开机脚本、前台服务、logcat 崩溃报告。支持 API21 起全 ABI，v1/v2/v3 签名。v1.3.0 修黑屏与崩溃：shell 继承系统环境而非清空、布局固定 56dp、简化 ShellEnvironment。

## 与本项目对照

无源码故无架构对照价值。两处声称细节与本项目规范方向一致但须以本项目规范为准：shell 环境继承宿主存在变量而非硬编码（本项目仅宿主存在时透传固定名单）；崩溃收集走 logcat 可见（本项目日志须在 logcat 可见、不写文件）。

## 可参考点

- P2：继承宿主环境而非清空的思路（本项目已由透传名单覆盖）；logcat 崩溃报告（本项目已有 logcat 要求）。
- 其余声称（插件、小部件、包管理器、内建命令）均超出本项目范围或撞禁止项，不参考。

## 禁止项勿抄

源码不可见，任何实现细节不得引用。插件系统、开机脚本、API 桥等撞 PROHIBITED，不得引入。

证据：仓库唯一文件 `README.md`（v1.3.0 发布说明节）。
