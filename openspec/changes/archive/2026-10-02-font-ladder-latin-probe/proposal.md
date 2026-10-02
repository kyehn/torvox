# 主字体降级梯次加拉丁覆盖探测

## Why

模拟 ZTE 设备（`fonts.xml` 声明的 `DroidSansMono.ttf` 缺失）实测时，放宽装库后
梯次第 2 级「库内任一等宽面」选中了 `Noto Color Emoji Flags`——emoji 字体被
fontdb 标记为等宽却没有任何拉丁字形，ASCII 全为豆腐块，终端仍不可用。

## What Changes

- 梯次第 2 级收窄为「库内首个覆盖基本拉丁（charmap 探测 `'m'`）的等宽面」，
  跳过无拉丁字形的 emoji/符号「等宽」面；探测字符与单元格度量所用一致。
- 新增第 3 级「库内首个覆盖拉丁的面」与原最后手段「库内首个面」顺移为第 4 级。
- 修正 `set_font_family` 按需装入日志把布尔结果写成「true 个文件」的错误。
