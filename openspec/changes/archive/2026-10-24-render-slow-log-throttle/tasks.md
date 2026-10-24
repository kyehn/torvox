# Tasks

## 1. 限频闸门

- [x] 1.1 新增 `render::slow_log::SlowLogThrottle`：进程内单调时钟 + CAS 推进窗口，
      验证：单元测试覆盖首条放行、窗口内压住、窗口走完放行、时钟回跳不放行，全绿
- [x] 1.2 闸门以「尚未放行过」为初始状态，首条无条件放行，
      验证：`first_diagnostic_is_always_logged` 用例

## 2. 诊断接入

- [x] 2.1 四类原生慢帧诊断接入各自闸门：帧内分段、渲染分段、单元构建、采样段，
      验证：`cargo clippy --all-targets --all-features -- -D warnings` 与
      `cargo test --lib` 全绿
- [x] 2.2 四类逐帧告警接入各自闸门：取纹理工作线程忙、取纹理 panic、取纹理超时、
      配置与纹理尺寸不符，验证：同上
- [x] 2.3 升级信号不受限频影响：连续跳过计数到上限仍记 error，
      验证：代码走查——限频只包住日志输出，计数与阈值判定在闸门之外

## 3. 规范增量

- [x] 3.1 规范增量落 `specs/logcat-hygiene/spec.md`：限频要求覆盖宿主侧与原生侧，
      并写明首条可见、限频不得吞掉升级信号，
      验证：`openspec validate --strict` 通过
- [x] 3.2 留档只写聚合计数与结论，MUST NOT 写原始日志行、时间戳、PID、设备标识，
      验证：逐条核对本 change 的全部文档

## 4. 集成验证

- [x] 4.1 Rust 全量，验证：`cargo fmt --all`、
      `cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --lib` 全绿
- [x] 4.2 模拟器复测（滚动仪器化用例期间）：每一类慢帧诊断的最小输出间隔为 500ms，
      原生侧帧内分段诊断的条数由数百条降为二十余条，
      验证：logcat 间隔统计，逐类最小间隔不小于 0.5s
- [x] 4.3 仪器化用例复跑，验证：`:app:connectedDebugAndroidTest` 跑
      `ScrollBehaviorQuantifiedTest` 全绿

## Workflow follow-up

- `openspec validate 2026-10-24-render-slow-log-throttle --strict` 通过后，
  执行 `openspec archive 2026-10-24-render-slow-log-throttle`，
  把 `logcat-hygiene` 的增量同步进主 spec。
- 归档后执行 `openspec validate --all --strict`，确认全量规范一致。
- 归档结果连同本 change 的全部文档作为一个小步提交并推送到 `origin/main`。
