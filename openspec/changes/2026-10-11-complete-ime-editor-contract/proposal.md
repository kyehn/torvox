# Proposal

## Why

本仓的 `InputConnection` 只覆写了 `setComposingText` / `finishComposingText` /
`commitText` / `deleteSurroundingText` / `sendKeyEvent` 五项，其余编辑契约全部继承
`BaseInputConnection` 的默认实现。`DESIGN.md` 要求「支持全功能输入法，不限制输入法
特性」，而这些默认值恰好把本仓未实现的编辑契约**表达成平台的既定事实**：输入法据此
判定字段不支持批量编辑、取不到任何文本、拒绝改写已提交的词、放弃校正——表现为终端内
输入法的静默功能缺失，或输入法因看不到自己刚提交的内容而整词重发、终端里出现重复文本。

Android 14 起新增的 `replaceText` 更直接：其默认实现改的是 `BaseInputConnection`
自持的 editable，而本仓从不读它、也不覆写 `getEditable`，PTY 收不到任何字节——
字符静默丢失，且本仓无任何日志或状态能表明丢失发生。

## 本轮实测（依据）

平台接口面以 `javap` 取自本机 SDK（API 33 / 36 / 37.1）：

| 事实 | 依据 |
| --- | --- |
| `InputConnection.replaceText` 是 `default` 方法，可覆写；API 33 运行时无此方法，故覆写不破坏 minSdk | `javap android.view.inputmethod.InputConnection`（36 / 37.1 命中 `public default`，33 命中 0） |
| `updateSelection` 在 API 33 / 36 / 37 的**公开** `InputConnection` 接口上均不存在（框架内部 @hide） | 三次 `javap` 均命中 0；故该方法不可由应用覆写 |
| `beginBatchEdit` / `commitCorrection` / `getExtractedText` / `requestCursorUpdates` / `setComposingRegion` / `setSelection` 均为公开 abstract 方法，默认可用实现存在 | `javap android.view.inputmethod.BaseInputConnection` |

本仓现状（`android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt` 的
`InputConnection` 匿名类）：

| 方法 | 覆写 | 本仓实测行为 |
| --- | --- | --- |
| `setComposingText` / `commitText` / `finishComposingText` / `deleteSurroundingText` / `sendKeyEvent` | 是 | 既有差分与抑制窗口语义（N1-24 / N1-27）保持不动 |
| `getEditable` | 否 | `BaseInputConnection` 自持 editable，PTY 无感知 |
| `replaceText` | 否 | 继承默认实现，编辑不落 PTY |
| `getExtractedText` / `getTextBeforeCursor` / `getTextAfterCursor` / `getSurroundingText` | 否 | 一律取自未同步的 editable |
| `beginBatchEdit` / `endBatchEdit` / `commitCorrection` / `setComposingRegion` / `requestCursorUpdates` / `setSelection` | 否 | 继承默认返回 |

另一处独立缺口：`handleKeyEvent(event) = onKeyDown(event.keyCode, event)`
（`TerminalSurface.kt:2619`）只取 `keyCode`。`ACTION_MULTIPLE` 事件的 `keyCode` 为
`KEYCODE_UNKNOWN`，正文在 `event.characters`——日文硬键盘一次送多个字符时本仓产出空，
字符静默丢失。

## What Changes

- **组字基线成为唯一镜像并对外只读**：本仓已在维护的 `composingBuffer` 是输入法文本的
  唯一真实镜像（`setComposingText` 写入、`commitText` / `finishComposingText` 清空）。
  取文类接口（`getTextBeforeCursor` / `getTextAfterCursor` / `getExtractedText` /
  `getSurroundingText`）一律只反映该基线，基线为空即返回空；MUST NOT 臆造整行历史——
  终端的文本真相在 PTY 与回滚区，不在输入法镜像。
- **`replaceText` 重放到组字基线**：按框架给出的 `start` / `end` 范围回退相应码点数再
  追加 `text`，与 `commitText` 落到同一出口，使新编辑路径不产生静默丢字；范围不落在
  组字基线内时返回 false，不谎称接受。
- **`setComposingRegion` 重锚已提交词**：输入法点建议改写一个已提交的词时，把该范围按
  `ComposingDiff` 重锚到组字基线后走同一差分出口；范围不在基线内则返回 false。
- **批量编辑如实受理**：`beginBatchEdit` / `endBatchEdit` 返回 true 并以嵌套计数配对
  （输入法把 begin 被拒判定为「字段不支持批量编辑」）。
- **光标位置如实回报**：`requestCursorUpdates` 受理 `CURSOR_UPDATE_IMPLICIT`，并以组字
  光标回报 `CursorAnchorInfo`，坐标取自终端网格单元格，使组字窗口落在真实光标格。
- **`setSelection` 的位移折算为方向键**：输入法（如 Gboard 空格滑动）用 `setSelection`
  表达光标移动时，把相对位移折算为 `DPAD_LEFT` / `DPAD_RIGHT` 送 PTY；绝对位置对终端
  无意义，MUST NOT 回写 editable。
- **`ACTION_MULTIPLE` 逐码点提交**：`sendKeyEvent` 遇该动作时按 `event.characters` 的
  码点逐个提交，不经 keyCode 映射。
- **每项「受理」声明以探针为准**：凡返回 true 的接口，其对应输入 MUST 先由探针证明
  确实到达 PTY；未证明者返回 false（宁缺勿谎）。

## Capabilities

### New Capabilities

- `ime-editor-contract`: 输入法编辑契约的履约边界——组字基线作为唯一镜像、取文与
  替换类接口的履约范围、批量编辑受理、组字光标位置回报、硬键盘多字符提交，以及
  「只回报已做到之事」的诚实性约束

### Modified Capabilities

无。已逐条核对 `ime-small-write-direct`（只约束小写入的发送延迟）、
`backspace-input-wake`（只约束输入触发的渲染唤醒）、`ime-animation-smoothness`
（只约束输入法引起的高度变化与视觉稳定）、`modifier-bar-sticky-encoding`（只约束
修饰键编码）：本变更不改它们的任何行为要求。

## Non-goals

- 不引入整行文本镜像或输入法编辑器模型。终端的文本真相在 PTY 与回滚区；输入法镜像
  只服务于「输入法必须能看见自己刚写入的内容」这一条契约。
- 不实现 `updateSelection`：已实测该方法在公开 `InputConnection` 接口上不存在
  （框架内部 @hide），应用无法覆写。
- 不改 `setComposingText` / `commitText` 的抑制窗口与差分语义（N1-24 / N1-27 不动）。
- 不改 `EditorInfo` 属性：`TerminalEditorInfo.kt` 现有取值已有充分论证。
- 不引入 `regex` 之外的依赖，不引入网络与额外权限。

## Impact

- Kotlin：`ui/TerminalSurface.kt`（`InputConnection` 匿名类、`handleKeyEvent`），
  组字基线的取文/替换逻辑宜下沉到 `input/` 或 `ui/` 的独立文件以便单测。
- 测试：`testDebugUnitTest`（Robolectric sdk=33 覆盖取文与批量编辑等公开接口），
  `replaceText` 因 API 34+ 须在 API 34+ 上验证（Robolectric 提 sdk 或仪器测试）。
- 规范：新增 `ime-editor-contract`；`upstream-alignment` 的「同类安卓终端复查台账」
  已登记 `CocaKova/charon` 为「需改造后采纳」，本变更取其 `TextSelection` 之外的
  IME 契约经验，但 `updateSelection` 一项经实测不可实现、不采纳。
