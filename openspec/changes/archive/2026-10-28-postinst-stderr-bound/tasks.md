# 任务

- [x] `drainQuietly` 改为读到 EOF、只保留前 64 KiB
- [x] stderr / stdout 交接改用 `AtomicReference`，去掉非线程安全的 `StringBuilder`
- [x] 新增单测：4 MiB 输入读完且返回长度不超过上限
- [x] `SecondStageRunnerTest` 全绿
- [x] 规范补上「读到 EOF、保留量有界、交接线程安全」的要求与场景
