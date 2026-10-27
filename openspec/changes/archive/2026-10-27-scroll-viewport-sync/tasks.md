# 任务

- [x] 实证上游 `scrollbar()` 语义：视口锚在内容上，回滚浏览期间「离实时视图多少行」随输出漂移
- [x] 规范补上「陈旧帧用请求序号识别」与「亚行余量与行偏移同拍提交」的要求与场景
- [x] 滚动命令改带请求序号（进程级单调递增），VT 处理后随帧回传
- [x] 新帧路径拦截早于在途滚动命令的陈旧帧：不呈现，也不解除滚动在途门控
- [x] 宿主侧：行偏移未被终端接受时不提交亚行余量，余量保留旧值下一帧重试
- [x] 回归测试：陈旧帧判定表 + 序号随帧回传
- [x] `cargo fmt`、`cargo clippy --all-targets -- --deny warnings` 通过
- [x] `cargo test` 全绿（739 + 2）
- [x] `detekt`、`spotlessCheck`、`testDebugUnitTest` 通过
- [x] `openspec validate --all --strict` 通过
