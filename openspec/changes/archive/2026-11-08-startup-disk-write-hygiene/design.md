# Design

## 现状

- `SettingsDataStoreProvider` 是 Hilt 单例，在主线程上被 `SettingsRepository` 依赖注入
  时构造。构造体里先 `context.getDir("prefs", MODE_PRIVATE)` 建目录，再把
  `File(prefsDir, "settings.preferences_pb")` 交给 `PreferenceDataStoreFactory`。
- 建目录发生在主线程，StrictMode 因此打出 DiskWriteViolation。原有处置是把它包进
  `StrictMode.allowThreadDiskReads()`，该开关只放宽「读」，对「写」无效。
- `TerminalViewModel.clearAppData()` 删除并重建 `prefs` 目录；目录路径由
  `context.getDir` 给出，与本单例的目录同源。

## DataStore 的文件回调落在哪个线程

按 1.3.0 的字节码核对（`datastore-core-android` 的 `DataStoreImpl`、
`datastore-core-okio-jvm` 的 `OkioStorage`）：

1. `DataStoreImpl.readDataOrHandleCorruption` 的函数体包在
   `withContext(scope.coroutineContext)` 里。
2. 该 scope 的调度器取自 `Actual_jvmKt.ioDispatcher()`，即 `Dispatchers.IO`。
3. `storageConnectionDelegate` 是 `lazy`，其值来自 `storage.createConnection()`；
   `OkioStorage.createConnection()` 解析 `canonicalPath$delegate`，后者调用
   `produceFile()`。

结论：`produceFile` 只在首次读写时被求值，且求值发生在 DataStore 自己的 IO 作用域
上，不在收集 `data` 的线程上。

## 设计决定

### 目录创建移入惰性文件回调

把 `context.getDir("prefs", MODE_PRIVATE)` 写进 `PreferenceDataStoreFactory.create`
的文件回调：目录创建与首次读写一并交给 IO 线程，主线程构造期不再产生磁盘写。

不选「构造期在后台线程建目录」：那需要额外的作用域与时序保证，且目录与文件路径
要跨线程传递；移入回调后两者同处一个表达式，没有中间状态。

不选「保留 StrictMode 线程策略切换」：它放宽的是磁盘读，而告警来自目录创建这一
磁盘写，放宽读不会让告警消失，只会掩盖读类告警。

不选「把磁盘写告警过滤掉」：告警由平台打出，过滤日志等于把真实缺陷一起藏起来。

### 与清除应用数据的关系

`clearAppData()` 删除 `prefs` 目录后立即用 `context.getDir` 重建，目录名与回调里
用的是同一个，故路径解析结果不变；DataStore 已解析的绝对路径仍然指向重建后的
目录，行为与改动前一致。

## 验证

- 单测：构造设置存储单例时不得创建任何目录；首次读取 `data` 时目录由非构造线程
  创建；写入与读取照常往返。
- 实机：清空应用数据后冷启动，按进程号过滤 logcat，StrictMode 的
  DiskWriteViolation 计数为零。
