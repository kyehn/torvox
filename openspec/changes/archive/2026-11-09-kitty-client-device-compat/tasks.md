# 任务

- [x] 修正 Kitty 控制数据键名：数据长度为 `S`、文件内偏移为 `O`
- [x] 会话生成成功后把 Surface 网格区像素尺寸补齐到该会话 PTY winsize
- [x] 切换会话时在网格对齐之后补齐目标会话的像素尺寸
- [x] 文件介质按声明长度读不满时由本层按上游语义代答 `EINVAL: invalid data`
- [x] 临时文件白名单对齐上游口径（`/tmp`、`/dev/shm`、会话 `TMPDIR` 及其规范化形式）
- [x] 补单测钉住代答与放行判据（含「像素宽不是数据长度」与「偏移越界」）
- [x] 仪器用例加入像素尺寸回归断言，并跑通真实 `kitten icat` 显示与 `clear` 清除
- [x] 运行 `cargo fmt`、`cargo clippy`、`cargo test`
- [x] 定向运行 OpenSpec 严格校验，确认全部能力通过
- [x] 归档本 change 并同步主规约
