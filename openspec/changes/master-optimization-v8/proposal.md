# Proposal: Master Optimization v8

> 日期：2026-09-01 | 状态：活跃

## 概述

以 openspec 为长期规范载体，完成全代码库逐步评审、保守精简、依赖滚动、自动化验证闭环，模拟器 90fps+ 可复现、后端确定性 1017 已达标、UI 可靠可测，连续三次审阅无问题后交付。

## 动机

1. **任务标记与实现不一致**：`comprehensive-hardening-v7/tasks.md` 32 项全未勾选，而代码与验证报告显示 70% 已落地，需对齐。
2. **构建产物与验证链路断点**：`jniLibs/arm64-v8a/libnative.so` 本机 release 产物缺失，`dumpsys gfxinfo` 在模拟器未启动 Activity 时返回 `No process`，需重建自动化脚本。
3. **代码规模与小瑕疵累积**：大文件耦合度高；`DEFAULT_LANG` 未遵循 DESIGN spec `C.UTF-8`；`cargo audit` 2 条 unmaintained 警告。
4. **像素级优点复制未量化**：26 项目每个优点的复制方式与验证手段分散，缺乏单一可执行清单与自动化判定。

## 成功判定

| ID | 判定 | 阈值 | 验证命令 |
|----|------|------|----------|
| S1 | Rust 单元测试全绿 | 1017+ passed, 0 failed | `cargo test -p native --lib` |
| S2 | Clippy 零新增告警 | 0 warnings | `cargo clippy -- -D warnings` |
| S3 | 未用依赖零 | machete 0 | `cargo machete` |
| S4 | Release .so 体积守卫 | arm64 14–18M, 无 NEEDED ghostty | `readelf --dynamic` + `ls -lh` |
| S5 | APK 含 .so | 含 libnative.so | `unzip -l app-debug.apk` |
| S6 | 模拟器帧率 | 90th <16ms | `dumpsys gfxinfo framestats` |
| S7 | 依赖滚动零破坏 | cargo update 后全测绿 | `cargo update && cargo test` |
| S8 | 连续三次审阅零阻塞 | review 3 轮均零 P0/P1 | 人工 + 自动化 review |
| S9 | 代码量净减少 | net LoC ≤ 0 | `git diff --stat` |

## 范围

- 文档：OPENSPEC-STATUS.md v1.1、测试计划、验证协议、依赖变更日志
- 代码：DEFAULT_LANG 修复、常量化 magic numbers、注释溯源、格式化
- 构建：verify-build-artifacts.nu、verify-emulator.nu、verify-cjk.nu
- OpenSpec：v8 change 文档全套
