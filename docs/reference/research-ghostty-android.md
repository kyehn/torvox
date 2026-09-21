# 深度研究：ghostty-android-terminal (sylirre)

> 研究日期：2026-08-06 | 克隆位置：`repositories/refs/ghostty-android`（depth 1）
> 参考索引：本项目 `docs/reference-projects.md` §6 选择系统三档（ghostty-android = 档位 3）
> 语言：Java（单 Activity + 自绘 View），核心文件 `TerminalView.java`（2473 行）

## 1. 项目定位

Ghostty 官方 Android 移植（sylirre 维护，非官方主线）。**纯 Java 自绘渲染**（无 wgpu），但**选择系统是当前所有参考项目中质量最高的**——其"终端拥有选择状态、视图只做镜像"的架构与本项目（Rust GhosttyTerminal 拥有选择状态）理念一致。

## 2. 选择系统（最高质量参考）

### 2.1 架构：选择状态由模拟器拥有

`TerminalView.java:298-314` 注释明确：
> The emulator owns the selection itself (it tracks its text across scrolling and new output); this view only mirrors it

- 视图字段：`selecting`（ActionMode 生命周期）、`draggingHandle`（-1/0/1）、`longPressDragging`、`actionMode`、`startHandleRect/endHandleRect`
- 模拟器 API（JNI 到 Rust Ghostty 侧）：`selectWord(cx, cy)`、`selectLine(cx, cy)`、`selectAll()`、`selectionAnchor(which)`、`selectionDrag(x, y)`、`selectionClear()`、`snapshot()`
- 选区滚动/新输出时由模拟器保持（不丢失）

**对比 torvox**：torvox 的选择状态在 Kotlin ViewModel（`SelectionState`）+ Rust `RenderState.selection`，滚动时靠 `setSelection` JNI 重新下发。ghostty-android 的 `snapshot` 模型（每次 onDraw 前取最新快照）更简洁，torvox 的 `render_inner` 中 scrollback 坐标转换（`internal.rs`）是等效实现但更绕。

### 2.2 多击选择：tapCount 计数器（最佳实现）

`TerminalView.java:1051-1083`：

```java
private void handleTap(MotionEvent e) {
    long now = e.getEventTime();
    boolean continues = now - lastTapTime <= tapTimeoutMs
            && Math.abs(e.getX() - lastTapX) <= tapSlopPx
            && Math.abs(e.getY() - lastTapY) <= tapSlopPx;
    tapCount = continues ? tapCount + 1 : 1;
    ...
    if (tapCount == 1) {
        if (selecting) { finishSelection(); tapCount = 0; return; }  // 单击取消选择
        requestFocus();
        if (tapToOpenLinks && openLinkAt(e.getX(), e.getY())) return;  // OSC 8 链接
        if (touchKeyboardEnabled) imm.showSoftInput(...);  // 弹键盘
    } else if (tapCount == 2) {
        selectWordAt(e.getX(), e.getY());   // 双击选词
    } else {
        selectLineAt(e.getX(), e.getY());   // 三击选行
    }
}
```

关键设计（构造函数 `:494-498`）：
```java
// Disable GestureDetector's own double-tap detection: with it on, the second
// tap's up is routed to onDoubleTapEvent instead of onSingleTapUp, hiding it
// from our tap counter.
gestures.setOnDoubleTapListener(null);
```

要点：
1. **禁用 GestureDetector 双击**，`onSingleTapUp` 每次触发 → 自己计数（`tapTimeoutMs` = `ViewConfiguration.getDoubleTapTimeout()`，`tapSlopPx` = `getScaledDoubleTapSlop()`）
2. 单击行为：选择中→取消选择；非选择中→弹键盘（或开 OSC 8 链接）
3. 计数即时生效（无延迟）—— 单点键盘立即响应

**对比 torvox**：torvox 用 `GestureDetector.onDoubleTap`（系统双击检测，第二次 UP 被 onDoubleTapEvent 吃掉）。ghostty-android 的自计数方法解决了"双击延迟"问题，且支持三击。torvox 的 `DOUBLE_TAP_WINDOW_MS=400L` 自实现与 ghostty-android 思路相同，但 torvox 需要检查是否处理了"选择中单击取消"和"OSC 8 链接优先"。

### 2.3 菜单：Callback2 + TYPE_FLOATING + onGetContentRect

`TerminalView.java:1423-1505`：

```java
private final ActionMode.Callback2 selectionActions = new ActionMode.Callback2() {
    @Override public boolean onCreateActionMode(ActionMode mode, Menu menu) {
        menu.add(Menu.NONE, MENU_COPY, 0, android.R.string.copy);
        menu.add(Menu.NONE, MENU_SELECT_ALL, 1, android.R.string.selectAll);
        if (clipboardHasText()) menu.add(Menu.NONE, MENU_PASTE, 2, android.R.string.paste);
        return true;
    }
    @Override public boolean onActionItemClicked(ActionMode mode, MenuItem item) {
        if (item.getItemId() == MENU_COPY) copySelection();
        else if (item.getItemId() == MENU_PASTE) pasteClipboard();
        else if (item.getItemId() == MENU_SELECT_ALL) { selectAll(); return true; } // 保持工具栏
        mode.finish();
        return true;
    }
    @Override public void onDestroyActionMode(ActionMode mode) {
        // 单一重置路径：finishSelection() 和系统 dismiss 都走这里
        actionMode = null; selecting = false; draggingHandle = -1;
        longPressDragging = false; toolbarSelGeom = Long.MIN_VALUE;
        if (session != null) session.emulator.selectionClear();
        invalidate();
    }
    @Override public void onGetContentRect(ActionMode mode, View view, Rect outRect) {
        // Float the toolbar around the visible part of the selection,
        // leaving room for the handles below it.
        int top = snapshot.selectionStartVisible() ? snapshot.selectionStartY() * cellHeight : 0;
        int bottom = snapshot.selectionEndVisible()
                ? (snapshot.selectionEndY() + 1) * cellHeight
                        + (handleRight != null ? handleRight.getIntrinsicHeight() : 0)
                : getHeight();
        int left = 0, right = getWidth();
        if (单行) { left = textMarginLeft + (int)(startX * cellWidth);
                    right = textMarginLeft + (int)((endX+1) * cellWidth); }
        outRect.set(left, top, right, bottom);
    }
};
```

要点：
1. **Java 匿名类 `new ActionMode.Callback2()`** —— Kotlin 对应 `object : ActionMode.Callback2() { }`（**必须带括号**，Callback2 是抽象类不是接口，这是之前编译错误的正确解法）
2. 菜单项：Copy / Select All / Paste（Paste 仅在剪贴板有文本时显示，用 `clipboardHasText()` 元数据检查避免触发"应用读取剪贴板"toast —— **重要细节**）
3. Select All 不 finish（`return true` 保持工具栏，Copy 一步可达）
4. onDestroyActionMode 是**单一重置路径**（系统 dismiss 也会触发）
5. onGetContentRect：top/bottom 按可见性（selectionStartVisible），单行时缩窄 left/right 到选区列，底部预留手柄高度

**对比 torvox**：torvox 的 `SelectionActionCallback` 是 `Callback`（无 onGetContentRect），菜单锚定整个 view。**应立即迁移到 `Callback2()`**（Kotlin 带括号）+ 上述算法。Paste 可见性检查（clipboardHasText 元数据模式）也值得移植（torvox 当前总是显示 Paste）。

### 2.4 工具栏重定位：selectionGeometryKey + reshowToolbar（关键技巧）

`TerminalView.java:1153-1176` + `1239-1250`：

```java
private long selectionGeometryKey() {
    if (!snapshot.hasSelection()) return Long.MIN_VALUE;
    long flags = (snapshot.selectionStartVisible() ? 1 : 0) | (snapshot.selectionEndVisible() ? 2 : 0);
    return (flags << 48)
            | ((long)(snapshot.selectionStartX() & 0xFFF) << 36)
            | ((long)(snapshot.selectionStartY() & 0xFFF) << 24)
            | ((long)(snapshot.selectionEndX() & 0xFFF) << 12)
            | (snapshot.selectionEndY() & 0xFFF);
}
// onDraw 中：几何变化才 invalidateContentRect
if (toolbarSelGeom != selectionGeometryKey()) {
    toolbarSelGeom = selectionGeometryKey();
    if (actionMode != null) actionMode.invalidateContentRect();
}

private void reshowToolbar() {
    if (actionMode == null) return;
    toolbarSelGeom = selectionGeometryKey();
    actionMode.invalidateContentRect();
    actionMode.hide(0);  // 关键：取消 framework 的 hide-requested flag
}
```

**关键技巧**：`invalidateContentRect()` 单独调用只重定位，但 framework 的 hide-requested flag 仍置位 → 工具栏保持隐藏 ~2s（"工具栏延迟出现" bug 的根源）。`hide(0)` 立即重新显示。

**对比 torvox**：torvox 拖动结束后菜单重新显示有延迟问题（用户截图 `drag_menu_visible.png` 抱怨过）——`reshowToolbar` 的 `hide(0)` 技巧是直接解药。

### 2.5 手柄拖动 + 长按拖动

`TerminalView.java:1178-1236`（selectionHandleTouch）：

```java
case ACTION_DOWN:
    for (int which = 0; which < 2; which++) {
        RectF r = which == 0 ? startHandleRect : endHandleRect;
        if (r.isEmpty() || !r.contains(event.getX(), event.getY())) continue;
        draggingHandle = which;
        // 拖拽相对抓住的端点 cell 中心：选择不跳动
        int hx = which == 0 ? snapshot.selectionStartX() : snapshot.selectionEndX();
        int hy = which == 0 ? snapshot.selectionStartY() : snapshot.selectionEndY();
        dragOffsetX = textMarginLeft + (hx + 0.5f) * cellWidth - event.getX();
        dragOffsetY = (hy + 0.5f) * cellHeight - event.getY();
        if (session != null) session.emulator.selectionAnchor(which);
        return true;
    }
case ACTION_MOVE:
    if (draggingHandle < 0) return false;
    dragSelectionTo(event.getX() + dragOffsetX, event.getY() + dragOffsetY);
    return true;
case ACTION_UP: case ACTION_CANCEL:
    if (draggingHandle < 0) return false;
    draggingHandle = -1;
    reshowToolbar();  // 拖动结束重定位+重显工具栏
    return true;
```

`dragSelectionTo`（:1252-1268）：
```java
if (py < 0) session.emulator.scrollBy(-1);          // 拖出顶部滚一行
else if (py >= rows * cellHeight) session.emulator.scrollBy(1);  // 拖出底部滚一行
session.emulator.selectionDrag(clampToGrid(...), clampToGrid(...));
if (actionMode != null) actionMode.hide(ActionMode.DEFAULT_HIDE_DURATION);  // 拖动时隐藏
```

`longPressDragTouch`（:1208-1237）：长按后不抬手指继续拖动扩展选择（`longPressDragging`），UP 时 `reshowToolbar()`。

**对比 torvox**：torvox 的锚点拖拽（`dragAnchorRow/Col`）与 dragOffset 思路等价；但 torvox 拖动时菜单隐藏/重现时序有 bug（见上 reshowToolbar）。ghostty-android 的"每次 MOVE 边缘滚一行"比 torvox 的定时 edgeScrollHandler 更直接。

### 2.6 手柄绘制：placeHandle 镜像翻转（最佳细节）

`TerminalView.java:1709-1760`：

```java
private Drawable placeHandle(boolean start, float tipX, float tipY, RectF touchRect) {
    // 两个系统 drawable 互为镜像：left 的 tip 在右（bulb 向左，hotspot 3/4 宽），
    // right 的 tip 在左（bulb 向右，hotspot 1/4）。所以任一端点的"向内"drawable
    // 就是另一个。
    Drawable outward = start ? handleLeft : handleRight;
    Drawable inward = start ? handleRight : handleLeft;
    float outwardHotspot = start ? 0.75f : 0.25f;
    float inwardHotspot = start ? 0.25f : 0.75f;
    Drawable d = outward; float hotspot = outwardHotspot;
    int w = d.getIntrinsicWidth();
    float leftEdge = tipX - hotspot * w;
    boolean spills = start ? leftEdge < 0 : leftEdge + w > getWidth();
    if (spills && inward != null) {
        d = inward; hotspot = inwardHotspot; w = d.getIntrinsicWidth();
        leftEdge = tipX - hotspot * w;
    }
    int h = d.getIntrinsicHeight();
    d.setBounds(Math.round(leftEdge), (int) tipY, ...);
    touchRect.set(...);
    touchRect.inset(-w / 4f, -h / 4f);  // 触摸区域放大 1/4
    return d;
}
```

要点：
1. 手柄 tip 精确对准端点 cell（start 在 `startX * cellWidth`，end 在 `(endX+1) * cellWidth`）
2. **边缘溢出时用镜像 drawable**（bulb 向内翻）——保证手柄永远完整可见且 tip 不偏离
3. 触摸区域 inset 放大（`-w/4, -h/4`）——小手柄易点中
4. 手柄位置：`(selectionStartY() + 1) * cellHeight`（行下方）

**对比 torvox**：torvox 手柄用自绘 drawable + anchor 偏移，无镜像翻转逻辑。ghostty-android 用系统 drawable（`textSelectHandleLeft/Right` 通过 `obtainStyledAttributes` 获取 `:385-390`）——**系统样式要求的最直接实现**。

### 2.7 长按选词

`startSelection`（:1085-1092）：
```java
private void startSelection(float px, float py) {
    if (session == null) return;
    if (!selectWordAt(px, py)) return;
    // 长按后继续拖动扩展：固定 start，拖动移动 end
    session.emulator.selectionAnchor(1);
    longPressDragging = true;
}
```

与 termux 不同：ghostty-android 的选词逻辑在 Rust 侧（`selectWord` 由 libghostty 实现），视图只转换像素→网格坐标：
```java
int cx = clampToGrid((px - textMarginLeft) / cellWidth, cols);
int cy = clampToGrid(py / (float) cellHeight, rows);
```

**对比 torvox**：torvox 的 `SelectionExpander`（Kotlin 纯实现，URL/引号处理）比 ghostty 的 Rust `selectWord` 更丰富，但 ghostty 的像素→网格转换（`textMarginLeft` 处理文本左边距）值得检查 torvox 是否遗漏了 margin 偏移。

### 2.8 其他 UX 细节

- `flashBell()`：视觉铃声（BEL 时闪烁终端表面，`BELL_FLASH_MS`）
- `MAX_FLING_ROWS_PER_SEC = 600f`：fling 限速（防硬甩跨越整个 scrollback）
- 滚动：`smoothScroll` 像素滚动 + 逐行滚动双路径；alt screen 时滚动转为方向键（`scrollRemainder += dy / cellHeight`）
- `computeVerticalScrollRange/Offset/Extent`：框架滚动指示器（scrollState 缓存避免每帧 JNI）
- `clipboardHasText()`：仅元数据检查（`getPrimaryClipDescription().hasMimeType("text/*")`）避免剪贴板 toast

## 3. 渲染

- **CPU 渲染**：`canvas.drawText` 按 run 批处理（`runText` StringBuilder 批量，:1832），非逐格 —— 比 termux 逐格优化
- 壁纸：`setBackgroundImage(Bitmap)` 持有 bitmap，滚动时缓存瓦片（`clearImageCache`）
- 网格：`cellWidth = textMarginLeft + x * cellWidth` 公式，`textMarginLeft` 用于文本左边距
- 无 GPU 加速（本项目 wgpu 是架构优势）

## 4. 依赖清单

| 依赖 | 用途 | 本项目适用性 |
|------|------|--------------|
| libghostty（JNI 封装） | 终端核心 | 已用（libghostty-vt）——本项目的 Rust 直接链接方式更优 |
| 无第三方 UI 依赖 | 纯 View 自绘 | torvox 用 Compose，不适用 |

## 5. 项目文档吸收

- README 描述了安装/构建流程（Android 交叉编译 libghostty 的坑），本项目 `docs/` 已覆盖
- **选择系统 API 设计**（emulator owns selection）值得吸收进本项目架构文档

## 6. 代码注释引用（待加入 torvox 代码）

```
TerminalSurface.kt SelectionActionCallback:
// 菜单锚定参考 ghostty-android TerminalView.java:1423-1505 (Callback2 + TYPE_FLOATING)
// Kotlin 必须写 object : ActionMode.Callback2()（带括号，Callback2 是抽象类）
// onGetContentRect: top/bottom 按可见性，单行缩窄 left/right，底部预留手柄高度
TerminalSurface.kt 菜单重显:
// reshowToolbar 参考 ghostty-android TerminalView.java:1239-1250
// invalidateContentRect() + hide(0) 取消 framework hide-requested flag
// （否则工具栏延迟 ~2s 出现——用户截图 drag_menu_visible.png 的 bug）
TerminalSurface.kt 手柄:
// placeHandle 镜像翻转参考 ghostty-android TerminalView.java:1709-1760
// 边缘溢出时用镜像 drawable 保持 tip 精确 + 触摸区域 inset 放大
TerminalSurface.kt 多击选择:
// tapCount 自计数参考 ghostty-android TerminalView.java:1051-1083
// 禁用 GestureDetector 双击 (setOnDoubleTapListener(null))，onSingleTapUp 自计数
TerminalSurface.kt showSelectionMenu:
// Paste 可见性：clipboardHasText() 元数据检查参考 ghostty-android :1281-1291
// （避免触发"应用读取剪贴板"toast）
```

## 7. 结论

ghostty-android 是**选择系统 UX 的最佳参考**（比 termux 更完整）：
1. `Callback2()`（Kotlin 带括号）→ 修复 torvox 菜单锚定
2. `reshowToolbar` 的 `hide(0)` → 修复菜单延迟重现
3. `placeHandle` 镜像翻转 → 手柄边缘可见性
4. `tapCount` 自计数 → 多击选择即时响应
5. `clipboardHasText()` → Paste 按钮智能显示
6. 模拟器拥有选择状态 → 架构一致性
