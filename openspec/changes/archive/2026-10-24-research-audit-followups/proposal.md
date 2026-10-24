# Proposal

## Why

`docs/research/` 六份调研各带「可参考点 / 行动建议」，但结论只以散文形式存在：
哪些已落地、哪些是评审口径、哪些仍缺证据，没有任何一处可核对。结果是同一批条目
会被反复「再调研一次」，而真正的缺口（键位映射少一个键、无障碍长按时长不生效、
缩窄重排没有用例）一直沉在文档里。

本轮逐份复查六个来源，把每条建议落到「已有实现 + 用例」或「明确不适用 + 理由」，
并补齐复查中发现的三个缺口。

## What Changes

- `key-input-encoding`：`KEYCODE_BREAK`（Pause/Break）此前未映射，退回按键文本为空，
  整个键静默失效；现映射到上游 `key::Key::Pause`，与 Termux `ESC [ 34 ~` 同源。
- `modifier-bar-auto-repeat`（新能力）：长按锁定与自动重复起点 MUST 取系统长按超时
  （随无障碍「长按时长」变化），越界退回 400ms；重复间隔 80ms。此前固定 400ms，
  用户在系统里调长时长后键栏不跟随。
- `terminal-state-regression`：补两条用例——主屏缩窄重排保留完整正文；备用屏缩窄
  只裁剪不重排（此前备注为最大缺口）。
- 其余条目逐条给出核验结论（`design.md` 的对照表），不产生代码变更。

## Capabilities

### New Capabilities

- `modifier-bar-auto-repeat`：修饰键栏长按阈值与重复节奏的来源与边界。

### Modified Capabilities

- `key-input-encoding`：Android 键码映射须覆盖 Termux 键表（Pause/Break 不得静默失效）。
- `terminal-state-regression`：缩窄重排与备用屏裁剪的行为判据。

## Impact

- `native/src/android/keymap.rs`、`native/src/terminal/ghostty_terminal/tests.rs`
- `android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt`
- 测试：`ModifierBarLongPressTest`（新增）、`ModifierBarTest`（注释同步）、Rust 用例两例
- 不改 `docs/specification/`；`docs/research/` 保持调研时的原文
