# 任务

- [x] 逐项 grep 核实 `read_error_action`、`exit_signal_target`、`hyperlink_at_impl`、
      `on_clipboard_write`、`MAX_PTY_WRITES_PER_FRAME` 的现行落点
- [x] 核实 `installPtyWriteQueue` 已由 `Bridge.ptyWriteQueue` 与
      `TerminalSurface.bindActivePtyWriteQueue` 取代
- [x] 编写 `cjk-rendering` 增量（RENAMED + MODIFIED）与 `pty-lifecycle` 增量
- [x] 订正 `upstream-alignment`、`render-loop-scroll-cadence` 的失效指针
- [x] 订正 `Bridge.kt` 两处过时注释
- [x] 校验规约增量并保持既有能力全绿
- [x] 归档本 change 并同步主规约
