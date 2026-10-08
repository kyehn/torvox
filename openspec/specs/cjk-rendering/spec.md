# cjk-rendering Specification

## Purpose

CJK 字形在终端中的渲染质量与输入链路要求。记录设备取证结论、根因与已验证的实现细节，作为后续渲染/输入工作的参考（置信度低于 docs/specification/DESIGN.md）。

实现细节：重建逻辑为 `native/src/android/ffi.rs` 的 `rebuild_font_pipeline`，与调用点同受 `#[cfg(target_os = "android")]` 约束（两个调用点都在 JNI 导出的安卓分支内，主机构建不编译该函数）；`loadFontFile` 在重建后再设字体族，`setExtraFontPaths` 不设。

## Requirements

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

### Requirement: CJK 字形按位图物理像素 1:1 取样

CJK 字形 SHALL 按 atlas 位图物理像素 1:1 采样，MUST NOT 按
`glyph_advance / quad_size` 比率缩放（等宽西文 advance==cell 时比率恒为 1.0，
掩盖了公式错误；CJK quad 与 advance 不等会导致水平拉伸约 1.197x）。

#### Scenario: 中英文混排字形大小协调

- **WHEN** 同一终端同时显示西文与 CJK 字形
- **THEN** CJK 笔画不增粗，中/文/测/试宽度与 FreeType 无 hint 参考一致（比率 1.000）

### Requirement: IME 提交的 CJK 文本首帧可见

IME commitText 经 TerminalInputEncoder UTF-8 编码、InputBatchBuffer 单线程发送、
bridge.writeToPty 进入 PTY 后，提交的 CJK 文本 MUST 在首帧渲染可见，无需额外点击。
退格按码点 1:1（CJK 一次一个汉字）。

#### Scenario: 拼音提交后立即显示

- **WHEN** 冷启动后一次点击开键盘，经拼音 SPACE 提交“你好”
- **THEN** 终端立即显示“你好”，无需额外点击

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

### Requirement: 字体管线重建统一失效实例缓存并请求新帧

任何重建字体管线的入口 MUST 在替换管线后无条件失效字形实例缓存并请求新帧：新管线
重新分配图集，旧实例携带的 UV 指向旧图集，混用会导致字形错乱且不会主动重绘。

重建入口 MUST 共用同一实现，MUST NOT 各自复制序列——复制会使某入口遗漏上述两步，
且遗漏处不会产生任何日志。

#### Scenario: 通过字体目录重建后字形立即正确

- **WHEN** Termux 字体目录存在并注册到字体数据库后重建管线
- **THEN** 实例缓存失效且请求新帧，终端以新字体渲染
