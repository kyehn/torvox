# nex 上游审查

- 上游：https://github.com/jramirez-codes/nex @ `43356e5`（2026-11-26）；单 Rust crate 的 GPU 终端 + 远程助手。
- 定位：`alacritty_terminal` 负责 VT，`egui/eframe` 负责全部 UI；集成标签、分屏、文件、git、SSH 与 AI agent 面板。

## 架构与数据流

- `src/terminal/mod.rs` 每窗格一个终端，拥有 PTY 与 VT 状态；`input.rs` 把键鼠转成 xterm 序列；`osc7.rs` 从 PTY wrapper 提取 cwd；`search.rs` 做滚动搜索与 URL/OSC 8 检测；`snapshot.rs` 在锁内复制可见屏幕，渲染只读副本。
- `src/render/` 负责运行段、颜色、光标、选择、搜索高亮；`glyphs.rs` 字形缓存与每窗格 mesh，`boxdraw.rs` 程序化绘制框线/块/盲文/Powerline，`fonts.rs` 管字体设置。
- `src/app/workspace/layout.rs` 是二叉分割树，焦点按方向计算；`session/` 以单写者锁管理 `session.json`。后台任务拆为 `Job`（后者取代前者）与 `Jobs`（并行），完成即请求重绘。
- 远程窗格不是纯 SSH 文本流：`terminal/remote.rs` 与 `nex-host` 配合让分屏留在服务器上；文件/git 面板在远端执行 shell 脚本。`remote/upload.rs` 用 `posix_quote` 拼接上传命令，`remote/exec.rs` 承载会话与跟随。
- `nex-android/` 与 `nex-ios/` 是同一库的包装；Android 端通过 JNI 暴露键盘/剪贴板接口，`build.rs` 收集 Java/Kotlin 源码。

## 已核实问题

- `App::run` 在启动时无条件调用 `upload_all(false, false)` 与 `save_png()`：前者尝试上传未确认资源，后者把当前帧写为 PNG；两者都应受设置或显式动作控制，不能作为默认副作用。
- `unsafe` 计数约 114，集中在 JNI/平台边界；集成功能使状态空间很大，若只借终端核心应避免连带引入面板状态模型。
- 文档自述 iOS 为 beta、macOS 支持较新；Android 路径的平台差异主要在输入与窗口生命周期。

## 对 torvox 的取舍

- 只取两点：`snapshot.rs` 的“锁内复制、锁外绘制”与 `render/boxdraw.rs` 的程序化框线兜底。
- 对照本地 `native/src/render/cell_builder.rs` 的 `compute_dirty_bands`：Nex 是整窗格 mesh，本地是脏行增量，不能把其缓存策略整体搬过来。
- 启动期副作用是明确反例：本地 `TerminalScreen` 的 Surface 生命周期已由显式门闸控制，不增加隐式上传/截图。
- 跳过：未构建桌面/移动端与 nex-host（需 Rust 图形栈和 SDK）；风险是运行状态只按源码与 `ARCHITECTURE.md` 核对。
