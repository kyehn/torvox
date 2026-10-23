## 未被解决的问题

基于 `bf6583c0082567e2e5773e33ffb342440ec7cb50`

- 上下滑动滚动 缓慢 卡顿 闪烁
- 上下滑动滚动 缓慢 卡顿 闪烁
  - 已改为滚动更新由 vsync 合并（不再逐触摸事件立即唤醒），避免交换链排队等待两个 vsync；待真机复测 `SLOW_RENDER_PHASE acquire` 占比
- 应用图标较旧版观感下降，由于应用图标空间有限内容可以溢出以保证观感，软件启动图实际内容占比较小观感不佳
- 可执行文件通过文档提供者修改后丢失权限，修复无效
  - 已覆盖：`copyDocument`/`moveDocument` 复制权限位，`openDocument("w"/"wt")` 原地截断保留执行位（用例 `copyDocument_preserves_executable_bit`、`openDocument_mode_w_preserves_executable_bit`）。若仍复现，需给出具体客户端与操作：客户端「写临时文件再重命名」会新建 inode，提供器无法继承旧权限
- 使用 kitten icat 测试 Terminal graphics protocol 可以工作，但是图像部分位置不会自动移动（手势滚动无问题），后面显示的内容会逐渐被图像覆盖。yazi 无法显示图像，实际为数个方框叉号
- 设置终端主题的跟随系统应该根据规范更新为日夜切换
- 修饰栏按键接线不完整，yazi 的对话框选项 使用 左右箭头无法移动选择
- 01-fastfetch.png 根据新规范 安装 fastfetch，运行 `fastfetch --logo none --pipe -s "Title:Separator:OS:Host:Kernel:Uptime:Uptime:Packages:Shell:Terminal:CPU:Memory:Swap:Disk:Locale"` 命令 应该修改
- 设置终端主题的跟随系统应该根据规范更新为日夜切换
  - 已实现：`ThemeMode.FOLLOW_SYSTEM` 按 `isSystemInDarkTheme()` 在日间/夜间主题间切换，设置页提供 FIXED/DAY/NIGHT/FOLLOW_SYSTEM 四档
- 修饰栏按键接线不完整，yazi 的对话框选项 使用 左右箭头无法移动选择
  - 已核实：左右箭头经同一编码器发送 CSI/SS3（`horizontal_arrows_encode_in_both_cursor_modes`）；yazi 的 `confirm`/`pick` 默认只绑定 Up/Down（`k`/`j`），左右键不移动属于其默认键位，不是终端丢键。需要左右切换时应在 yazi 键位里自行绑定
- 01-fastfetch.png 根据新规范 安装 fastfetch，运行 `fastfetch --logo none --pipe -s "Title:Separator:OS:Host:Kernel:Uptime:Uptime:Packages:Shell:Terminal:CPU:Memory:Swap:Disk:Locale"` 命令 应该修改
  - 待真机按 `docs/specification/TESTING.md` 的命令重新截图；仓库不保存截图，无法离线生成
