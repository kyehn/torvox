## MODIFIED Requirements

### Requirement: 选择菜单内容

菜单条目 MUST 有且只有：无内容区域长按 → 粘贴，且剪贴板无文本时 MUST NOT 显示菜单；有内容区域 → 复制、分享、全选，选区起点存在 OSC 8 超链接时 MUST 含打开链接。条目 MUST 一次点击即生效。

#### Scenario: 剪贴板有文本时长按空白

- **WHEN** 剪贴板含文本且长按无内容区域
- **THEN** 菜单仅有粘贴

#### Scenario: 剪贴板无文本时长按空白

- **WHEN** 剪贴板无文本且长按无内容区域
- **THEN** 不显示菜单

#### Scenario: 内容区域完整条目

- **WHEN** 长按含 OSC 8 超链接的区域
- **THEN** 菜单为复制、分享、全选、打开链接

#### Scenario: 一次点击即生效

- **WHEN** 点击复制
- **THEN** 立即写入剪贴板且无需二次确认

## ADDED Requirements

### Requirement: 打开链接

打开链接 MUST 仅检查格式：选中文本非 URL 形态时 MUST 回退使用选区起点的 OSC 8 超链接 URI，经系统意图跳转且 MUST NOT 检查可达性。

#### Scenario: OSC 8 非 URL 文本打开

- **WHEN** 选中文本非 URL 形态但起点存在 OSC 8 超链接且点击打开链接
- **THEN** 经系统意图打开该超链接 URI

## REMOVED Requirements

### Requirement: 打开链接与打开文件

打开文件条目、文件路径形态匹配、点击时存在检查与读写权限授予一并移除，不再提供按选中文本打开文件的跳转。
