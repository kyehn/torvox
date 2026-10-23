# STATUS 七项处置证据

对应 `docs/STATUS.md` 的「未被解决的问题」七项。该文件受保护，本留档是证据的
唯一去处：只写结论、用例名与计量，不写原始日志行、时间戳、PID、设备标识。

证据采集环境：x86_64 模拟器（软件 Vulkan），应用以 debug 签名安装，仪器化用例
经 `:app:connectedDebugAndroidTest` 运行。

## 一览

| 项 | 当前状态 | 自动化判据 | 剩余条件 |
| --- | --- | --- | --- |
| 滚动缓慢卡顿闪烁 | 机制已改（滚动更新由 vsync 合并），模拟器量化达标 | `ScrollBehaviorQuantifiedTest` 三例通过 | 真机（Mali）复测 `SLOW_RENDER_PHASE` 的 acquire 占比 |
| 应用图标观感下降 | 几何判据已改（不越出平台裁切圆） | `SplashIconRobolectricTest`、`LauncherAdaptiveIconRobolectricTest` 通过 | 观感属主观判断，需真机人眼确认 |
| 文档提供者改文件丢执行位 | 应用侧可修路径已覆盖 | `DocumentsProviderTest` 两例 + `DocumentsProviderInstrumentedTest.copy_document_preserves_executable_bit_across_process` 通过 | 需具体客户端与操作（写临时文件再重命名会新建 inode） |
| kitten icat 图像不移动 / yazi 不显示图像 | 已实现并有像素级判据 | `KittyGraphicsPlacementTest` 两例、`KittyGraphicsOverlayTest` 通过 | 真实客户端（yazi、kitten icat）在真机上的表现 |
| 主题跟随系统应按日夜切换 | 已实现并有像素级判据 | `ThemeFollowSystemInstrumentedTest` 通过 | 无 |
| 修饰键栏方向键 | 终端侧已核实可用 | `horizontal_arrows_encode_in_both_cursor_modes`、`VtCorrectnessInstrumentedTest` 通过 | 无（yazi 默认键位只绑 Up/Down，需在 yazi 键位里自行绑定左右） |
| fastfetch 截图 | 仓库不保存截图，离线无法生成 | 无（需真机） | 真机 Termux prefix 内安装 fastfetch 后按 `docs/specification/TESTING.md` 的命令截图 |

## 逐项证据

### 一、滚动缓慢卡顿闪烁

- 量化用例：`ScrollBehaviorQuantifiedTest`（进入即吸附底部、洪峰不打断手势、
  拖拽循环帧节奏），三例通过。
- 实测计量：`drag_loop_p95_ms=17`、`drag_loop_fps=72`、`drag_loop_pre_max_ms=0`；
  渲染循环窗口在拖拽期间稳定在 ≈60fps（`avg=16.66ms`）。
- 观察到的慢帧集中在 PTY 洪峰阶段（软件渲染下的全量帧，500ms 级），非拖拽路径；
  该阶段的 `SLOW_RENDER_PHASE acquire` 为一次 28.9ms 的取帧等待，其余阶段 acquire
  接近 0。
- 剩余：模拟器是软件 Vulkan，真机 Mali 的取帧等待分布不同，须在真机复测。

### 二、应用图标观感下降

- 判据已从「墨迹半径不越 66dp 安全区」改为「不越出平台圆形蒙版裁切圆」，
  彩色/单色前景由 0.65 放大到 0.75。
- `SplashIconRobolectricTest` 与 `LauncherAdaptiveIconRobolectricTest` 通过。
- 剩余：观感是主观项，自动化只能给几何边界，人眼确认需真机。

### 三、文档提供者改文件丢执行位

- `copyDocument`/`moveDocument` 复制权限位；`openDocument("w"/"wt")` 原地截断保留
  执行位。
- 单元：`DocumentsProviderTest.copyDocument_preserves_executable_bit`、
  `openDocument_mode_w_preserves_executable_bit`；仪器化（跨进程）：
  `DocumentsProviderInstrumentedTest.copy_document_preserves_executable_bit_across_process`。
- 剩余：客户端「写临时文件再重命名」会新建 inode，提供器无法继承旧权限——这不是
  提供器能修的，需要具体客户端与操作才能定位。

### 四、kitten icat 图像不移动 / yazi 不显示图像

- 根因一：渲染线程的放置缓存键漏了「回滚长度」，新输出把行推入回滚时放置不重查，
  图像钉在旧视口行，被后续文本覆盖。
- 根因二：Unicode 占位符（`U=1`）路径没有把占位单元画成图像，yazi 只看到方框叉号。
- 像素级判据（`KittyGraphicsPlacementTest`）：
  - `kitty_image_moves_up_with_scrolled_content`：视口上滚 3 行后图像包围盒上移
    51px（3 行 × 17px 单元格高，容差半格），像素数不变（4096）——图像跟着内容走。
  - `unicode_placeholder_cell_is_drawn_as_image`：占位单元处出现 64×64 的图像
    （4096 像素），位置落在该单元格内——占位符真的被画成图像。
- 叠加层判据（`KittyGraphicsOverlayTest`）：图像在屏 4096 像素，帧率 60–97fps、
  p95 16ms——图像在场时渲染循环不塌陷。
- 剩余：真实客户端（yazi、kitten icat）在真机上的行为。自动化注入的是同一套
  kitty 协议序列（`a=T` 传输与 `U=1` 虚拟放置 + 占位单元），但客户端自身的
  绘制时序仍需真机确认。

### 五、主题跟随系统应按日夜切换

- `ThemeFollowSystemInstrumentedTest`：终端主题模式与软件主题模式都设为跟随系统，
  系统夜览模式由 shell 切换（`cmd uimode night`），判据是 Surface 的呈现像素。
- 实测计量：日间底色 189252 像素 → 夜间底色 189250 像素，夜间画面里残留的日间
  底色仅 10 像素 → 切回日间 189252 像素。
- 剩余：无。

### 六、修饰键栏方向键

- 终端侧已核实：左右箭头经同一编码器发送 CSI/SS3 两种形式
  （`horizontal_arrows_encode_in_both_cursor_modes`），DECCKM 复位后回到 CSI 形式
  （`VtCorrectnessInstrumentedTest`）。
- yazi 的 `confirm`/`pick` 默认只绑定 Up/Down（`k`/`j`），左右键不移动属于其默认
  键位，不是终端丢键；需要左右切换时应在 yazi 键位里自行绑定。
- 剩余：无。

### 七、fastfetch 截图

- `docs/specification/TESTING.md` 要求安装 fastfetch 后运行
  `fastfetch --logo none --pipe -s "…"` 并截图。
- 仓库不保存截图，离线生成无意义；该命令依赖真机 Termux prefix 内安装的
  fastfetch（bootstrap 包本身不含该包），故只能在真机执行并人工核对。
- 剩余：真机执行该命令并截图核对。

## 附：应用侧 W/E 卫生的最后一块

本轮在模拟器上复测会话收尾路径时发现：等待线程靠退出码反推 execve 失败
（`100 + errno` 编码），而 shell 被 SIGHUP 杀掉时退出码是 129，与「errno=29」的
编码撞车——正常收尾被打成 E 级日志。已改为子进程经 CLOEXEC 管道回报确切 errno，
等待线程只按管道判定。复测：会话收尾的 logcat 里不再有 `E/native::terminal::session`，
只剩 debug 级的「session exit: signalling process group」与 info 级的「child exited」。
对应规范增量见 `logcat-hygiene` 的「execve 失败的判定依据」。
