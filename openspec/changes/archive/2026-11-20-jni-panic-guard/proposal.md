# Proposal

## Why

Rust 侧的 JNI 导出（`Java_terminal_emulator_bridge_NativeBridge_*`，共 78 个）必须把
panic 转成 Java 异常，否则 panic 越过 `extern "system"` 边界会 **abort 整个进程**：
没有 Java 异常、没有崩溃处理器、现场只剩 tombstone。

该约束此前只由 `jni_export_guard!` 宏的文档注释与逐个调用点的自觉维持：新增导出时
只要漏写守卫，后果就是「一个 panic 带走整个进程」，而代码里没有任何东西会提示这件事。

本次复核确认：现有 78 个导出全部经守卫（另有 `JNI_OnLoad` 一例，由 VM 在加载期调用、
无 `Env`，不在守卫范围内）。缺的是**让这条约束可持续**的手段。

## What Changes

- 新增 `jni-boundary` 能力：规定 JNI 导出的 panic 处置、失败值口径、panic 文本可读性，
  以及该约束必须由静态扫描固化。
- 新增静态规则 `jni-export-must-use-panic-guard`：命中未使用 `jni_export_guard!` 的
  `Java_*` 导出函数；`JNI_OnLoad` 不命中。
- 顺带补齐取纹理交接协议的并发回归：真实线程反复制造「放弃」与「回送」交错，断言
  结果恰好被一方取走（对应 `render-stability` 的「迟到结果的归还责任唯一」，此前只有
  两种固定先后次序的单线程用例）。

## 不做什么

- 不改 `jni_export_guard!` 的实现：它已把 panic 与 Rust 错误一并转为 Java 异常，并按
  调用点声明的失败值返回。
- 不给 `JNI_OnLoad` 加守卫：VM 在加载库时调用它，此刻没有 `Env` 可用。
- 不把「宿主不捕获 Java 异常」改成别的策略：进程该不该退出由宿主决定，边界只负责
  把 panic 变成可归因的异常。
- 不动 `docs/STATUS.md`：该文件首行声明禁止修改。

## Capabilities

### New Capabilities

- `jni-boundary`：JNI 导出边界的 panic 处置、失败值口径与静态固化要求。

### Modified Capabilities

（无）

## Impact

- Affected specs: `jni-boundary`（新增）。
- Affected code: `.semgrep/rules.yml`（新增一条规则）、
  `native/src/render/pass/tests.rs`（新增并发回归用例）。
- 行为变化：无。现行导出全部合规，规则只是把既有事实固化为可扫描的不变量。
