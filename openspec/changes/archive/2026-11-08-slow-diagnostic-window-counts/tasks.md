# 任务

- [x] 核对规范对逐帧诊断的要求：行内 MUST 带窗口内事件计数
- [x] 原生侧四类逐帧诊断改用带计数的闸门，行内加 `slow_frames`
- [x] 宿主侧新增 `CountedLogThrottle`（单调时钟、窗口内计数延后交出）
- [x] 宿主侧 `SLOW_FRAME` 与 `SLOW_LOOP` 改用带计数的闸门，行内加 `slow_frames` / `slow_loops`
- [x] 补宿主侧单测：首条可见、窗口内压住、计数延后交出、时钟回跳不放行
- [x] `cargo test --lib`、`cargo clippy --all-targets -- -D warnings`、`cargo fmt --check` 通过
- [x] `:app:testDebugUnitTest`、`detekt`、`spotlessCheck` 通过
- [x] 归档本 change 并同步主规约
