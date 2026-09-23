# 深度研究：termlib 6 特性对比与采纳计划

> 研究日期：2026-08-07
> 参考：connectbot/termlib（/tmp/refs5/termlib）
> 本地：torvox native/ + android/

---

## 1. OSC 133 语义段（TerminalEmulator.kt:830-930 + OscParser.kt:275-340）

### termlib 实现
- `SemanticType`：DEFAULT/PROMPT/COMMAND_INPUT/COMMAND_OUTPUT/COMMAND_FINISHED/ANNOTATION/HYPERLINK
- `SemanticSegment(startCol, endCol, type, metadata, promptId)` 挂在每行上，滚动时随行迁移（pendingSegments + movedSegmentRows）
- OscParser `handleOsc133`：A=prompt 开始、B=prompt 结束+COMMAND_INPUT 标记（零宽，C 时更新）、C=COMMAND_OUTPUT、D=COMMAND_FINISHED（带 exit code）
- `getLastCommandOutput()`：用语义段找最近完成命令的输出文本
- 关键细节：COMMAND_INPUT 在 B 时刻就放置（Enter 后滚动不影响标记）

### 本地现状
- `output_processor.rs`：`extract_osc133` 检测 A/B/C/D → `snapshot.shell_integration` 枚举（PromptStart/PromptEnd/CommandStart/CommandFinished）——**只有单值标记，无列范围/无 promptId/无输出提取**
- 用途：仅测试断言

### 采纳决策：**实施（高价值）**
- Rust 侧：输出处理器维护 `Vec<SemanticSegment>`（行→段列表），A/B/C/D 事件按当前 cursor 列记录；D 时记录 exit code
- JNI `getLastCommandOutput()`：返回最近完成命令的输出文本（termux shell integration 标准功能）
- 测试：OSC 133 序列 → 段生成 → getLastCommandOutput 内容断言

---

## 2. Compose 键序列模式（ComposeMode.kt:30）

### termlib 实现
- Compose 模式激活时文本本地缓冲 + 光标处 overlay 显示；Enter 提交、Esc 取消
- 防误触的安全输入（vi 风格的组合键输入）

### 本地现状
- 无 Compose 模式（终端模拟器通常长按 Esc 模拟）

### 采纳决策：**暂缓**（终端键盘布局不同——Android IME 无独立 Esc 长按语义；termux 也无此功能；低优先级）

---

## 3. 自绘放大镜（Terminal.kt:2044 MagnifyingGlass）

### termlib 实现
- Compose Box 绘制圆形放大镜：手指上方偏移显示放大内容，用 textPaint 直接画字符
- `magnifierOffset` 计算：保持屏幕内 + 手指上方偏移 + 边界翻转

### 本地现状
- `TerminalSurface.kt` 用系统 `android.widget.Magnifier`（`magnifier?.show()`）——已验证工作（round 早期）

### 采纳决策：**保留系统 Magnifier**（已实现等效功能；系统实现有硬件加速 + 自动边界处理；自绘反而增加维护）

---

## 4. 无障碍朗读（AccessibilityOverlay）

### termlib 实现
- LazyColumn 镜像 scrollback + 每行 semantics；hidden live region + announcementCounter 触发 TalkBack 朗读
- 屏幕阅读器模式：逐行 review、朗读当前行

### 本地现状
- `TerminalSurface.kt:1523` `importantForAccessibility = IMPORTANT_FOR_ACCESSIBILITY_YES`
- **无 live region / 无朗读支持**（终端内容是自绘 SurfaceView，TalkBack 读不到）

### 采纳决策：**实施（高价值，无障碍合规）**
- TerminalSurface 增加 `AccessibilityOverlay` 等效：每帧更新当前可见文本为 contentDescription + `announceForAccessibility` 触发关键事件（bell、标题变化）
- 注意：不能每帧 announce（会刷屏）——只在事件（bell/输出结束）时 announce
- 测试：Robolectric 断言 contentDescription 随输出更新

---

## 5. URL 滚动位置惰性缓存

### termlib 实现
- `getUrls()` 每次显式扫描（osc8 + 正则），`autoDetectUrls` 开关控制持续扫描
- 扫描含跨行 wrap 的 URL；alt screen 时不扫 primary scrollback
- 惰性：URL 列表按需计算（getUrls 调用时）

### 本地现状
- `UrlDetector.kt` + `UrlDetectorTest`（6 测试）——纯文本 URL 检测已有
- OSC 8 hyperlink：本地有 `hyperlinkAt(col,row)` 吗？查

### 采纳决策：**补齐 gap**
- 本地 URL 检测已有；补 OSC 8 超链接点击/提取（termux 标准功能）+ 跨行 URL

---

## 6. CellRun 游程编码

### termlib 实现
- `CellRun` 可复用对象：连续相同格式 cell 一批传输 JNI（fg/bg/attrs/chars）
- `getCellRun(row, col, cellRun)` → run 长度 + 格式 + 字符数组
- 减少 JNI 往返（一次取一批同格式 cell）

### 本地现状
- `build_cell_data` 每 cell 独立 FFI 调用（row-level dirty cache 已减少重建，但每个 cell 仍单独取）
- **无游程合并**

### 采纳决策：**实施（性能优化）**
- `build_cell_data` 内：行内检测连续相同格式 cell → 合并游程（减少 CellData 条目数：同格式连续 cell 可共享样式，仅字符不同）
- 影响：CellData 结构需支持"同格式游程"（当前每 cell 独立）——中等改动
- 更轻方案：行内检测 fg/bg/flags 相同 → 生成游程标记，渲染侧逐字符展开
- 测试：格式相同连续文本 → 游程数减少断言

---

## 采纳优先级

| # | 特性 | 决策 | 理由 |
|---|------|------|------|
| 1 | OSC 133 语义段 + getLastCommandOutput | **实施** | shell integration 标准功能，用户价值高 |
| 2 | Compose 键序列 | 暂缓 | Android IME 布局不适用 |
| 3 | 自绘放大镜 | 保留系统 | 已等效实现 |
| 4 | 无障碍朗读 | **实施** | 合规 + TalkBack 支持 |
| 5 | URL 滚动惰性缓存 | **补齐 OSC8** | 已有正则检测 |
| 6 | CellRun 游程编码 | **实施** | 性能优化 |
