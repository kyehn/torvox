# Proposal

## Why

真机 logcat（2026-11-13 09-22-15）中应用进程自身的 W/E 条目共 112 条，逐条核对后全部
有归因：设备图形分配器的格式探测（`mali_gralloc`）、平台帧时间线同步组查询
（`SurfaceSyncer`）、系统窗口事务的空 Binder 返回（`Parcel`）三组已由现行规约的对应
Scenario 覆盖。余下一条 tag 为进程名的运行时 GC 告警
（`Cleared Reference was only reachable from finalizer`）尚无归因——它是本轮唯一未被
规约记录的 W 级条目。

该条目出自 AOSP ART 的引用处理：软引用与弱引用被清理时，若引用所指对象只被 finalizer
的 `this` 引用持有，ART 会在 `reference_queue.cc` 的 `ClearWhiteReferences` 打出这条
警告（每个虚拟机只打一次）。ART 源码注释自述这是语言规范未覆盖的角落，不是应用缺陷；
仓库内没有任何软/弱/幻引用 API 调用，这类引用由平台与 libcore 内部创建，应用侧无开关。

## What Changes

- `logcat-hygiene` 的「平台与驱动侧条目必须给出可复核的上游事实」补上这组条目的归因
  Scenario：判据位置、一次性语义、应用侧无对应调用。
- 不改任何日志的输出与否：条目照旧出现在 logcat。

## Capabilities

### Modified Capabilities

- `logcat-hygiene`：运行时 GC 引用的 finalizer 可达性告警的归因补全。

## Impact

- `openspec/specs/logcat-hygiene/spec.md`

## 上游事实核对

- 判据位置：AOSP ART `runtime/gc/reference_queue.cc` 的 `ReferenceQueue::ClearWhiteReferences`
  （形参 `report_cleared`），由 `runtime/gc/reference_processor.cc` 在软引用与弱引用的
  引用处理路径以 `/*report_cleared=*/ true` 调用（幻引用不报）。
- 触发条件：待清理的引用是 soft/weak，其 referent 仍被 pending finalizer 的 `this`
  引用持有。ART 同处注释原文：It is unclear that there is any way to do this without
  slightly violating some language spec——即 ART 自认是语言规范未覆盖的角落。
- 一次性：消息自带 `(only reported once)`，`ClearWhiteReferences` 内的静态标志
  `already_reported` 只让首条通过。
- 应用侧：仓库内检索 `WeakReference|SoftReference|PhantomReference|WeakHashMap|
  finalize()` 零命中，应用不创建这类引用，故该条目没有应用栈。
