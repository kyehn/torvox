# Proposal

## Why

`ComposingDiff.reconcile` 按 **UTF-16 下标**求最长公共前缀。两串只在代理对的
低代理位上不同时（👍 与 👎、😀 与 😁 都是同一高代理位加不同低代理位），公共前缀
会落在代理对**中间**：

- 前缀长度 = 1（只匹配到高代理位）；
- 待擦除段 = 低代理位半个字符 → 退 1 格；
- 待追加段 = 另一个低代理位 → 半个 UTF-16 码元。

半个码元编不出合法 UTF-8，编码器把它替换成 U+FFFD。于是候选区从「👍」切到
「👎」时，终端收到的是「退格 + 乱码」，而不是「👎」——用户看到的是一串问号，
不是他选的 emoji。

退格数按码点计是对的（半个码元算 1 个码点），错的是**切分点**：切分必须落在
码点边界上，否则追加的是半个字符。

## What Changes

- `ComposingDiff.reconcile` 的公共前缀改为按**码点**推进：比较两个串在同一
  UTF-16 下标处的码点，相同则整码点前进（`Character.charCount`）。
- 切分点因此永远落在码点边界，待擦除段与待追加段都是完整码点序列。
- 其余语义不变：退格数仍按码点计，追加仍是公共前缀之后的完整后缀。
- 新增单测：同高代理位不同低代理位（👍 → 👎）必须整字替换，不产生半个码元。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ime-editor-contract`：组字差分的公共前缀必须按码点边界切分。

## Impact

- `android/app/src/main/java/terminal/emulator/ui/ComposingDiff.kt`
- `android/app/src/test/java/terminal/emulator/ui/ComposingDiffTest.kt`
- 不改 `android/app/src/main/` 的其余部分，不改 `docs/specification/`、
  `docs/STATUS.md` 等保护文件，不引入新依赖。
