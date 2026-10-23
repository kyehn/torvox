## 未被解决的问题

基于 `bf6583c0082567e2e5773e33ffb342440ec7cb50`

- 上下滑动滚动 缓慢 卡顿 闪烁
- 应用图标较旧版观感下降，由于应用图标空间有限内容可以溢出以保证观感，软件启动图实际内容占比较小观感不佳
- 可执行文件通过文档提供者修改后丢失权限，修复无效
- 使用 kitten icat 测试 Terminal graphics protocol 可以工作，但是图像部分位置不会自动移动（手势滚动无问题），后面显示的内容会逐渐被图像覆盖。yazi 无法显示图像，实际为数个方框叉号
  - 已实现：Unicode 占位符（`U=1`）解码与绘制、放置缓存纳入回滚长度与占位符布局签名；待真机验证 icat 滚动与 yazi 预览
- 设置终端主题的跟随系统应该根据规范更新为日夜切换
- 修饰栏按键接线不完整，yazi 的对话框选项 使用 左右箭头无法移动选择
- 01-fastfetch.png 根据新规范 安装 fastfetch，运行 `fastfetch --logo none --pipe -s "Title:Separator:OS:Host:Kernel:Uptime:Uptime:Packages:Shell:Terminal:CPU:Memory:Swap:Disk:Locale"` 命令 应该修改
