# nex 研究留档

## 定位

nex 是 Rust 单体 crate 的 GPU 终端，桌面用 egui/eframe 与自己的 winit 事件循环，VT 引擎是 `alacritty_terminal`。除 `platform/` 外不检查操作系统；Android 与 iOS 是同一库包出来的瘦客户端，所有 pane 都跑在 ssh 主机上的 `nex-host` 会话里。

## 结构

`layout.rs` 是二叉 split 树，pane 用 `PaneId` 标识，四种类型（终端、文件浏览器、Changes、Agents）在 `layout::PaneKind` 中穷举；`App::panes` 是统一存储，访问某一种类型用 typed lookup，分支代码必须 match 整个 `Pane`，从而让编译器发现新增类型要改的所有位置。后台工作统一走 `jobs.rs`：`Job` 同时只保留一个，后来者让先前的答案作废；`Jobs` 并发多个，完成都会请求重绘，帧循环不阻塞。

`terminal/mod.rs` 的每个 pane 持有 PTY、子进程、`FairMutex<Term<EventProxy>>`、标题、cwd 与前台进程信息；标签只在这些信息变化时重建。`Snapshot::capture` 在锁内把可见 cell 拷入复用缓冲，放开锁后再绘制，避免 PTY 线程在 UI 绘制期间被卡住。`osc7.rs` 包裹 PTY 以捕获 shell 工作目录，`remote.rs` 把远端会话伪装成本地 pane，`search.rs` 覆盖回滚搜索与 URL/OSC 8 链接。`render/` 用字形缓存与 per-pane mesh，`boxdraw.rs` 程序化绘制盒线、块、盲文与 Powerline，并内嵌 Nerd Font。

## Android 状态

`nex-android` 只是 NativeActivity 加载的共享库，`android_main` 交给 `nex::android_main`。README 把移植分成里程碑：启动到连接屏与 ssh 已完成；触摸输入（Esc/Ctrl/Tab/方向键的额外键栏、终端避开键盘、拖动滚动）标注为 on hold；Agents/Changes/Files pane 也还没做完。设备侧没有本地 shell，每个 pane 是服务器上的 `nex-host` 会话，网络断开后重连恢复画面。

## 已核实局限

- Android 版没有完成触摸输入链路，不能作为本地 Android 终端的现成参考；只能借鉴桌面侧的输入编码与快照模型。
- 大量功能（Agents 流水线、git diff、文件浏览器、MCP 之外）超出本仓规范，不能引入。
- 快照在锁内拷贝是正确方向，但 `Snapshot::capture` 复用了上一次的 `Vec<SnapCell>`，若被多处同时持有需要额外约束；本仓的快照共享边界要自己核对。

## 与本仓对照

- “锁内拷贝快照、锁外绘制”与本仓 Rust 侧渲染、Kotlin 只收事件的架构一致，适合用来核对本仓帧获取是否存在持有会话锁绘制的路径。
- 后台任务统一注册、完成时请求重绘，可对照本仓 `render/pump` 与事件唤醒，避免固定时间轮询。
- 程序化盒绘制与 OSC 7 捕获能作为本仓已有实现的交叉检查；触摸输入尚未完成这一部分不能照搬。
