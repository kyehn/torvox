# 深度研究：termux-app v0.119.0-beta.3

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/termux-app`（depth 1，branch `github-releases/v0.119.0-beta.3`）
> 参考索引：本项目 `docs/reference-projects.md` §6 选择系统三档（termux = 档位 2）
> 语言：Java（终端核心），197 个 Java/Kotlin 文件

## 1. 项目定位

Termux 是 Android 上最成熟的终端模拟器（GNU/Linux 环境）。本研究的核心价值在于其**文本选择系统**（`terminal-view` 模块），这是 termux 区别于其他 Android 终端的关键 UX。

模块结构：
```
termux-app/
├── app/                 # 主应用（Activity、安装、bootstrap）
├── terminal-emulator/   # TerminalEmulator（VT 解析、TerminalBuffer、TerminalSession）
├── terminal-view/       # TerminalView（渲染、手势、文本选择）← 核心研究目标
└── termux-shared/       # 共享工具（Term.java 等）
```

## 2. 文本选择系统（核心参考）

### 2.1 总体架构

文件：`terminal-view/src/main/java/com/termux/view/textselection/`
| 文件 | 行数 | 职责 |
|------|------|------|
| `TextSelectionCursorController.java` | 407 | 选择状态机：显示/隐藏/渲染/拖动/菜单 |
| `TextSelectionHandleView.java` | 352 | 选择手柄（PopupWindow 承载的自绘 View） |
| `CursorController.java` | 55 | 接口 |

### 2.2 菜单锚定：`onGetContentRect`（核心算法）

`TextSelectionCursorController.java:194-215`（API 23+ 用 `ActionMode.Callback2` + `TYPE_FLOATING`）：

```java
public void onGetContentRect(ActionMode mode, View view, Rect outRect) {
    int x1 = Math.round(mSelX1 * terminalView.mRenderer.getFontWidth());
    int x2 = Math.round(mSelX2 * terminalView.mRenderer.getFontWidth());
    int y1 = Math.round((mSelY1 - 1 - terminalView.getTopRow()) * terminalView.mRenderer.getFontLineSpacing());
    int y2 = Math.round((mSelY2 + 1 - terminalView.getTopRow()) * terminalView.mRenderer.getFontLineSpacing());
    if (x1 > x2) { int tmp = x1; x1 = x2; x2 = tmp; }
    int terminalBottom = terminalView.getBottom();
    int top = y1 + mHandleHeight;
    int bottom = y2 + mHandleHeight;
    if (top > terminalBottom) top = terminalBottom;
    if (bottom > terminalBottom) bottom = terminalBottom;
    outRect.set(x1, top, x2, bottom);
}
```

要点：
1. 坐标 = 单元格列/行 × 字体像素（`getFontWidth()`/`getFontLineSpacing()`）
2. `getTopRow()` 处理滚动偏移（选区在 scrollback 中时）
3. 上下各扩展 1 行（`mSelY1 - 1`/`mSelY2 + 1`）——给菜单留呼吸空间
4. 加 `mHandleHeight`（手柄高度偏移，菜单浮在手柄上方）
5. clamp 到 `terminalBottom`（防菜单出屏）
6. 单行/多行都返回**选区矩形**，系统 FloatingActionMode 自动把菜单放到矩形上方/下方最近空间

**对本项目的意义**：torvox 当前 `SelectionActionCallback` 是 `ActionMode.Callback`（无 `onGetContentRect`），菜单只能 TYPE_FLOATING 但锚定整个 view。应迁移到 `Callback2`（Kotlin 写法 `object : ActionMode.Callback2() {...}` 带括号，Java 匿名类在 Kotlin 中基类是抽象类时必须带 `()`）实现同样的锚定算法。termux 用 `Build.VERSION.SDK_INT < M` 分支兼容旧版本（minSdk 33 的本项目不需要）。

### 2.3 长按选词：`setInitialTextSelectionPosition`

`TextSelectionCursorController.java:93-108`：

```java
public void setInitialTextSelectionPosition(MotionEvent event) {
    int[] columnAndRow = terminalView.getColumnAndRow(event, true);
    mSelX1 = mSelX2 = columnAndRow[0];
    mSelY1 = mSelY2 = columnAndRow[1];
    TerminalBuffer screen = terminalView.mEmulator.getScreen();
    if (!" ".equals(screen.getSelectedText(mSelX1, mSelY1, mSelX1, mSelY1))) {
        // Selecting something other than whitespace. Expand to word.
        while (mSelX1 > 0 && !"".equals(screen.getSelectedText(mSelX1 - 1, mSelY1, mSelX1 - 1, mSelY1))) {
            mSelX1--;
        }
        while (mSelX2 < terminalView.mEmulator.mColumns - 1 && !"".equals(screen.getSelectedText(mSelX2 + 1, mSelY1, mSelX2 + 1, mSelY1))) {
            mSelX2++;
        }
    }
}
```

要点：
1. 空单元格（空格）→ 不扩展，单格选择（后续显示 paste-only 菜单）
2. 非空白 → 向两侧扩展到空字符（`""`，即行尾）为止
3. **没有** URL 智能识别（torvox 的 `SelectionExpander` 比 termux 激进，有 URL/引号处理）

对比 torvox：`SelectionExpander.expandBounds(line, col)` 支持 URL、引号修剪、单词收缩，功能上是 termux 的增强版；但 termux 用 `screen.getSelectedText()` 直接在终端缓冲上查字符（含宽字符宽度感知），torvox 用 `scrollbackLine(row)` 字符串。行为差异：termux 扩展停在**任意空字符**（空格/制表），torvox 停在标点边界。termux 的语义更符合"单词"直觉。

### 2.4 手柄拖动：`updatePosition`

`TextSelectionCursorController.java:218-306`：

```java
public void updatePosition(TextSelectionHandleView handle, int x, int y) {
    TerminalBuffer screen = terminalView.mEmulator.getScreen();
    final int scrollRows = screen.getActiveRows() - terminalView.mEmulator.mRows;
    if (handle == mStartHandle) {
        mSelX1 = terminalView.getCursorX(x);
        mSelY1 = terminalView.getCursorY(y);
        if (mSelX1 < 0) mSelX1 = 0;
        if (mSelY1 < -scrollRows) mSelY1 = -scrollRows;
        else if (mSelY1 > terminalView.mEmulator.mRows - 1) mSelY1 = terminalView.mEmulator.mRows - 1;
        // 阻止反向跨越：start 不能超过 end
        if (mSelY1 > mSelY2) mSelY1 = mSelY2;
        if (mSelY1 == mSelY2 && mSelX1 > mSelX2) mSelX1 = mSelX2;
        // 边缘滚动：手柄到 viewport 边缘时滚动 scrollback
        if (!terminalView.mEmulator.isAlternateBufferActive()) {
            int topRow = terminalView.getTopRow();
            if (mSelY1 <= topRow) { topRow--; if (topRow < -scrollRows) topRow = -scrollRows; }
            else if (mSelY1 >= topRow + terminalView.mEmulator.mRows) { topRow++; if (topRow > 0) topRow = 0; }
            terminalView.setTopRow(topRow);
        }
        mSelX1 = getValidCurX(screen, mSelY1, mSelX1);
    } else {
        // mEndHandle 对称逻辑
    }
    terminalView.invalidate();
}
```

要点：
1. **范围钳制**：行限制在 `[-scrollRows, mRows-1]`（scrollback 上界 + 屏幕下界）
2. **顺序保持**：start 永远 ≤ end（同行时列也保持）
3. **边缘滚动**：选区行到达 viewport 顶部/底部时滚动一行（`topRow--/++`），且交替缓冲（vim 等）不滚动
4. **宽字符吸附**：`getValidCurX`（307-337）用 `WcWidth.width()` 计算每个字符显示宽度，光标落在宽字符中间时吸附到字符右边界

对比 torvox：torvox 有 `edgeScrollHandler`（定期滚动），但没有"拖动即滚一行"的即时反馈；torvox 的 `SelectionExpander`/`isCellEmpty` 走 `scrollbackLine` JNI 查询，无宽字符吸附逻辑（CJK 宽度由 Rust 侧 `is_wide` 处理，但手柄列号吸附缺失）。

### 2.5 手柄视图：`TextSelectionHandleView`

要点：
- `PopupWindow` + `TYPE_APPLICATION_SUB_PANEL`（API 23+）+ `setSplitTouchEnabled(true)` + `setClippingEnabled(false)` + `setBackgroundDrawable(null)` + `setAnimationStyle(0)`（`:44-60`）
- 系统 drawable：`text_select_handle_left_material` / `text_select_handle_right_material`（`:105-108`）
- 热点（hotspot）偏移：LEFT 手柄 `hotspotX = width * 3/4`，RIGHT `width/4`（`:78-92`）——手柄指尖对准手柄图像靠选区侧
- `mTouchOffsetY = -mHandleHeight * 0.3f`（`:93`）
- `isPositionVisible()`：手柄位置不可见时隐藏（防越界）

对比 torvox：torvox 手柄同样用 PopupWindow + APPLICATION_SUB_PANEL + 锚点拖拽，但 drawable 是自绘的（非系统 material drawable），hotspot 偏移实现不同（anchor 方式）。termux 用系统 drawable 保证与系统选择手柄外观一致——**用户"菜单必须和系统样式一致"要求的正确做法**。

### 2.6 防误关：`hide()` 300ms 保护

`TextSelectionCursorController.java:57-80`：show 后 300ms 内的 hide 请求被忽略（防止长按后的立即抬起误关）。

### 2.7 手势：`TerminalView.java`

- `onLongPress`（:255-260）：`mClient.onLongPress(event)` 优先（应用可消费），否则 `performHapticFeedback(LONG_PRESS)` + `startTextSelectionMode(event)` —— 触觉反馈是标准 UX
- `onDoubleTap`（:244-246）：**返回 false**（不实现双击选词，交给 zoom 手势识别）—— 与 torvox 的"双击选词/三击选行"不同，termux 没有多击选择
- `getColumnAndRow`（:546-551）：`column = x/fontWidth; row = (y - fontLineSpacingAndAscent)/fontLineSpacing`，`relativeToScroll` 时加 `mTopRow`
- `startTextSelectionMode`（:1407-1412）：`requestFocus()` 守卫 + `mClient.copyModeChanged(isSelectingText())` 回调（应用层可感知选择模式变化，如禁用滚动）
- `stopTextSelectionMode`（:1418-1423）：`hideTextSelectionCursors()` + 回调

对比 torvox：torvox 支持双击/三击（更激进）；termux 的 `copyModeChanged` 回调模式值得借鉴（torvox 通过 ViewModel state 实现）。

## 3. 渲染：`TerminalRenderer.java`

- 每帧：`TerminalBuffer.getScreen()` → 遍历可见行 → 逐字符 `canvas.drawText`（**CPU 渲染**，`Canvas.drawText` 每单元格）
- 颜色表：`mColors[]`（256 色索引），主题切换重建
- 选择渲染：`mCursorController.render()` 画手柄，选区反色由 `TerminalBuffer` 的 `isSelected` 状态驱动每格颜色 swap

**本项目对比**：torvox 用 wgpu GPU 渲染（`Canvas.drawText` 在本项目 AGENTS.md 中被列为禁止项）。termux 的 CPU 渲染在低端设备滚动时掉帧，torvox 的 GPU 路径是架构优势。选择反色语义一致（fg↔bg swap）。

## 4. 依赖清单

| 依赖 | 用途 | 本项目适用性 |
|------|------|--------------|
| `com.termux:terminal-emulator` | 自研 VT 解析 | 不适用（torvox 用 libghostty-vt，更完整） |
| `com.termux:terminal-view` | 自研渲染+选择 | 不适用（torvox 用 Compose+wgpu；但选择算法可移植） |
| `net.i2p.crypto:eddsa` | SSH 密钥 | 不适用（无 SSH 功能） |
| AndroidX（appcompat/recyclerview/viewpager2） | UI | 部分（torvox 用 Compose） |
| `com.termux:termux-shared` | 共享工具 | 不适用（架构不同） |

## 5. 项目文档吸收价值

- `docs/`（termux-app 仓库）：termux 有丰富的文档（README.md 900+ 行），包括 termux-exec（LD_PRELOAD 机制）、bootstrap 结构说明
- **termux-exec 机制**：`LD_PRELOAD=libtermux-exec.so` 拦截 execve，把 `/bin/sh` 等重定向到 `$PREFIX/bin`。torvox 的 bootstrap 安装已用 linker64+LD_PRELOAD 方案（见 `SecondStageRunner.kt`），与 termux-exec 同源思路

## 6. 代码注释引用（建议加入 torvox 代码）

```
TerminalSurface.kt showSelectionMenu():
// 菜单锚定参考 termux-app TextSelectionCursorController.java:194-215 (onGetContentRect)
// TYPE_FLOATING + Callback2 (API 23+)；Kotlin 需写 object : ActionMode.Callback2()
TerminalSurface.kt 手柄拖拽:
// 手柄拖动参考 termux-app TextSelectionCursorController.java:218-306 (updatePosition)
// 顺序保持 + 范围钳制 + 边缘滚动 + 宽字符吸附 (getValidCurX :307-337)
TerminalSurface.kt handleLongPress:
// 长按选词参考 termux-app setInitialTextSelectionPosition :93-108
// 空单元格→不扩展；非空白→扩展到空字符
```

## 7. 结论

termux-app 是文本选择系统的最佳参考：`Callback2.onGetContentRect` 锚定算法、手柄拖动状态机（顺序保持+边缘滚动+宽字符吸附）、PopupWindow 手柄（系统 drawable + SUB_PANEL）、300ms 防误关。torvox 的选择系统已在功能上超越（GPU 反色渲染、URL 智能扩展、多击选择），但**菜单锚定（onGetContentRect）和宽字符吸附是两个明确的移植缺口**。
