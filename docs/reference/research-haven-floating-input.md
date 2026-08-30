# 深度研究：Haven 浮动文本输入 — 亲自逐文件阅读补充

> 研究日期：2026-08-06 | 项目链接：https://github.com/GlassHaven/Haven
> 前置：`research-haven.md`（SelectionToolbar smartCopy，亲自精读）；本文补充 **FloatingTextInputDialog.kt**（759 行，亲自精读核心）

## 1. 文件：`feature/terminal/src/main/kotlin/sh/haven/feature/terminal/FloatingTextInputDialog.kt`

### 1.1 关键坑位：Popup 内 ActionMode 静默 no-op（:202-226 注释，极其重要）

```kotlin
/**
 * ⚠️ On the Compose version this app pins (foundation 1.11.4 via compose-bom
 * 2026.06.01) this class is normally DORMANT: the compiled default of
 * `ComposeFoundationFlags.isNewContextMenuEnabled` is `true` ...
 * a popup window has no real DecorView, so `startActionMode(TYPE_FLOATING)`
 * silently no-ops there
 */
```

**核心教训**：**Popup/PopupWindow 没有真实 DecorView，`startActionMode(TYPE_FLOATING)` 在其中静默 no-op**——菜单永远不会出现且无报错。Haven 的解决方案：菜单在**同一 Popup 窗口内**作为 sibling 渲染（`TextContextMenuOverlay`），绝不再开第二个窗口。

**torvox 对比**：torvox 的选择菜单 `startActionMode` 在 `TerminalSurface`（普通 View 层级）调用，不在 Popup 内——**无此问题**。但若未来把菜单移入 Compose Popup 必须警惕此坑。

### 1.2 Compose 新 context menu 系统（:285-339）

- foundation 1.11.4 `ComposeFoundationFlags.isNewContextMenuEnabled = true`（作者反汇编 AAR 字节码验证，非文档）：TextField 走 `TextContextMenuProvider` 系统而非旧 `TextToolbar`
- `FloatingInputTextContextMenuProvider`（:304-339）：`suspendCancellableCoroutine` 挂起 → 请求存 snapshot state → 菜单在**同一窗口**渲染 → session.close() 恢复挂起
- **MUST 同时提供 LocalTextContextMenuToolbarProvider + LocalTextContextMenuDropdownProvider**（:332-339）：`ProvideDefaultPlatformTextContextMenuProviders` 会 re-provide 平台默认值遮蔽我们的实现，除非两者都非空

**torvox 对比**：torvox 的文本选择不走 Compose TextField（自绘 TerminalSurface + 系统 ActionMode），此系统不适用。但**"新 context menu 系统取代旧 TextToolbar"的迁移模式**值得记录（Compose 生态演进）。

### 1.3 浮动文本输入（:346+）

`FloatingTextInputDialog(text, onTextChange, onSend, onDismiss)`：
- **可拖动（moveBy dx/dy）+ 可缩放（resizeBy dw/dh）**的浮动窗口
- 完整 IME 能力（自动纠正、滑行输入、语音输入、光标移动）编辑**整条命令**，然后一次发送（onSend 由调用方做 bracket-paste 感知注入）
- **状态提升**：text 由 TerminalScreen 的 per-tab draft map 持有（tab 切换/旋转/进程死亡不丢草稿）；**仅成功发送才清草稿**（dismiss 不清）
- 菜单项（Cut/Copy/Paste/Select-all）由 TextField 自身贡献（addBasicTextFieldTextContextMenuComponents），provider 不加自己的

**torvox 对比**：torvox **无浮动文本输入**——终端里输入长命令时只能靠 IME 直接写 cell。这是 UX 增强候选（P2，用户未要求）："浮动命令编辑窗口 + 一次发送"解决终端输入法组合/纠正困难的痛点。

## 2. 与 SelectionToolbar.kt 的协作

- `TextSelectionMenuRequest`（:189-197）：rect + onCopy/onPaste/onCut/onSelectAll 回调——选择菜单请求模型
- FloatingInputTextToolbar（:227-260）：旧 TextToolbar 实现（休眠路径，仅捕获请求到 snapshot state）
- 浮动输入菜单渲染在 Popup 窗口内（TextSelectionMenu/TextContextMenuOverlay 是同一窗口的 sibling）

## 3. 结论

Haven 的浮动文本输入有两个可吸收点：
1. **坑位记录（P1）**：Popup 内 startActionMode(TYPE_FLOATING) 静默 no-op——写入 torvox 代码注释（TerminalSurface 选择菜单处），防未来误用
2. **浮动命令编辑（P2 候选）**：完整 IME 编辑 + bracket-paste 一次发送 + per-tab 草稿持久化——终端 UX 增强
3. Compose context menu 系统演进记录（P2，文档级）
