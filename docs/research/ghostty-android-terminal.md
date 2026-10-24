# ghostty-android-terminal 深度研究报告

对 [sylirre/ghostty-android-terminal](https://github.com/sylirre/ghostty-android-terminal) 的深度调研结论。该项目是与本项目同引擎（`libghostty-vt`）的 Android 终端，DESIGN 指定其为像素滚动、鼠标操作、单码位整形等特性的参考实现，因此本研究重点是「同引擎集成方式的对照」与「上游 C API 使用模式的印证」。

## 概况

四层架构：Java UI（`TerminalView` 约 2500 行 Canvas 网格渲染）→ `libterm.so` JNI（`terminal_jni.c` 约 2100 行 + `pty_jni.c` 约 400 行）→ `libghostty-vt.a`（Zig 预编译 per ABI，pinned commit）→ arm64chroot/arm64emu（自研用户态/全系统模拟器，本项目禁止清单内）。无 JVM 单测，全部为 instrumented 集成测试。

## 关键实现要点

### libghostty-vt 集成模式

- **「Effects are polled, not pushed」**：write-pty/title/bell 回调在 native 侧缓冲，`feed()` 返回后由 Java 拉取——完全避免 `AttachCurrentThread` 上调
- 快照拉平为 flat arrays（codepoint/fg ARGB/bg ARGB/attrs），颜色在 native 解析为最终 RGB，Java 侧无调色板逻辑；缓冲不足走 grow-and-retry 容量契约
- `ghostty_key_encoder_*` 编码按键，每次编码前用 `setopt_from_terminal` 同步 DECCKM 等模式
- 会话以最终网格尺寸启动（初始 winsize 即含 `ws_xpixel`/`ws_ypixel`），布局完成才 spawn——因为 mksh 收到 SIGWINCH 会擦掉提示符且不重印，猜尺寸再 resize 会留给用户一个空屏直到首次按键

### 渲染

Canvas `drawText` 按同色同属性连续段批量绘制；CJK 宽字符占两格、尾格跳过；grapheme 簇走带外溢出缓冲，行级 `GHOSTTY_ROW_DATA_GRAPHEME` 标志门控使普通行零开销；完整 grapheme 分段（天城文合字、ZWJ emoji）依赖 DEC mode 2027 且默认关（列宽核算会与 libc `wcwidth` 失配）。bidi 与歧义宽明确不支持——引擎只输出二值 NARROW/WIDE，歧义宽需改 Ghostty 源码重编。

### 选择与搜索

选区装进引擎 `GHOSTTY_TERMINAL_OPT_SELECTION`，由 Ghostty 转为 tracked grid refs，随滚动、新输出、resize/reflow 自动跟随，Java 侧零簿记；快照以 fg/bg 反相呈现选区，渲染器无需改动。搜索因引擎早期版本无内建而自建：native 全屏扫描（回滚+活动区、软换行拼逻辑行）、命中装进 `MAX_MATCHES` ring 只保留最新、set+step 在单锁内完成防 PTY 竞争。

### 其他

OSC 52/9;4 引擎不回调，以 side-scanner 在 PTY 流被动扫描（1 MiB 上限）；修饰键符号用矢量 drawable 而非 font glyph（防 OEM 字形 tofu）；富输入为可选 `TYPE_CLASS_TEXT` + 本地 Editable 镜像（最长公共前缀回退，alt-screen 自动禁用）；`TYPE_NULL` InputConnection 为默认路径。

## 与本项目的对照

### 已对齐项（同引擎同模式印证）

- **poll 拉取模式**：本项目 `pty_write_responses` 缓冲 + `pollEvent` 派发，与其「polled, not pushed」同构
- **winsize 像素尺寸**：本项目 `Pty::set_pixel_size` 经 TIOCSWINSZ 写 `ws_xpixel`/`ws_ypixel`，同源
- **选择走引擎**：本项目同样经 `GHOSTTY_TERMINAL_OPT_SELECTION` 用 tracked refs，选区跟随由上游承担
- **bidi/歧义宽不支持**：同引擎的固有边界，其文档论证（渲染层改宽会与列核算失配）可直接引用为依据

### 参考点归属

原「可参考点」四项已全部核结，条目随解决即从本文档移除：

- mksh SIGWINCH 不变量：`startup-grid-resize` 规定首次 resize 前不把提示符喂给 shell。
- grapheme 行级标志门控：本仓 `CellData` 直接携带 `grapheme_extra`，无带外溢出缓冲层。
- key encoder 前同步 setopt：VT 线程每次编码前调用 `set_options_from_terminal`。
- 文档质量（写清为什么这么做）：本仓以 openspec 变更的 design.md 承担该写法。

### 不适用项

- arm64chroot/arm64emu、rootfs 资产与备份还原（内嵌模拟器与会话持久化均在禁止清单）
- 自建 native 搜索（本项目 text-search 规格已定为上游 `search::Search` 单一来源，不自建匹配器；其 ring 上限方案在上游 50k 命中窗口下无需复刻）
- OSC 52/9;4 side-scanner、富输入 Editable 镜像、tab 标签页 UI（DESIGN 未要求或路径不同）
- Canvas 渲染全套（本项目 wgpu/Vulkan 直绘，不存在其批量 drawText 优化问题域）

## 路线印证

两参考项目的渲染都在主线程 View 管线内（Termux 的 invalidate、g-a-t 的 onDraw 快照），网格数据在 Java 侧、每帧过 JNI 拉取。本项目独立渲染线程 + wgpu 直绘 ANativeWindow、网格归属 Rust、FFI 只传轻量事件的架构，是两个对照项目都没有走的路——它们的每帧 JNI 拉平快照（flat arrays + grow-and-retry）正是本项目 DESIGN「无沉重网格数据跨越 FFI 边界」条款所规避的成本。
