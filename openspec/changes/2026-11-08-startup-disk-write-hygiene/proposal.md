# Proposal

## Why

冷启动 logcat 里的应用侧告警必须逐条归零，否则日志分级失去判据。审查三份 logcat
时发现：设置存储单例在**主线程构造期**就调用 `Context.getDir` 建目录，StrictMode
因此打出 DiskWriteViolation。该告警由平台打出却指向应用代码，与真实缺陷无法区分；
而当时的处置是切换 StrictMode 线程策略（允许线程磁盘读），那只放宽「读」，并不
覆盖「写」——告警仍在。

DataStore 的文件回调只在首次读写时、于它自己的 IO 作用域上被调用。把目录创建一并
放进去，这次磁盘写就离开主线程，既不需要额外线程，也不需要兜底或降级。

## What Changes

- 设置存储单例的目录创建从构造期移入 DataStore 的惰性文件回调，由 IO 线程承担。
- 删除仅为放宽 StrictMode 而存在的线程策略切换代码。
- 补单测：构造期不得创建目录；首次读取时目录在非构造线程上创建；读写照常往返。
- `logcat-hygiene` 增一条要求：冷启动不得出现主线程磁盘写告警。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：增一条「启动期不得在主线程产生磁盘写告警」的要求。

## Impact

- `android/app/src/main/java/terminal/emulator/settings/SettingsDataStoreProvider.kt`：
  目录创建移入 DataStore 的惰性文件回调，删除 StrictMode 线程策略切换。
- `android/app/src/test/java/terminal/emulator/settings/SettingsDataStoreProviderTest.kt`：
  新增回归用例。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步该条要求。
