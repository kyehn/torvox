# 深度研究：termlib IME 与 Compose 模式 — 亲自逐文件阅读补充

> 研究日期：2026-08-06 | 项目链接：https://github.com/connectbot/termlib
> 前置：`research-termlib.md`（SelectionManager，亲自精读）；本文补充 **ImeInputView.kt（379 行）+ ComposeController.kt（57 行）**

## 1. ImeInputView.kt（379 行，亲自精读核心）

### 1.1 设计（:40-67）

不可见 `View` + 自定义 `TerminalInputConnection`（BaseInputConnection 子类），依赖注入（`inputMethodManager`/`onUpdateSelection`/`onRestartInput` 可测试）。

### 1.2 两种输入模式（:91-121，核心设计）

```kotlin
override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
    outAttrs.imeOptions = outAttrs.imeOptions or
        EditorInfo.IME_FLAG_NO_EXTRACT_UI or
        EditorInfo.IME_FLAG_NO_ENTER_ACTION or
        EditorInfo.IME_ACTION_NONE
    if (isComposeModeActive) {
        // Compose mode: 允许语音输入和 IME 建议
        // TYPE_CLASS_TEXT 保持建议条（含麦克风按钮）可见。
        // fullEditor=true 使 BaseInputConnection 提供真实 Editable，
        // 让 getExtractedText() 返回非 null（Gboard 语音输入必需）。
        outAttrs.inputType = EditorInfo.TYPE_CLASS_TEXT
    } else {
        // 普通终端模式：
        // TYPE_NULL | VISIBLE_PASSWORD | NO_SUGGESTIONS
        // VISIBLE_PASSWORD 显示带数字行的密码风格键盘（文本我们自己显示）
        outAttrs.inputType = EditorInfo.TYPE_NULL or
            EditorInfo.TYPE_TEXT_VARIATION_PASSWORD or
            EditorInfo.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
            EditorInfo.TYPE_TEXT_FLAG_NO_SUGGESTIONS
    }
    return TerminalInputConnection(this, isComposeModeActive)
}
```

**关键洞见**：
- **普通模式用 `TYPE_TEXT_VARIATION_PASSWORD`**——密码风格键盘自带数字行（终端常用），`VISIBLE_PASSWORD` 保持文本可见（终端自己显示）
- **compose 模式才允许建议/语音**——平时禁用自动纠正（避免 IME 篡改终端输入），输入长命令时才启用
- `fullEditor=true`（BaseInputConnection 构造参数）→ 真实 Editable → `getExtractedText()` 非 null → **Gboard 语音输入必需**

**torvox 对比**：torvox 的 IME 配置（TerminalScreen/TerminalSurface 的 onCreateInputConnection）是否用了 PASSWORD 变体？若没有，终端键盘缺数字行。**这是可核查点**。

### 1.3 TerminalInputConnection（:154-320）

- `setComposingText`（:163-188）：compose 模式支持语音部分结果；**防 IME 重放刚提交的组合**（:178 注释："Some IMEs replay the just-submitted composition through setComposingText"）
- `finishComposingText`（:227）
- `deleteSurroundingText`（:237-259）：发 KEYCODE_DEL
- `sendKeyEvent`（:263）：Enter 等键
- `commitText`（:299-320）：**只有 compose 模式才走 commitText 路径**（:308-311 注释：普通模式字符"ONLY arrive via commitText because they have no direct KEYCODE"——即语音/自动纠正字符只能通过 commitText 到达）

### 1.4 IME 显示控制（:69-89）

- `showIme()`：`requestFocus() + showSoftInput(SHOW_FORCED)`——**注释明确"比 SoftwareKeyboardController 更可靠"**（:69）
- `hideIme()`：hideSoftInputFromWindow
- **`onDetachedFromWindow` 强制隐藏**（:84-89）：防 SHOW_FORCED 在 Activity 销毁后卡住键盘

**torvox 对比**：torvox 用 Compose 的 `SoftwareKeyboardController`？若是，SHOW_FORCED 更可靠的经验值得记录（zelland 也用隐藏 EditText + showSoftInput）。

## 2. ComposeController.kt（57 行，完整阅读）

```kotlin
interface ComposeController {
    val isComposeModeActive: Boolean
    fun startComposeMode()   // 清空选区
    fun stopComposeMode()    // 丢弃缓冲文本
    fun toggleComposeMode()
    fun getComposedText(): String
    val pendingDeadChar: Int  // 待组合的死键（重音），0=无
}
```

**Compose 模式**：本地缓冲输入文本，在光标处显示为 overlay；Enter 提交、Esc 取消。`pendingDeadChar` 支持死键（重音组合）。

**torvox 对比**：torvox **无 compose 模式**。价值场景：① 语音输入（长命令）；② 死键/重音组合（欧洲语言）；③ 自动纠正（中文输入法需要）。**这是 P2 功能候选**——尤其中文用户用拼音输入法时，普通终端模式禁用建议会导致候选条消失。

## 3. 结论

termlib IME 有两个可吸收点：
1. **普通模式 PASSWORD 键盘变体（P1 核查）**：torvox 键盘是否带数字行
2. **compose 模式（P2 候选）**：本地缓冲 + overlay + Enter 提交——解决语音/建议/中文输入法问题
3. **showIme SHOW_FORCED 可靠性 + onDetached 强制隐藏（P1 记录）**
4. **Gboard 语音需要 fullEditor=true（P2 记录）**
