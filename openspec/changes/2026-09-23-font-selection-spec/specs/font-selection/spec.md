# font-selection Specification

## ADDED Requirements

### Requirement: 字体列表来自 fonts.xml

字体列表 MUST 按文档顺序返回 `/system/etc/fonts.xml` 中 `<family name>` 属性值，不改写、不去重、不排序、不硬编码。文件缺失或不可解析时 MUST 输出日志并崩溃退出。

#### Scenario: 列表与系统一致

- **WHEN** 打开字体选择对话框
- **THEN** 列表内容等于 `fonts.xml` 的 family 名序列，无“系统默认”、无“从文件加载”行

#### Scenario: fonts.xml 缺失即崩溃

- **WHEN** `/system/etc/fonts.xml` 不存在或无法解析
- **THEN** 输出日志并崩溃退出，不静默回退

### Requirement: font.ttf 存在即默认

`files/home/.termux/font.ttf`（或同名 `.ttc` `.otf`）存在时 MUST 将其探测到的 family 作为生效字体，不复制不移动该文件。

#### Scenario: 复制 font.ttf 后生效

- **WHEN** `font.ttf` 存在
- **THEN** 生效字体为其 family，界面显示一致（`TESTING.md` 设备项）

### Requirement: 用户字体目录

`files/home/.termux/font` 目录下字体文件 MUST 出现在列表中（经 `setExtraFontPaths` + `listFontFamilies`，无手动判断）。

#### Scenario: 目录字体可选

- **WHEN** 该目录存在字体文件
- **THEN** 其 family 可在对话框中选择并生效

### Requirement: 设置错误重置应用数据

用户设置的 family 经 native 确认不存在时 MUST 输出日志并清除该设置，不 toast 掩盖。

#### Scenario: 无效 family 被清除

- **WHEN** `setFontFamily` native 返回 false
- **THEN** 该设置被清除并回落系统默认
