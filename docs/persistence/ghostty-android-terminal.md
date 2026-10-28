# ghostty-android-terminal

## 概况

四层架构：Java UI（`TerminalView` Canvas 网格渲染）→ `libterm.so` JNI（`terminal_jni.c` + `pty_jni.c`）→ `libghostty-vt.a`（Zig 预编译 per ABI，pinned commit）→ arm64chroot/arm64emu（自研用户态/全系统模拟器）。无 JVM 单测，全部为 instrumented 集成测试。

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
