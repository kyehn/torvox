## Purpose

本能力规定终端渲染侧为 CJK 字符选择回退字体时 MUST 与 Android 系统 `fonts.xml` 定义的语言特定回退链保持一致，使用户看到的字形与 Termux 及系统其他应用相同。规范依据：`docs/specification/DESIGN.md` 设置·字体选择条款与终端·CJK 渲染条款。

## Requirements

### Requirement: CJK 回退字体跟随系统 fonts.xml

当主字体缺失 CJK 字形时，系统 SHALL 按以下顺序确定 CJK 回退字体：

1. 读取系统 `fonts.xml`（含 `fonts_fallback.xml`，若存在）中与当前系统 locale 匹配的 `<family lang>` 块所列字体文件；
2. 仅当上述文件在已加载字体库中不存在或无法提供可用字形时，才使用启发式扫描（家族名与 locale 匹配评分）作为补充；
3. 解析失败（文件缺失、格式无法识别）时 MUST 完全回退到启发式扫描行为，不得报错、不得无字体可用。

#### Scenario: 简中 locale 使用系统指定的 CJK 字体

- **WHEN** 系统 locale 为简体中文（`zh-CN` / `zh-Hans` / `zh`）且主字体不支持 CJK
- **THEN** CJK 回退字体 MUST 为 `fonts.xml` 中简中回退链首选的字体文件所对应的字体（如 Noto Sans CJK SC），不得为宋体风格的 serif CJK 字体，不得为日文/韩文变体字形

#### Scenario: 同一字体集合内选择正确的语言变体

- **WHEN** `fonts.xml` 多个 locale 块指向同一字体集合文件（如 `NotoSansCJK-Regular.ttc`）但变体序号不同（简中→2，繁中→3，日文→0，韩文→1）
- **THEN** 系统 MUST 选择与当前 locale 对应的变体字形，简中 locale 不得渲染出日文/韩文变体字形

#### Scenario: 主字体已支持 CJK 时跳过回退

- **WHEN** 主字体的字符映射表已覆盖 CJK 测试字符（中、日、韩文代表字）
- **THEN** 系统 MUST NOT 配置任何 CJK 回退字体，实际字体信息 MUST 标示回退已跳过

#### Scenario: fonts.xml 缺失或损坏时保持可用

- **WHEN** 系统不存在 `fonts.xml` 或其内容无法解析
- **THEN** 系统 MUST 回退到启发式扫描选择 CJK 字体，终端仍能渲染 CJK 字符，不得崩溃、不得显示空白

### Requirement: locale 与回退链的匹配规则

系统 SHALL 将当前 locale 映射到 `fonts.xml` 的语言标签：简体中文匹配 `zh-Hans`、`zh-CN`、`zh`；繁体中文匹配 `zh-Hant`、`zh-TW`、`zh-HK`、`zh`；日文匹配 `ja`；韩文匹配 `ko`。同一字体文件包含多个语言变体（如 TTC 集合）时，MUST 优先选择与当前 locale 一致的变体。

#### Scenario: 繁体 locale 不使用简体字形

- **WHEN** 系统 locale 为繁体中文（`zh-TW` / `zh-Hant` / `zh-HK`）
- **THEN** CJK 回退 MUST 优先繁体变体字体，不得使用简体变体

#### Scenario: 非 CJK locale 不配置 CJK 回退

- **WHEN** 系统 locale 为英文等非 CJK 语言（如 `en-US`）
- **THEN** 系统 MUST NOT 配置 CJK 回退字体，实际字体信息 MUST 标示无回退

### Requirement: 字体回退信息可观测

实际字体信息（设置页展示）MUST 包含主字体名称与 CJK 回退状态（已回退的字体名 / 已跳过 / 无），以便用户验证 CJK 字体是否与系统默认一致。

#### Scenario: 用户可验证 CJK 回退字体

- **WHEN** 用户打开设置中的实际字体信息
- **THEN** 能看到主字体名与 CJK 回退字体名（或跳过/无状态），且 CJK 字体名与系统默认 CJK 字体一致
