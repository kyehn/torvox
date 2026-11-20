# 任务

## 一、定位

- [x] 复核上一轮修复（`2026-11-20-swapchain-texture-return`）覆盖的三条丢弃路径
- [x] 确认上游事实：wgpu-hal 30.0.2 的 Vulkan 后端 `discard_texture` 是空实现
- [x] 定位遗漏窗口：渲染线程超时返回后接收端仍存活，工作线程 `send` 成功的窄窗口

## 二、实现

- [x] 新增 `ResultHandoff`：渲染线程放弃与工作线程回送在同一把锁内互斥
- [x] 工作线程：结果被退回时自行归还纹理（清成背景色后呈现）
- [x] 渲染线程：超时路径取回已回送的结果并自行归还
- [x] 归还助手 `release_acquired_result` 统一处理「结果里没有纹理」的情形

## 三、验证

- [x] 回归测试三条：先放弃 ⇒ 工作线程拿回结果；先回送 ⇒ 渲染线程取回结果；结果不被交出两次
- [x] 敏感性验证：临时退回旧行为（忽略放弃标记 / 取回返回 None）时回归测试确实失败
- [x] `cargo clippy --all-targets -- --deny warnings`：通过
- [x] `cargo fmt --check`：通过
- [x] `cargo test --no-fail-fast`：936 passed, 0 failed
- [x] `openspec validate --all --strict`：通过
- [x] 模拟器（API 35 / SwiftShader）实测：冷启动、输入、旋转后持续出帧，本应用 0 条 W/E，
      无「取纹理超时」与停帧
- [x] 模拟器仪器化回归：`SurfaceLossRecoveryInstrumentedTest`、
      `SurfaceRedrawAsyncInstrumentedTest`、`TerminalHoleCoverInstrumentedTest` 共 10 例全过
- [x] `scripts/check.nu` 全量门禁：通过
