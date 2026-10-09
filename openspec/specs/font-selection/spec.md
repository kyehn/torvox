# font-selection Specification

## Purpose

终端字体选择的来源与生效链路。字体列表须是 `/system/etc/fonts.xml` 声明的族，
不是硬编码表也不是目录扫描结果；渲染路径只常驻主字体、符号族与区域族三族，完整
族索引仅在设置页按需构建。`files/home/.termux/font.{ttf,ttc,otf}` 存在即默认生效
（不复制不移动），`files/home/.termux/fonts` 目录内字体并入列表。设置项写错时
清除该设置而不是弹一次 toast 掩盖。

## Requirements

### Requirement: 渲染侧字体库只常驻三族

渲染路径的字体库 MUST 只常驻主字体、一个符号族（`fonts.xml` 中既无 `name` 也无
`lang` 的族）与一个区域族（当前 locale 对应的 `lang` 块）。主字体恒为 `fonts.xml`
的 monospace 族：用户 `files/home/.termux/font.{ttf,ttc,otf}` 覆盖经 `setFontFamily`
按需装入生效，`files/home/.termux/fonts` 投放字体只进字体列表、选中时经
`load_family` 装入；两者 MUST NOT 在建库时抢占主字体位置。其余 200 余族是
WebView/UI 用字，渲染永不触及，MUST NOT 常驻加载。

#### Scenario: 库只含三族

- **WHEN** 字体库构建完成
- **THEN** 面数等于 fonts.xml 等宽主字体 + 符号族 + 区域族，不含用户投放字体与
  UI 用字族

#### Scenario: 投放字体不抢占主字体

- **WHEN** `files/home/.termux/fonts` 目录存在字体文件且用户未选择字体
- **THEN** 主字体仍是 fonts.xml 的 monospace 族，应用正常启动不 abort

### Requirement: 字体列表仅设置页按需构建

完整字体族索引 MUST 只在显示字体列表时构建并缓存至进程结束：族名存在字体的
name 表里，只有读完 `fonts.xml` 声明的全部文件才能得到。渲染路径 MUST NOT 触发该
枚举。索引 MUST 按 `fonts.xml` 文档顺序返回并精确去重，不排序、不归并；
`files/home/.termux/fonts` 投放的字体追加在后。

#### Scenario: 渲染不触发族枚举

- **WHEN** 只启动终端不做字体设置
- **THEN** 族索引日志 `FONT_INDEX` 不出现

#### Scenario: 按需构建不改变列表顺序

- **WHEN** 打开字体列表
- **THEN** 前若干项与 `fonts.xml` 的 family 声明顺序一致

### Requirement: 用户字体路径只追加不覆盖

`loadFontFile` 把自定义字体登记到渲染器时 MUST 追加到已注册路径，MUST NOT 覆盖
`files/home/.termux/fonts` 目录项——覆盖会使目录内其余字体永久不可选。

#### Scenario: 探测 font.ttf 后目录字体仍在

- **WHEN** 先注册 `files/home/.termux/fonts` 目录再探测 `font.ttf`
- **THEN** 额外路径仍含该目录，目录内字体仍可被选中

### Requirement: 字体列表取自渲染侧字体库

字体列表 MUST 取自渲染侧字体库（`list_monospace_fonts` → `font_db::family_index`），
该库内容即 `/system/etc/fonts.xml` 声明的文件集加用户投放目录，故列表与渲染侧
可选择的字体必然一致。列表 MUST 按 `fonts.xml` 文档顺序返回、按族名精确去重，
MUST NOT 做名称归一（不把 `DroidSans` 与 `Droid Sans` 视为同一）、不排序、不硬编码
字体名，也 MUST NOT 出现「系统默认」「从文件加载」等含糊行。平台
`SystemFonts.getAvailableFonts()` 不可用于此：它返回 `Set<Font>` 而 SDK 无公开族名
访问器，且未装入渲染库的文件本就无法被选择。字体库为空时 MUST 输出日志并抛
`IllegalStateException` 崩溃退出；`fonts.xml` 缺失或不可解析由 native 侧直接 abort。

#### Scenario: 列表与可生效字体一致

- **WHEN** 打开字体选择对话框
- **THEN** 列表中任一 family 都能被 `setFontFamily` 成功应用

#### Scenario: 列表顺序等于 fonts.xml 文档顺序

- **WHEN** 打开字体列表
- **THEN** 顺序与 `fonts.xml` 的 family 声明顺序一致，无排序痕迹

#### Scenario: 字体库为空即崩溃

- **WHEN** 字体库为空
- **THEN** 输出日志并抛 `IllegalStateException`，不返回空列表

### Requirement: font.ttf 存在即默认

`files/home/.termux/font.ttf`（或同名 `.ttc` `.otf`）存在时 MUST 将其探测到的 family
作为生效字体，不复制不移动该文件。探测结果 MUST 按路径 + mtime + 大小缓存，同一文件
只进库一次。

#### Scenario: 复制 font.ttf 后生效

- **WHEN** `font.ttf` 存在
- **THEN** 生效字体为其 family，界面显示一致（`TESTING.md` 设备项）

### Requirement: 用户字体目录

`files/home/.termux/fonts` 目录下的字体文件 MUST 出现在列表中（经 `setExtraFontPaths`
登记 + `list_monospace_fonts` 枚举，无手动判断）；目录内含 `.ttc` 与 `.otf`。

#### Scenario: 目录字体可选

- **WHEN** 该目录存在字体文件
- **THEN** 其 family 可在对话框中被选中并生效

### Requirement: 设置错误重置而非提示

用户设置的 family 经 native `setFontFamily` 返回 false 时 MUST 输出日志并清除该
设置（`SettingsRepository.clearFontFamily`），使会话回落默认；MUST NOT 只弹 toast
掩盖失败。无会话可验证（返回 null）时不做处理。

#### Scenario: 无效 family 被清除

- **WHEN** `setFontFamily` native 返回 false
- **THEN** 该设置被清除并回落系统默认

#### Scenario: 无会话时不动设置

- **WHEN** `setFontFamily` 因无会话返回 null
- **THEN** 设置保持原值，不清除也不报错

### Requirement: 主字体选择降级梯次

主字体选择 MUST 按序降级：声明的等宽文件按词干精确匹配（分隔符归一，再去空白）→
库内首个覆盖基本拉丁的等宽面（charmap 探测 `'m'`，跳过 emoji/符号这类被标记为
等宽却无拉丁字形的面）→ 库内首个覆盖拉丁的面 → 库内首个可用面；每级降级 MUST
输出日志。仅当库内没有任何可用面时，MUST 经 `font_db::fatal` 输出日志并 abort。
`fonts.xml` 缺失、不可解析或未声明等宽族 MUST 保持 abort（DESIGN 不变），
并同样经该唯一出口退出。

#### Scenario: 声明文件被 OEM 改包装

- **WHEN** `fonts.xml` 声明的等宽文件已加载但家族名与词干不匹配
- **THEN** 选择库内首个覆盖拉丁的等宽面并输出警告日志，不 abort

#### Scenario: 无拉丁字形的等宽面被跳过

- **WHEN** 库内等宽面不含拉丁字形（如 emoji 字体）
- **THEN** 该面被跳过，选择首个覆盖拉丁的面

#### Scenario: 全库无等宽面

- **WHEN** 放宽装库后库内仍无可用等宽面但至少有一个覆盖拉丁的面
- **THEN** 选择首个覆盖拉丁的面并输出错误日志，终端降级可用

#### Scenario: 库为空

- **WHEN** `fonts.xml` 全量声明文件无一可加载
- **THEN** 经 `font_db::fatal` 输出 `FONT_FATAL` 行并 abort

#### Scenario: fonts.xml 未声明等宽族

- **WHEN** `fonts.xml` 可解析但只含 `lang` 族与无名族
- **THEN** 无主字体候选，经 `font_db::fatal` 输出 `FONT_FATAL` 行并 abort

### Requirement: 声明等宽字体不可用时放宽装库

最小常驻集构建完成后若不含任何等宽面（OEM 声明的等宽文件缺失或损坏），MUST 一次性
放宽装入 `fonts.xml` 全量声明文件（跳过已在库中的文件），并由进程级缓存承载；
健康设备 MUST NOT 进入该分支。

#### Scenario: 声明文件缺失时启动

- **WHEN** `fonts.xml` 声明的等宽文件在所有平台字体目录中都不存在
- **THEN** 装库放宽到全量声明集并输出错误日志，应用正常启动

#### Scenario: 健康设备不放宽

- **WHEN** 声明的等宽文件正常加载
- **THEN** 常驻集保持三族，不出现放宽日志

### Requirement: 原生日志在首个 JNI 调用之前就位

原生日志 MUST 由 `JNI_OnLoad` 安装，使 logger 在任何 JNI 方法之前就位。
Kotlin 侧 MUST NOT 以独立线程异步安装日志：启动数秒内的致命退出会抢在安装之前，
经 `log` 门面的原因全部被静默丢弃，现场只剩无符号 tombstone（真实机启动崩溃即此情形）。
MUST NOT 保留已无用途的日志初始化导出与调用方。

#### Scenario: 启动即致命退出仍留下原因

- **WHEN** 进程启动后立即触发原生致命退出
- **THEN** logcat 中出现该退出写出的错误行，而不只是 tombstone

### Requirement: 字体致命退出单一出口且原因指向真实前提

字体侧所有致命退出 MUST 经 `font_db::fatal(reason) -> !` 唯一出口：以 `FONT_FATAL`
为 target 输出 ERROR 后 `abort`，使同类崩溃一次 grep 即可归因。`reason` MUST 陈述真实
前提，MUST NOT 把「字体文件装入失败」写成 `fonts.xml` 问题。
`font_db::fatal` MUST NOT 依赖 `android` 模块（层方向由 semgrep `no-android-in-render`
守卫），日志安装的时序保证由 `JNI_OnLoad` 承担而非直接写 logcat。

#### Scenario: 库空的原因指向字体文件

- **WHEN** `fonts.xml` 可解析但其声明的字体文件一个都装不进 `fontdb`
- **THEN** 致命原因指向字体文件装入失败，`FONT_FATAL` 行不归因 `fonts.xml`

#### Scenario: 等宽字体只解析一次

- **WHEN** 同一进程内创建管线并随后多次以空族名设置字体
- **THEN** `fonts.xml` 只被读盘解析一次，其结果由进程级缓存承载

### Requirement: 字号上下限按完整 sp→px 系数换算

用户可选字号上界 MUST **只有一处定义**：Termux 像素上限（256px）按完整
sp→像素系数（显示密度 × 系统字体缩放，即推给原生 `setRasterScale` 的同一值）
换算后按 Termux 的调整步长向下取整。Kotlin 侧 MUST NOT 另存一份与原生字号守卫
重复的上界常量——两份魔数漂移时用户拖到的字号会被原生静默丢弃，
表现为「设置条范围与实际可设置范围不一致」。

像素上限换算 MUST 使用完整系数，MUST NOT 只按显示密度：字形的实际光栅尺度即为
`sp × 该系数`，只用密度会在系统「字体大小」> 1 时放行实际超出 Termux 像素上限的
字号。调节条、缩放手势、预览、落盘值应用与设置页展示的像素值 MUST 共用该单一来源。
该系数的钳位区间 MUST 与原生 `setRasterScale` 的接受区间一致（0.5..=8.0），否则
合法系数会被原生拒收，反而使上界与实际渲染脱节。

原生字号守卫的上界 MUST 由图集边长推导（字形位图必须放进图集是唯一真实约束），
MUST NOT 是另一份魔数；越界值 MUST 记错误日志，MUST NOT 静默丢弃。

#### Scenario: 系统字体放大时上界同步收紧

- **WHEN** 设备显示密度为 2.625 且系统字体缩放由 1.0 改为 1.3（系数 3.412）
- **THEN** 可选字号上界由 96sp 收紧为 74sp，实际像素不超过 Termux 的 256px

#### Scenario: 低密度设备不被截到重复常量的上界

- **WHEN** sp→px 系数为 1.0（Termux 的 256px 对应 256sp）
- **THEN** 可选字号上界为 256sp，调节条末端与手势末端均能给出该值

#### Scenario: 原生上界不小于可选上界

- **WHEN** 任意合法 sp→px 系数（含 0.75..=4.0）下比较「图集边长 ÷ 光栅缩放」与
  按 Termux 换算出的可选上界
- **THEN** 前者不小于后者，即滑块能划到的每个字号都被原生接受

#### Scenario: 原生拒收的字号留痕

- **WHEN** 收到的字号超出「图集边长 ÷ 光栅缩放」
- **THEN** 原生记错误日志说明合法区间，而不是无声忽略

#### Scenario: 展示的像素值与真实光栅尺度一致

- **WHEN** 设置页显示「字号：94 SP（约 X px）」
- **THEN** X 等于 94 乘以完整 sp→px 系数，而非仅乘显示密度
