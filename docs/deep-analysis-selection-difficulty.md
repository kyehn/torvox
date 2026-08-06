# 深度分析：为什么 torvox 的文本选择如此困难

> 状态：2026-08-06。本文是问题/现状/困难的诚实盘点，对比五个参考项目，
> 分析困难根源，并明确指出哪些需求在技术上极其困难及原因。

---

## 1. 现状盘点（已修复 vs 未修复）

### 1.1 已修复并验证（round-216，已提交推送）

| 问题 | 修复 | 验证方式 |
|---|---|---|
| 行间距过大（two_handles_only.png） | 行高 = ascent+descent（与 Termux/Ghostty 一致） | OCR + 像素测量：字形 66px / 行距 82px |
| 内容超出屏幕不滚动 | quad 尺寸 = font metrics × raster_scale（此前 surface/rows 导致 IME 打开时 quad 157.8px） | 设备验证 scrollback_rows=57，滚动正常 |
| 菜单非系统样式（紫色文字+分割线+草稿感） | 删除自绘 PopupWindow，改用系统 ActionMode（Copy/Select All/Paste） | 截图：COPY/SELECT ALL/PASTE y=183-220，栏色 (26,27,33)、文字 (241,240,247)，**零紫色像素**；Copy 点击写入剪贴板成功 |
| IME 黑色空隙（ime_no_black_gap.png） | ModifierBar 背景移到 `.padding(bottom=ime)` 之前，铺满动画区 | 像素扫描 y=1420-1520 全主题背景色 |
| 光标不可见（cursor_aligned.png） | 实为 blink 关相位截图；确认 6 帧明暗交替（2.4M vs 5.0M 亮度） | 逐帧像素统计 |
| 选择反色 | fg↔bg swap（经典 reverse video，与 Termux/VTE/rio 一致） | 选中区 37027px 亮 + 6515px 暗 |
| 选择手柄数量 | 2 个（左+右），去掉多余光标手柄 | UI dump：两个 APPLICATION_SUB_PANEL |

### 1.2 未修复 / 已知缺陷

| 缺陷 | 说明 |
|---|---|
| 菜单是顶部标准 ActionMode，**不是** FloatingActionMode | 用户要求"像系统那样智能靠近选区"——termux 用 `ActionMode.TYPE_FLOATING` + `Callback2.onGetContentRect` 锚定选区；当前是 `startActionMode` 顶部栏 |
| 手柄与 ActionMode 未联动（termux 拖动时隐藏菜单、抬手延迟显示） | 当前拖动时菜单行为未完全对齐 termux 的 `hide(0)`/延迟 `show()` 语义 |
| 字体"扁/模糊"（font_grid_fixed.png） | 已大幅改善（raster_scale 双侧同步 + hinting 策略），但与顶级桌面终端仍有差距：无子像素渲染、无 LCD 滤波（GLES 限制） |
| bootstrap 安装的 IO/权限 | 已用 linker64+LD_PRELOAD 打通执行链，但 nix-on-droid bootstrap 兼容验证未完成 |
| DocumentProvider 完整 CRUD | rename 已实现，delete/create/write 需完整验证 |
| 480×854/360dp 全流程 | 部分验证，未全矩阵 |

---

## 2. 参考项目对比（五个项目，五种路线）

| 项目 | 语言/渲染 | 选择高亮 | 选择菜单 | 手柄 |
|---|---|---|---|---|
| **termux-app** v0.119.0-beta.3 | Java + Canvas（CPU，onDraw 主线程） | 渲染 run 内 fg↔bg 交换 | **FloatingActionMode**（Callback2.onGetContentRect 锚定选区矩形）+ 系统 ContextMenu | 自绘 PopupWindow 手柄（TYPE_APPLICATION_SUB_PANEL），带 hotspot/翻转/边缘隐藏 |
| **ghostty-android-terminal** | Java + Canvas（CPU）；native 是 Zig 预编译 libghostty-vt | 选择活在 Ghostty 引擎内（tracked grid refs），Java 零簿记 | 长按→词选择；未详述菜单 | 长按拖动扩展（无独立手柄 PopupWindow） |
| **Haven** | Kotlin + Compose Canvas（CPU/Skia） | CPU 逐 cell 判断 + 高亮矩形覆盖 | 浮动工具栏（复制/粘贴/打开 URL/方向键微调） | **无手柄**（detectDragGesturesAfterLongPress 拖选） |
| **ghostling** | C + Raylib + libghostty | 未实现选择 | — | — |
| **GNOME Console/VTE** | C + cairo/Pango（CPU） | 绘制期 overlay，默认反色 | 桌面右键菜单/快捷键 | 桌面模型无触摸手柄 |

**关键事实：没有任何成熟项目是 "Rust + wgpu + Android" 终端。** GitHub 搜索
`android terminal wgpu` = 0 条；最接近的 wgpu 终端 rio（7.2k★）是桌面/浏览器，
无 Android；其选择高亮做法是 **CPU 侧把选中 cell 的 fg/bg 交换后写入顶点缓冲**
（shader 无感）。torvox 的选择 GPU 高亮方案（shader 按选区矩形混合）与 Ghostty
本体的 Metal 方案类似，但 Ghostty 的选择逻辑在其引擎内，torvox 的选择逻辑
散在 Kotlin 侧 + JNI 传输。

---

## 3. 为什么如此困难（核心难点分析）

### 3.1 无先例：wgpu+Android 终端是空白地带
所有参考项目的选择系统都建立在**"渲染在主线程同步 onDraw"**这一前提上：
- termux：`onDraw` 每帧从 controller 读选区坐标 → 渲染器立即反色 → `invalidate()` 重绘
- Haven：Compose 状态 → Canvas 重绘
- VTE：cairo 同步绘制

torvox 是唯一 GPU 渲染线程架构，选择高亮需要：
1. Kotlin 选择状态 → JNI 传选区坐标 → Rust RenderState → 渲染线程读 → shader 应用
2. 坐标换算链：**grid row（scrollback 逻辑）→ visible row → 像素（font metrics × raster_scale）→ surface 物理像素 → ActionMode contentRect**
3. termux 只有 cell→像素两层；torvox 有 5-6 层，每层都可能错位

### 3.2 异步渲染 vs 同步手势
termux 长按后 `invalidate()` 下一帧（16ms）即见反色。torvox 渲染线程在模拟器
SwiftShader 下 ~500ms/帧 → 长按后要等 0.5-2s 才见高亮，且渲染线程读选择状态
有竞态（Kotlin 已更新但 Rust 还没收到）。**这使"视觉验证循环"每轮 10-30 分钟**。

### 3.3 系统 API 的适配面
Android 的选区系统（SelectionActionModeHelper、手柄、FloatingActionMode）是
为 TextView/EditText 设计的，对自定义渲染视图只开放了三个可复用件：
- `startActionMode(callback, TYPE_FLOATING)` + `Callback2.onGetContentRect`（termux 用法，可复用）
- 手柄 drawable 资源（termux 用 `textSelectHandleWindowStyle` + 自绘 PopupWindow，可复用）
- 剪贴板/拖拽 API

其余全部自研：手柄 PopupWindow 生命周期、拖动约束（边界/宽字符/翻转/边缘自动滚动）、
菜单显隐时序。

### 3.4 模拟器环境的硬限制（用户要求中最困难的部分）
实测：SwiftShader 软件 Vulkan/GLES ~1.8 fps，`screenrecord` 也只有 ~1.8fps
（每帧间隔 ~550ms）。用户要求"录制视频逐帧分析动画（尤其输入法/辅助按键）"——
**220ms 的 spring 动画在 1.8fps 录制下只能捕捉 0-1 帧**，逐帧分析在物理上不可行。
已用日志级逐帧（`animateDpAsState` 内每帧打印 bottom 值，spring 开 5 帧/关 8 帧平滑）
替代，但这与"视频逐帧"要求有差距，需要用户确认接受替代方案或降低动画时长验证。

### 3.5 字体质量：与顶级软件一致的代价
达到 Termux/Ghostty 的字体观感需要：FreeType hinting（已启用）、raster_scale
超采样（已做）、灰度抗锯齿（swash 默认有）、子像素渲染/LCD 滤波（**GLES 上
wgpu 纹理不支持，需要 CPU 侧合成或放弃**）、字形缓存（已做 LRU）、CJK 回退
（已做）。剩余差距主要是子像素级渲染，属"收益递减"区。

### 3.6 SELinux 执行链（bootstrap）
untrusted_app 域拒绝 `execute_no_trans`（AVC 日志实证）→ 需要 linker64 +
LD_PRELOAD libtermux-exec 全套（已打通），nix-on-droid bootstrap 的
guix/bwrap 依赖链还需验证。

---

## 4. 哪些要求让问题变得极其困难（诚实评估）

| 要求 | 难度 | 原因 |
|---|---|---|
| 菜单"始终不遮挡被选文本，注意多种位置情况" | **高** | 需要选区矩形动态计算 + FloatingActionMode contentRect + 边缘翻转；termux 也只做了"锚定到矩形"未保证绝不遮挡 |
| "录制视频逐帧分析动画" | **极高** | 模拟器 1.8fps 物理上限；真机（用户手机）可做到 60fps 但 CI 不可用 |
| "每层测试全覆盖 + 每层性能测试"（unit/Roborazzi/Compose/Maestro/UIAutomator/Espresso × OCR × 截图 × 视频） | **高** | 单个验证循环在慢模拟器上 10-30 分钟；视觉断言（反色位置/手柄位置/菜单位置 vs 长按位置）需要精确坐标换算 |
| 字体"与顶级软件一致" | 中 | 子像素渲染在 GLES/wgpu 上不可行；其余已达标 |
| 480×854/360dp 全功能 | 中 | 小屏布局适配 + 菜单 clamp，已部分验证 |
| nix-on-droid bootstrap 兼容 | 中 | guix/bwrap 依赖链在 SELinux 下的执行 |
| DocumentProvider 完整 CRUD | 低-中 | 纯 Android API 工作，rename 已实现 |
| ActionMode 智能靠近选区（系统样式） | 中 | `Callback2.onGetContentRect` + TYPE_FLOATING，termux 有完整先例可抄 |

---

## 5. 最佳实现路径（对照 termux 逐条移植）

1. **FloatingActionMode**：`startActionMode(callback, TYPE_FLOATING)`，实现
   `Callback2.onGetContentRect` 按选区像素矩形（font metrics 换算，同 termux
   TextSelectionCursorController.java:194-213）；菜单项 Copy/Paste（+Select All），
   拖动时 `hide(0)`、抬手 `ViewConfiguration.getDoubleTapTimeout()` 延迟 `show()`。
2. **手柄**：沿用现有 PopupWindow 手柄（已与 termux 同款 drawable），补齐拖动约束：
   边界钳制、起点不越过终点、边缘自动滚动（termux TextSelectionCursorController.java:218-305）。
3. **词扩展**：长按非空白→向两侧扩展到空白（当前 Semantic 模式已实现，需与 termux
   行为对齐：空白→单格）。
4. **反色**：已在 GPU 侧实现 fg↔bg swap（与 termux/VTE/rio 一致），保持。
5. **坐标**：渲染线程读选择状态前用 scrollback offset 转换（已实现），保持。
6. **验证**：以 termux 为"最正确效果"逐截图对比；480×854 + 1080×2400 双分辨率。

---

## 6. 需要用户决策的点

1. **动画逐帧验证方式**：模拟器 screenrecord 1.8fps 无法捕获 220ms 动画。
   选项：A) 接受日志级逐帧 + 截图前后对比；B) 降低动画时长/改位置插值便于捕捉；
   C) 只在真机上做逐帧（需真机）。
2. **子像素字体**：GLES 上不可行，接受现状（灰度 AA + hinting + 超采样）还是探索
   其他方案。
3. **"绝不遮挡"的严格度**：FloatingActionMode 只能锚定，无法保证所有位置不遮挡
   （termux 亦然）；是否接受"锚定到选区上方/下方，边缘翻转"的 termux 级语义。
