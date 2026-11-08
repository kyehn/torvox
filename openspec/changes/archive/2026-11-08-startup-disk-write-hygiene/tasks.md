# 任务

- [x] 核对 DataStore 1.3.0 的文件回调所在线程（`DataStoreImpl` 与 `OkioStorage` 字节码）
- [x] 把 `prefs` 目录创建移入 DataStore 的惰性文件回调
- [x] 删除仅为放宽 StrictMode 而存在的线程策略切换代码
- [x] 补单测：构造期不得创建目录
- [x] 补单测：首次读取时目录在非构造线程上创建
- [x] 补单测：设置读写经惰性创建的存储照常往返
- [x] `:app:testDebugUnitTest` 与 `detekt`/`spotlessCheck` 通过
- [x] 实机冷启动复核：StrictMode 的 DiskWriteViolation 计数为零
- [x] 归档本 change 并同步主规约
