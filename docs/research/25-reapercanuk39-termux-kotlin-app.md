# 25 reapercanuk39/termux-kotlin-app

上游：<https://github.com/reapercanuk39/termux-kotlin-app>
本地：/tmp/competitor-study/reapercanuk39__termux-kotlin-app
版本：主线 2026-01-27 取样，677 文件。termux 完整 Kotlin 化，模拟器与视图逐文件转 Kotlin，`termux.c` 仅一处释放错位（同 24）。外加 AI 代理（MCP、LLM、Ollama、技能、沙箱）与海量治理文档。核心 diff 已通读。

## 血缘

模拟器 14 文件 Kotlin 化（含 84K 模拟器与 43K 宽字符表），视图 Kotlin 化，JNI 同款错位。应用层加代理运行时：意图识别、任务规划、MCP 工具注册、沙箱执行、Ollama 客户端、文件网络包技能。文档含迁移、架构、路线、错误集。

## 与本项目对照

Kotlin 化本身无参考价值（本项目 Rust 持有会话，Kotlin 仅 Compose 薄层）。AI 代理、MCP、外部模型调用撞禁止项。JNI 错位同 24。宽字符表 Kotlin 化体量证明宽表应由 Ghostty 提供，不自研。

## 可参考点

- P2：Kotlin 迁移文档的问题清单组织（仅文档思路）；宽字符表体量作为不自研的佐证。
- 其余无。

## 禁止项勿抄

整套代理运行时、MCP、LLM 调用、JNI 错位、自研宽表。

证据：`terminal-emulator/.../kotlin/` 14 文件、`termux.c:164-165`、`agents/` 全目录、`KOTLIN-MIGRATION.md`。
