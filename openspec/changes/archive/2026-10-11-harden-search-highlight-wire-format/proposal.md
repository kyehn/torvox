# Proposal

## Why

搜索高亮是本仓**唯一**由 Kotlin 侧打包字节流、经 JNI `unsafe` 入口交给 Rust 解析的跨语言契约（`NativeBridge.setSearchHighlights`，`native/src/android/ffi.rs:2761`）。该入口的字节解析当前有两处真实问题：

**一、书面契约与实际行为矛盾。** `TerminalScreen.kt:674-675` 注释写「其余命中项：低于 128
反色阈值的 selection_background 着色——可见覆盖层，**不反色**」，而同一行的
`SearchHighlightColors.OTHER_MATCH_ALPHA = 160` 与 Rust 侧
`SEARCH_HIGHLIGHT_SWAP_ALPHA_THRESHOLD = 128`（`cell_builder.rs:15`）意味着
`160 >= 128`，**所有命中项都会反色**。三方证据一致证明实际行为是「反色」：
`SearchHighlightColors.kt:13` 的注释写「同样反色」、`native/src/render/tests.rs:1501`
的断言写 `alpha=160 must swap foreground/background like every match`。即
`TerminalScreen.kt` 的注释是唯一说反的一方，它把后续维护者引向「非当前命中不反色」
的错误心智模型。

**二、畸形字节流被静默吞掉，且该解析路径零测试覆盖。** 现有实现在四种畸形输入下
静默返回/中断：长度不足 4 字节（`return Ok(())`）、计数为负（`.max(0)` 抹平）、
尾部残缺记录（`break`）、计数超出实际记录数（封顶后静默截断）。这违反 DESIGN
「尽早抛出错误，不要掩盖错误」与 TESTING「不隐藏错误」。该入口还是唯一接收
调用方字节数组的 unsafe 边界，却没有一条断言守着它。

对照同类项目 termux-ghostty（`terminal-emulator/src/test/java/com/termux/terminal/
ScreenSnapshotE2ETest.java`）的 F6「Out-of-Bounds & Malformed Input Hardening」：
它逐类断言畸形载荷被拒绝为安全异常而非越界崩溃。但其**往返类**用例（native 序列化
→ Java 反序列化 → 比对）属自验证，按本仓 `upstream-alignment` 的
「自验证的对照断言不得反向采纳」不予采纳；只有「拒绝畸形」这组期望值锚定安全契约，
可以采纳为判据来源。

## What Changes

- `TerminalScreen.kt`：订正「其余命中项不反色」的注释为实际语义（全部命中项按 alpha
  阈值决定是否反色，当前项靠不透明度区分而非靠反色与否）。
- `ffi.rs`：把 `setSearchHighlights` 的字节解析抽为纯函数
  `parse_search_highlights(bytes) -> Result<Vec<SearchHighlight>, SearchHighlightError>`，
  枚举全部畸形情形（`TooShort` / `NegativeCount` / `TruncatedTail` / `CountExceedsRecords`），
  JNI 入口对每个错误 `log::error!` 出声而非静默吞掉。**不在 JNI 边界 panic/abort**
  （会杀掉宿主应用且无法单测），改为出声上报。
- `ffi.rs` 内联测试：逐个畸形类别断言**具体错误变体**（非「不 panic」这类无判别力断言），
  并断言合法载荷的字段值与 alpha 语义。

### New Capabilities

无。

### Modified Capabilities

- `text-search`：新增「搜索高亮线格式的畸形输入必须被拒绝并出声」要求。

## Impact

- `native/src/android/ffi.rs`：新增纯函数 + 错误枚举 + 内联测试。
- `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt`：仅订正注释。
- 无新增依赖、无二进制资产、无语料文件、不改线格式（不破坏既有 Kotlin 打包侧）。
- Rust 侧单测进既有 `cargo test --lib` 体系，不新增体系（TESTING 要求）。
