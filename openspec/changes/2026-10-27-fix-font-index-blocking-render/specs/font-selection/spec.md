# font-selection Specification

## MODIFIED Requirements

### Requirement: 字体列表仅设置页按需构建

完整字体族索引 MUST 只在显示字体列表时构建并缓存至进程结束：族名存在字体的
name 表里，只有读完 `fonts.xml` 声明的全部文件才能得到。渲染路径 MUST NOT 触发该
枚举——用户选定字体文件（`loadFontFile`）时路径已知，MUST 直接把文件装进渲染字体
库并按族名取用，MUST NOT 为定位这一个族而枚举全部字体文件。索引 MUST 按
`fonts.xml` 文档顺序返回并精确去重，不排序、不归并；
`files/home/.termux/fonts` 投放的字体追加在后。

族索引的构建 MUST NOT 发生在渲染状态锁内：它要读完全部声明字体文件（真机实测
127ms），而渲染线程每帧都要取这把锁，构建期间终端完全冻结。族已在渲染字体库中时
（常驻三族、用户选文件装入的面）MUST NOT 触发该枚举。

#### Scenario: 渲染不触发族枚举

- **WHEN** 只启动终端不做字体设置
- **THEN** 族索引日志 `FONT_INDEX` 不出现

#### Scenario: 装载字体文件不触发族枚举

- **WHEN** 用户选定字体文件（`loadFontFile`）
- **THEN** 族索引 MUST NOT 被构建，该文件 MUST 直接装入渲染字体库并可按族名选中

#### Scenario: 按需构建不改变列表顺序

- **WHEN** 打开字体列表
- **THEN** 前若干项与 `fonts.xml` 的 family 声明顺序一致

#### Scenario: 打开字体列表不冻结终端

- **WHEN** 用户打开字体选择器，族索引尚未构建
- **THEN** 族索引在渲染状态锁外构建，渲染线程 MUST NOT 被阻塞

### Requirement: 字体列表取自渲染侧字体库

字体列表 MUST 取自渲染侧字体库（Android：`font_db::family_index` 的展示名；宿主
构建：渲染字体库内的等宽族），该库内容即 `/system/etc/fonts.xml` 声明的文件集加
用户投放目录，故列表与渲染侧可选择的字体必然一致。列表 MUST 按 `fonts.xml` 文档顺序
返回、按族名精确去重，MUST NOT 做名称归一（不把 `DroidSans` 与 `Droid Sans` 视为
同一）、不排序、不硬编码字体名，也 MUST NOT 出现「系统默认」「从文件加载」等含糊行。
平台 `SystemFonts.getAvailableFonts()` 不可用于此：它返回 `Set<Font>` 而 SDK 无公开
族名访问器，且未装入渲染库的文件本就无法被选择。字体库为空时 MUST 输出日志并抛
`IllegalStateException` 崩溃退出；`fonts.xml` 缺失或不可解析由 native 侧直接 abort。

族索引是进程级缓存、与渲染状态无关，其列表构造 MUST NOT 持有渲染状态锁。

#### Scenario: 列表与可生效字体一致

- **WHEN** 打开字体选择对话框
- **THEN** 列表中任一 family 都能被 `setFontFamily` 成功应用

#### Scenario: 列表顺序等于 fonts.xml 文档顺序

- **WHEN** 打开字体列表
- **THEN** 顺序与 `fonts.xml` 的 family 声明顺序一致，无排序痕迹

#### Scenario: 字体库为空即崩溃

- **WHEN** 字体库为空
- **THEN** 输出日志并抛 `IllegalStateException`，不返回空列表

## ADDED Requirements

### Requirement: 按需装入族时族文件列表必须在渲染状态锁外解析

族不在渲染字体库中时需要按需装入，其族文件列表来自族索引，解析 MUST 在渲染状态
锁外完成：解析可能触发族索引构建（真机实测 127ms），持锁解析会把渲染线程冻结
同样时长。锁内 MUST 只做「把该族的文件装进渲染字体库」与「选中该族」两件事。

族文件列表的解析 MUST NOT 由渲染管线自行触发：管线住在渲染状态里，它自己解析
就等于在锁内解析。调用方 MUST 先判定族是否已在渲染字体库中，已在库中时 MUST NOT
解析族文件列表。

#### Scenario: 应用已存字体族不冻结终端

- **WHEN** 会话启动时应用一个不在渲染常驻字体库中的已存字体族
- **THEN** 族文件列表在渲染状态锁外解析，渲染线程 MUST NOT 被阻塞

#### Scenario: 常驻族不解析族文件列表

- **WHEN** 选中的族已在渲染字体库中（常驻三族或用户选文件装入的面）
- **THEN** 族文件列表 MUST NOT 被解析，族索引 MUST NOT 被构建
