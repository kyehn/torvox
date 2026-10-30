# Tasks

## 1. 归因复核

- [x] 1.1 取渲染能力提示的上游事实：告警发生在建设备时，能力位由设备扩展集决定，
      应用声明的表面格式列表不参与计算，
      验证：`wgpu-core`/`wgpu-hal`/`wgpu-types` 三处源码逐条比对
- [x] 1.2 取设备分配器格式探测的上游事实：表面重建时驱动枚举全部表面格式并逐个试分配，
      应用侧无开关，验证：同上，另在模拟器复跑中复核到同一条能力位告警
- [x] 1.3 定位系统窗口事务的空 Binder 返回成因（厂商动态库加载、弹出窗口的加入与重排），
      验证：应用代码无 `readStrongBinder` 调用，选区菜单在存活时复用同一窗口
- [x] 1.4 复核文档提供器回滚事务的文件不存在告警属正常流程，
      验证：文档提供器契约已声明缺失路径抛文件不存在异常

## 2. 应用自身的正常路径降级

- [x] 2.1 安装后脚本的执行轨迹行由 warn 降为 info，失败与重试仍记 warn，
      验证：模拟器复跑中该类条目不再出现在 warn 级
- [x] 2.2 剪贴板应答回写失败按会话是否已结束分级：会话已退出或 PTY 已随子进程关闭时
      记 debug，其余记 warn；判据与输入路径同口径，
      验证：`cargo test --lib clipboard` 全绿（三个分类用例通过）

## 3. 规范与留档

- [x] 3.1 `logcat-hygiene` 增量：正常退化路径补上「安装后脚本轨迹行」与「会话已结束的
      剪贴板应答回写」，并写明同一情形在不同调用点必须同等级
- [x] 3.2 `logcat-hygiene` 增量：新增「平台与驱动侧条目必须给出可复核的上游事实」，
      含渲染能力提示、设备分配器格式探测、系统窗口事务三个场景
- [x] 3.3 留档只写聚合结论、倍数关系与复现步骤，不写原始日志行、时间戳、PID、
      设备标识或下载链接，验证：逐条核对本 change 的全部文档

## 4. 验证

- [x] 4.1 Rust：`cargo fmt --all`、`cargo test --lib clipboard`
- [x] 4.2 Rust 全量：`cargo clippy --all-targets -- --deny warnings`、`cargo test --no-fail-fast`
      （751 个库用例与 2 个集成用例全绿）、`cargo ndk -t arm64-v8a clippy`、
      `cargo machete`、`cargo doc`、`cargo bench --quick`
- [x] 4.3 Kotlin：`:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessCheck`、
      `lintDebug`、`lintVitalRelease`、`app:dokkaGenerate`
- [x] 4.4 规范：`openspec validate --all --strict`（40 项全过）
- [x] 4.5 静态检查：`semgrep scan --config .semgrep/rules.yml`（0 findings）、
      `markdownlint-cli2 "**/*.md"`（0 issues）
- [x] 4.6 模拟器：`:app:connectedDebugAndroidTest` 全量复跑（235 个用例，0 失败），
      复跑日志确认安装后脚本轨迹行已降到 info 级

## Workflow follow-up

- `openspec validate 2026-10-29-logcat-third-pass --strict` 通过后，
  执行 `openspec archive 2026-10-29-logcat-third-pass`，
  把 `logcat-hygiene` 的增量同步进主 spec。
- 归档后执行 `openspec validate --all --strict`，确认全量规范一致。
- 归档结果连同本 change 的全部文档作为一个小步提交并推送到 `origin/main`。
