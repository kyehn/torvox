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

#### Scenario: 列表顺序等于文档顺序

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

#### Scenario: 列表顺序等于文档顺序

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
