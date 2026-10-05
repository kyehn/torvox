# 任务

- [x] 建 change
- [x] 原生日志改由 `JNI_OnLoad` 安装，删除 `initLogger` 导出与 `NativeInit` 线程
- [x] `font_db::fatal` 唯一致命出口，target 为 `FONT_FATAL`
- [x] `resolve_system_monospace` 改进程级缓存，建库与 `find_monospace_font` 共用
- [x] 库空致命原因改为陈述装入失败，不再归因 `fonts.xml`
- [x] 宿主单测：无等宽族的 `fonts.xml` 产出空主字体候选
- [x] cargo fmt / clippy / test / doc，aarch64-linux-android check
- [x] 更新 openspec/specs 并归档 change
