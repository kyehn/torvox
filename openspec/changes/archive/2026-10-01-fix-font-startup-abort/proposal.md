# 修复 fonts.xml 等宽字体缺失导致的启动崩溃

## Why

设备（ZTE P720S20，Android 13）启动即 SIGABRT：`FontPipeline::find_monospace_font`
要求 `fonts.xml` 声明的等宽字体（该机为 `DroidSansMono.ttf`）必须已在渲染字体库中，
找不到即 `abort`。自 b0cd2fca 渲染侧只常驻 主+符号+区域 三族后，声明的等宽面有
两条缺失路径：

- `~/.termux/fonts` 投放目录有字体时，建库把用户投放字体当作主字体装入而跳过
  `fonts.xml` 等宽字体，且 `OnceLock` 缓存初始化与 `setExtraFontPaths` 存在竞态；
- OEM 精简 ROM 声明了等宽字体却不提供文件：logcat 含 native WARN 级日志但无
  `failed to load font file` 警告，即从未尝试加载，文件在所有平台字体目录中不存在。

两者都在启动时必然 abort，应用完全不可用（连设置页都进不去）。

## What Changes

- 建库主字体恒为 `fonts.xml` 的 monospace 族；用户 `font.{ttf,ttc,otf}` 覆盖经
  `setFontFamily` 按需装入生效，`~/.termux/fonts` 投放字体只进字体列表、选中时
  经 `load_family` 装入——两者不再抢占主字体位置，建库结果与 `setExtraFontPaths`
  时序无关。
- 主字体选择改为降级梯次：声明词干精确匹配 → 库内任一等宽面 → 库内首个可用面；
  每级降级输出日志；仅当库内没有任何可用面（等同 `fonts.xml` 不可用）才 abort。
- 最小常驻集构建完成后若不含任何等宽面，放宽装入 `fonts.xml` 全量声明文件一次
  （进程级缓存承载，健康设备不进入该分支）。
- `fonts.xml` 缺失或不可解析仍 abort（DESIGN 不变）。

## Non-goals

- 不改 Kotlin 侧字体设置流程（`font.{ttf,ttc,otf}` 探测与 `setFontFamily` 链不变）。
- 不改字体列表的来源与顺序（`family_index` 行为不变）。
