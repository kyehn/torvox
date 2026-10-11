# 03 sohan-f/termux-monet

上游：<https://github.com/sohan-f/termux-monet>
本地：/tmp/competitor-study/sohan-f__termux-monet
自述同步上游 2026-09-26，版本 0.119.0-b1+monet36。termux 非官方换皮 fork，终端与 PTY 沿用上游，仅叠加 Monet 取色、背景图与换皮。全部源码已通读。

## 血缘与改动

新增取色引擎 `MonetTerminalColors.java:76-468` 与单测 `MonetTerminalColorsTest.java:11-205`。调色板注入点为 `TerminalColorScheme.java:60-68` 新增 `updateWithMonetColors`，调用方 `TermuxTerminalSessionActivityClient.java:489-510`，优先级为用户 `colors.properties` 高于 Monet 高于 xterm。新增背景管理器 `TermuxBackgroundManager.java:39-370`，管选图、裁剪、纯色与沉浸栏。风格设置链经 `termux_style_preferences.xml:8-24` 与 `TermuxStylePreferencesFragment.java:70-114`。应用改名 `Termux:Monet`，引入 material 库取 `Hct/Blend/MaterialColors`。

## 取色机制

仅 API31 以上生效。按界面模式判日夜，以壁纸标识做进程缓存。读框架 accent 与 neutral 色板，以 `colorPrimary` 为种子、`colorError` 为红色锚。前景背景取 neutral 系，六彩色以 xterm 色相为锚用高饱和固定明度合成，再向种子色 harmonize 偏转。单色壁纸跳过。有对比度、色相分离、明度分离三门限，按角色顺序逐色避撞。16-255 号色保留 xterm。背景开关仅定义偏好，实际检查字体颜色时无条件走 Monet。

## 与本项目对照

本项目终端主题构建时读 alacritty-theme、固定 10 名、dracula_plus 默认、日夜各一跟随切换；软件主题仅日夜跟随。Monet 为运行时随壁纸动态合成、无固定命名、改 0-15 号色，直接冲突。背景图透明模糊全在禁止列。Java View 改全局调色板单例的路径在本项目无对应物，Ghostty 为状态单源、渲染在 Rust。

## 可参考点

- P0：对比度与可辨识度门限思想及纯函数可测拆分，固定主题校验与预览可复用其测试套路。
- P1：日夜感知、壁纸变更缓存、状态栏导航栏随终端底色联动并按亮度切图标的联动逻辑，但颜色来源须为固定主题。
- P2：色相保持只动明度的思路；用户文件优先于自动主题的优先级链。

## 禁止项勿抄

Monet 动态取色整链、背景图与透明栏与模糊开关、自定义主题覆盖。引入即违反固定主题与禁止背景图透明的规定。

证据：`MonetTerminalColors.java:76-468`、`MonetTerminalColorsTest.java:11-205`、`TerminalColorScheme.java:60-68`、`TermuxTerminalSessionActivityClient.java:489-510`、`TermuxBackgroundManager.java:39-370`、`termux_style_preferences.xml:8-24`、`TermuxStylePreferencesFragment.java:70-114`、`TermuxPreferenceConstants.java:202-232`。
