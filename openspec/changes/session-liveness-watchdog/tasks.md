# 任务

- [ ] 补充现有 `RenderWatchDog` 的分层代数输入与纯函数判定的 JVM 单测
- [ ] 设计并评审 `SessionLivenessDiagnosis` 数据结构与 `HungLayer` 枚举
- [ ] 在 `SessionEntry` 中增加原子代数与最后进展时间字段（Rust 侧）
- [ ] 在渲染线程推进 `renderGeneration` 与时间
- [ ] 在 VT 线程推进 `vtGeneration` 与时间
- [ ] 在 PTY 读取线程推进 `ptyReadGeneration` 与时间
- [ ] 把 `RenderWatchDog` 的回调改为记录层级并交由会话管理器
- [ ] 为 VT/PTY 层挂起接入会话重建路径
- [ ] 增加自动重建节流（60s 内一次）
- [ ] 更新日志与诊断输出
- [ ] 补充 Rust 原子性与顺序单测
- [ ] 归档本 change 并同步主规约
