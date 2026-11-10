# Design

## Context

整轮 `connectedDebugAndroidTest` 偶发报「进程崩溃」，崩溃点每次不同。设备 logcat 给出
同刻的强杀记录（`ActivityManager: Killing …: stop … due to set debug app`），且同一窗口内
出现两次「设置调试应用」：框架的测试包安装器在 API 35 及以上会先执行 `am set-debug-app`，
这一步会强杀正在跑的同包应用进程，被杀的一轮遂报「进程崩溃」。

## Goals / Non-Goals

**Goals:**

- 让「并发跑」这一归因有规约依据，而不是每次从崩溃现象重新排查。

**Non-Goals:**

- 不引入设备租约或锁机制：设备编排属于 CI 与本地工具链，不在本仓规约范围。
- 不改应用侧代码：应用无法阻止框架按「设置调试应用」强杀另一轮进程。

## Decisions

- 约束落在 `build-and-validation`：它是验证入口的规约位置；`instrumentation-harness`
  管的是用例基类与规则链，不管「谁在用设备」。
- 判据取 logcat 的强杀记录，不取「崩溃点漂移」这类间接现象：强杀记录是框架写下的
  直接事实，漂移只是并发的一个表现。
- 要求写成「跑前确认设备空闲」而不是「加锁」：锁要落在调度侧，本仓只有一条命令的
  入口，写不出可靠的锁。

## Risks / Trade-offs

- [规约约束不了调度侧] → 靠跑前确认与归因口径兜住：并发造成的崩溃被正确归因，
  不会被误当缺陷反复排查。
