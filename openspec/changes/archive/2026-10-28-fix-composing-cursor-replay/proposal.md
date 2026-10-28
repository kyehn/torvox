# Proposal

## Why

组字改写靠「退格 + 重打」复现，但**退格删的是光标前的字符**。现有实现假定光标停在
组字区末尾，直接把差分退格打到光标处：

- 组字光标停在组字区**开头**时（`setComposingText(text, 0)` 即落在这里），
  差分退格删掉的是组字区**之前已提交**的内容——用户已敲的半行命令被吃掉。
- 组字文本未变时（输入法重复提交同一候选），实现仍按「差分后光标在末尾」补发左移，
  于是凭空多出一串左方向键：光标停在组字区开头时被推到更左，停在末尾时又每次多挪
  整段长度。

即：差分的落点必须先对齐到组字区末尾，光标才能落在组字光标上。

## What Changes

- `ComposingDiff` 增加纯函数 `planReplay` / `planClear`：把「组字基线 → 目标基线」
  翻译成「右移几次 → 退格几格 → 追加什么 → 左移几次」的重放计划。
- 差分非空时：先把光标右移到旧组字区末尾，再重放差分，最后左移到目标组字光标。
- 差分为空时（组字文本未变）：不重放字节，光标只按相对位移移动。
- `TerminalSurface` 的 `setComposingText` / `deleteSurroundingText` / `commitText`
  改走该计划；提交时清空组字区同样先挪到组字区末尾。
- 新增单测覆盖：组字光标在开头时的改写、组字光标在末尾时的改写（行为不变）、
  组字文本未变时只挪光标、清空组字区的退格数。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ime-editor-contract`：新增「组字改写的光标落点必须在组字区内」的要求。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/ComposingDiff.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- `android/app/src/test/java/terminal/emulator/ui/ComposingDiffTest.kt`
- 不改 `docs/specification/`、`docs/STATUS.md` 等保护文件，不引入新依赖。
