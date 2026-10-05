# 字体致命路径收敛与崩溃可诊断性

## Why

同一台设备（ZTE P720S20 / Android 13）在 2026-10-02 17:31 的启动崩溃只能拿到一段无符号
tombstone：`prefetchRenderState` 线程内 SIGABRT，`libnative.so` 只有偏移没有符号
（`[profile.release] strip = true`），logcat 里也没有任何原因行。排查后确认该崩溃已由
`2026-10-01-fix-font-startup-abort` 修复（2026-10-05 同机同包运行正常，
`setFontFamily: Droid Sans Mono found=true`）。但「崩溃时拿不到原因」这一缺口仍在：

- 原生日志由 Kotlin 的独立线程 `NativeInit` 异步安装（`NativeBridge.initLogger`）。
  启动 2 秒内的 abort 抢在该线程之前时，致命原因经 `log` 门面的输出被静默丢弃，
  现场只剩 tombstone——这正是本次 tombstone 里没有任何原因行的原因。
- `fonts.xml` 的等宽字体解析在一次管线创建里执行两次：`load_font_database` 的建库闭包
  内一次，`find_monospace_font` 内无条件再一次。每次都是重新读盘 + 重新解析整份 XML，
  且 `set_font_family("")` 每次调用都重做。
- 致命判定分散在三处、文案三份：`resolve_system_monospace_from_fonts_xml` 的
  `fonts.xml` 不可用、`find_monospace_font` 的库空。其中库空那条写作
  「fonts.xml 未提供任何可用字体面」，但它真正的含义是「声明的字体文件一个都装不进
  fontdb」（文件缺失/损坏/不可读），与 `fonts.xml` 本身无关——文案会把排查引向错误方向。

## What Changes

- 原生日志改由 `JNI_OnLoad` 安装：VM 在加载本库时调用，早于任何 JNI 方法，
  故第一个 `log::*` 调用之前 logger 已就位，致命原因必然落 logcat。由此删除
  `NativeBridge.initLogger` 导出与 `TerminalApp` 的 `NativeInit` 线程。
- 字体侧致命判定收敛到 `font_db::fatal(reason) -> !` 唯一出口：以
  `FONT_FATAL` 为 target 输出 ERROR 后 `abort`，使同类崩溃一次 grep 即可归因。
- `resolve_system_monospace_from_fonts_xml` 改为进程级缓存
  （`OnceLock<String>`，紧邻既有 `CACHED_FONT_DB`）：一次解析结果被建库与
  `find_monospace_font` 共用，`set_font_family("")` 不再重复读盘解析。
- 库空的致命原因改为陈述事实（声明文件全部装入失败），不再归因 `fonts.xml`。
- 宿主单测固化「`fonts.xml` 无等宽族即无主字体候选」这一触发条件。

## Non-goals

- 不改崩溃退出这一 DESIGN 错误策略本身，也不把致命条件改为降级。
- 不改 `select_primary_face` 的降级梯次与放宽装库行为。
- 不改 `[profile.release] strip` / `panic` 设置（`.so` 符号化与 `jni_export_guard`
  的 `catch_unwind` 在 `panic = "abort"` 下失效，属另一议题，需另行确认）。
