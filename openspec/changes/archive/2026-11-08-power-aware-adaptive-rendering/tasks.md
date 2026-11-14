# 任务

- [x] 通读 `docs/specification` 全部七篇、`AGENTS.md`、`docs/STATUS.md` 与 openspec 规约
- [x] 下载并对照 termux-app、ghostty-android-terminal、termlib、Drosh 与 AndroidTerminal
- [x] 审查 Rust 终端与渲染路径，包括输出、节流、帧回调、滚动与低延迟路径
- [x] 审查 Kotlin 会话生命周期、帧回调、滚动、输入批缓冲与电源状态
- [x] 复核 `docs/audit` 中已有的显示适配与代码健康审查留档
- [x] 识别显示节拍、vSync 链、省电模式与空闲策略的全集
- [x] 撰写本变更的 proposal、design 与规约增量
- [x] 接入真实 vsync 周期数据源，并按实际 vsync 计算帧时长窗口与 FPS（改源码）
- [x] 将帧回调改为按活跃度启停，并补空闲零回调与恢复出帧用例（改源码与测试）
- [x] 在 Manifest 补齐密度、最小屏幕尺寸、键盘与导航配置变化（改 Manifest）
- [x] 在配置回调比较显示密度并重推渲染契约与网格（改源码）
- [x] 在代码注释中把 17ms/34ms/100ms 改为显示周期倍数表述（改源码注释）
- [x] 覆盖断言：刷新率切换时每显示周期至多一帧，且空闲不强制高频（改测试）
- [x] 前台服务执行锁只在有存活会话时持有，无会话即释放（改源码与测试）
- [x] 定向运行 OpenSpec 严格校验，确认全部能力通过
- [x] 归档本 change 并同步主规约
