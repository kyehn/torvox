# Tasks

## 1. 死代码清理

- [x] 1.1 删除 `MockPtyHandle::set_exited` / `is_exited`、`MockPtyInner::child_exited`
      字段及 `MockPty::write` / `read` 中随之不可达的分支，
      验证：`cargo clippy --all-targets -- -D warnings` 零警告，`cargo test --lib` 全绿

## 2. `render_inner` 拆分

- [ ] 2.1 `FrameData` 枚举提到模块级；抽出 `prepare_render`（阶段 0 + 阶段 1）
      与 `collect_frame_data`（阶段 2），
      验证：锁的获取/释放点与返回值与拆分前逐条一致
- [ ] 2.2 抽出 `render_new_frame`（阶段 3 新帧路径）与 `render_idle_frame`
      （阶段 3 空闲路径），
      验证：Kitty 缓存失效规则、空闲重绘门控、脏标志写回时机不变
- [ ] 2.3 抽出共用的脏行掩码构建 helper，消除两条路径的重复，
      验证：新帧路径仍标脏上一帧光标行，空闲路径不标

## 3. `pump_sessions` 拆分

- [ ] 3.1 抽出剪贴板读请求处理函数（洪泛削减 + 一次性槽位 + 应答线程），
      验证：削减上限、事件顺序、槽位取消时机不变

## 4. `run_inner` 拆分

- [ ] 4.1 抽出终端初始化与上游回调注册函数（回滚上限、Kitty 介质、PTY 回写、
      OSC 52 写回、XTWINOPS 尺寸），
      验证：初始化失败仍记错误日志并退出 VT 线程，事件循环语义不变

## 5. 集成验证

- [ ] 5.1 Rust：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib` 全绿
- [ ] 5.2 `aislop scan` 的超长函数告警收敛
- [ ] 5.3 规范：`openspec validate --all --strict` 全绿

## Workflow follow-up

- 归档变更。
