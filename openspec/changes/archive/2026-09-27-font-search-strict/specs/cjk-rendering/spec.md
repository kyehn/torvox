## MODIFIED Requirements

### Requirement: CJK 回退字体遵循系统 fonts.xml

CJK fallback MUST 遵循 Android fonts.xml：zh-Hans 链首选 `NotoSansCJK-Regular.ttc`
index=2（Noto Sans CJK SC），MUST NOT 回退到 Serif/JP。`FontPipeline::find_cjk_fallback_fonts`
按 fonts.xml 顺序匹配，首个命中即用。匹配 MUST 为纯 `(文件名, ttc_index)` 精确命中，
MUST NOT 使用族名子串启发式或打分排序：族名在不同 ROM 间不稳定，启发式会把
Noto Serif CJK 或 Noto Sans CJK JP 排到 SC 之前。

#### Scenario: 简体中文环境选中 CJK SC

- **WHEN** 系统 locale 为 zh-Hans 且需要 CJK 字形回退
- **THEN** 选中 Noto Sans CJK SC，不使用 Noto Serif 或 Noto Sans CJK JP

#### Scenario: 同名族不靠打分取

- **WHEN** 库内同时存在 Serif 与 Sans 两个 CJK 族且 fonts.xml 先声明 Sans
- **THEN** 命中 Sans 的精确面，不因族名启发式改序
