## ADDED Requirements

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
