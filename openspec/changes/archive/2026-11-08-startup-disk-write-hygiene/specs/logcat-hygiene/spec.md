## ADDED Requirements

### Requirement: 启动期不得在主线程产生磁盘写告警

应用 MUST NOT 在主线程上创建或改写持久化文件：目录创建 MUST 与首次读写一并
推迟到承担该读写的 IO 线程，MUST NOT 以放宽 StrictMode 线程策略代替推迟写入
（允许线程磁盘读只放宽「读」，不覆盖「写」）。

冷启动 logcat MUST NOT 出现 StrictMode 的 DiskWriteViolation：该告警由平台
打出却指向应用代码，与真实缺陷无法区分，会掩盖真正的应用侧告警。

#### Scenario: 设置存储在主线程构造

- **WHEN** 应用在主线程上构造设置存储单例
- **THEN** MUST NOT 创建持久化目录；目录 MUST 在首次读写时由 IO 线程创建

#### Scenario: 冷启动日志无磁盘写告警

- **WHEN** 应用冷启动并完成首次设置读写
- **THEN** 应用进程的 logcat MUST NOT 出现 StrictMode 的 DiskWriteViolation
