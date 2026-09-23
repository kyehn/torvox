# 参考

项目唯一参考文档。代码注释中的 `docs/reference/REFERENCE.md` 指向本文件对应小节。研究报告原件已删除，历史版本见 git 记录（`git log -- docs/reference/`）。

## 上游对照

未声明细节以上游两家为准：

- [termux-app](https://github.com/termux/termux-app)
- [ghostty-android-terminal](https://github.com/sylirre/ghostty-android-terminal)，libghostty-vt 用法见其[架构文档](https://github.com/sylirre/ghostty-android-terminal/blob/main/docs/architecture.md#libghostty-vt)

## 已落地引用

仅收录已在代码中落地的外部参考，未落地想法不收录。

### 超链接识别

源自 zed-android-port `URL_REGEX`（20 协议前缀，须含 `ipfs:`/`ipns:`）。实现用 `linkify` 做 RFC-3986 风格扫描（括号平衡、尾标点清理），手写正则无法覆盖。落点：`native/src/terminal/url_regex.rs`。

### DECCKM 与方向键编码

应用光标模式（DEC 私有模式 1）下方向键须编码为 SS3（`ESC O A`）而非 CSI（`ESC [ A`），否则 vim/less 误读。来源：haven 通用终端知识、zed-android-port 按键映射。落点：`TerminalInputEncoder.kt`、`Bridge.kt`、`NativeBridge.kt`、`native/src/android/ffi.rs`（`getMode`）。

### CSI 27 修饰键编码

`ESC [ 27 ; modifier ; code ~`，修饰位 Shift=1、Alt=2、Ctrl=4（zed `mappings/keys.rs` `modifier_code`）。无传统 caret 折叠的 Ctrl+数字/标点走此编码。落点：`TerminalInputEncoder.kt`。

### 字体槽位

粗体/斜体独立 family 槽位（ghostty-android `TerminalFontStore` 四槽设计）：样式槽位已设置时优先使用真实字形，胜过同 family 查找加合成。槽位索引：0=粗体，1=斜体，2=粗斜体。落点：`native/src/render/font/pipeline.rs`。

### 离屏验证帧

程序化几何加深度附着 LOD 网格仅限 crate 测试；生产 `Renderer` 零深度附件（2D 终端不需要），不得随包发布或泄漏进 native 集成测试。来源：wgpu-example。落点：`native/src/render/mod.rs`。

### 选择手柄拖动

termlib `applyHandleDrag` 锚点语义加交叉翻转：拖动手柄越过静止手柄时归属权交换。目前仅做 `coerceIn` 夹取（无翻转），翻转缺口待补。落点：`TerminalSurface.kt`。

## 双标志协议

渲染线程每帧消费两个独立标志。两者正交：新输出可将视口复位到底部，非输出变更只重绘、永不复位视口。

### P1-1 `new_output`（滚动复位信号）

- 置位：PTY 摄取路径收到非空块（`OutputProcessor::process`）。空块不计数。
- 消费：渲染线程经 `take_new_output` / `consumeNewOutput` 单次读清。
- 形态：旁路标志，不是队列事件——输出洪峰下排队事件会被淹没，故与 `pollAll()` 循环并列读取。
- 语义：为真表示本帧有新终端输出，视口可复位到底部。

### P2-1 `dirty`（内容脏信号）

- 置位：变更延迟渲染输入的 JNI 入口（`setSearchHighlights` / `clearSearchHighlights` / `setFontSizeInPlace`）。
- 消费：渲染线程在 `render_inner` 内一次 `getAndSet(false)` 交换取走。
- 语义：高亮/字号变更只触发重绘，永不复位视口。
- 同时是 Kotlin 渲染循环的唤醒信号（UI 调用方 `notifyRender()` + 兜底 latch 节奏）。

## 旧文档对照

| 旧文档 | 去向 |
| --- | --- |
| `research-zed-port.md` | 超链接识别、DECCKM 与方向键编码、CSI 27 修饰键编码 |
| `research-haven.md` | DECCKM 与方向键编码 |
| `research-ghostty-android-extra.md` | 字体槽位 |
| `research-wgpu-example.md` | 离屏验证帧 |
| `research-termlib.md` | 选择手柄拖动 |
| `dual-flag-protocol.md` | 双标志协议（原文并入） |
| 其余研究报告与索引 | 未落地，不收录 |
