# cjk-rendering Specification

## Purpose

CJK 字形在终端中的渲染质量与输入链路要求。记录设备取证结论、根因与已验证的实现细节，作为后续渲染/输入工作的参考（置信度低于 docs/specification/DESIGN.md）。

## ADDED Requirements

### Requirement: 区域回退族按 locale 增补而非冻结

字体库构建时若 locale 尚未到达（Kotlin 侧按会话调用在 `sessionId == 0` 时被丢弃，
spawn 后重放的 locale 晚于库定型），区域回退族 MUST 能在 locale 到达后被补装进
**活动字体库**。实现 MUST 只增补缺失的面而不得重建整个库：`fontdb::ID` 是库内序号，
重建会使主字体 `font_id` 指向另一个面。同一文件已装入时 MUST NOT 重复装入。

CJK 回退层的发现 MUST NOT 依赖 locale 早于字体库构建这一调用顺序。

#### Scenario: locale 晚于字体库构建到达

- **WHEN** 字体库在 locale 为空时已定型（仅主字体与符号族）
- **AND** 随后 `setSystemLocale("zh-CN")` 到达
- **THEN** 区域族被补装，`find_cjk_fallback_fonts` 找到 fonts.xml `zh-Hans` 块声明的面
- **AND** 汉字渲染出真实字形而非 `.notdef`，`CJK_FALLBACK: found` 计数大于零

#### Scenario: 补装幂等

- **WHEN** 同一 locale 重复到达
- **THEN** 库内面数不变，区域族不重复装入

#### Scenario: 非 CJK locale 不补装

- **WHEN** locale 为 `en-US`
- **THEN** 不补装任何 `lang` 族，CJK 回退层保持为空
