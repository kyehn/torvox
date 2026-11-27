# velo-android 研究留档

## 定位

Velo 是 Rex 的 Android 客户端：手机上不跑本地 shell，终端状态与渲染由 Zig 原生核心 `VeloCore`（`native/src/jni.zig`）持有，Kotlin 只负责把 Surface、按键与触摸交给核心，并接收 JSON 状态与栅格化后的字形位图。另有 Go 实现的 Tailscale 节点与 SSH 支持。

## 架构

`HostConnection` 是 Kotlin 与 `VeloCore` 的唯一桥：`op()` 构造 JSON 命令，`requestState` 后的 `nativeState.accept` 解码出 `UiState`；`paneSurface`/`paneResume`/`paneSuspend` 管理各 pane 的 Surface 生命周期，`key`、`touch`、`pan`、`copy`、`find`、`click`、`markLink`、`bracketedPaste` 等把交互全部下推到核心。每个 pane 用 block id 标识，Kotlin 不持有 cell 网格或滚动状态。`CoreEnv` 提供 CA 目录、字体缩放、密度、`GlyphRasterizer`、设置与存储；`composerTarget` 返回单调 epoch 防止迟到提交写错目标。

`GlyphRasterizer` 从 assets 构造 `Font`/`Typeface`，按字体资产组合 regular/bold/italic 与韩文回退；`rasterizeGlyph(utf8, bold, italic, cellW, cellH, columns)` 把 cell 文本画成位图并返回字节：彩色 emoji 用 ARGB_8888，其余用 ALPHA_8，按 `(cellW, cellH)` 缓存行距与基线，用 `TextRunShaper` 判断主字体是否覆盖。`PaneSurfaceView` 负责显示 Surface、fling、链接与选区手柄；`TerminalInputView` 处理 `InputConnection`、组合文本、sticky 修饰键、粘贴确认与硬件键分发。

应用层面还有多服务器/多 pane、折叠屏并排布局、代理端口转发、打印地址与文件路径的点击打开、任务完成通知等。

## 已核实局限

- 终端能力完全依赖远端 Rex 与原生核心；离线或没有服务器时没有本地终端可用，与本仓“本地 PTY + VT”的目标不同。
- 字形以位图字节回传，Kotlin 无法做逐 cell 的文本选择或富文本布局；选区、搜索、链接命中必须在核心里实现并经 JNI 查询，接口面很大。
- Tailscale、SSH 密钥、更新通道等属于本仓规范未声明功能，不移植。

## 与本仓对照

- 本仓架构与之相反：VT 状态、PTY 与 GPU 绘制在 Rust，Kotlin 只持有会话表、设置和渲染调度，并通过 JNI 取轻量事件；不能引入“Kotlin 交 Surface、native 回位图”的模型。
- `rasterizeGlyph` 按 `(cellW, cellH)` 缓存度量、区分彩色/单色位图的做法，可用来核对本仓字体度量缓存是否覆盖所有实际字号与 emoji 路径。
- pane 的 `suspend`/`resume`/`preview` 生命周期与前台暂停渲染的要求相近，可作为本仓后台暂停、切回无闪烁的用例来源。
