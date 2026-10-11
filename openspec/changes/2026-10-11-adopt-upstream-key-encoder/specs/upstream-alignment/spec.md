## MODIFIED Requirements

### Requirement: 上游 API 取用判定登记

本仓以 Ghostty 为终端状态单一来源：上游已有的能力 MUST 直接取用，MUST NOT 重实现；
上游有而本仓不用的能力 MUST 在此登记「不采用」的 DESIGN 依据或「待办」的阻塞点，
MUST NOT 留空使后来者重复调研或把合规分叉移植回来。每项 MUST 给可复核依据
（上游源码位置、本仓文件行号或 DESIGN 条款）。

#### Scenario: 结论可脱离审计报告独立复核

- **WHEN** 复查某项上游 API 的取用与否
- **THEN** 该项结论与依据 MUST 能在本规范内读到，MUST NOT 需要先翻审计报告或归档变更

上游 API 取用判定如下（上游位置均指 `Cargo.toml` 所钉 rev 的 `crates/libghostty-vt/src`）。

| 上游能力 | 结论 | 可复核依据 |
| --- | --- | --- |
| `terminal.rs`：`on_clipboard_write` 必须 `reply` | 采用（已修） | 上游文档「返回前必须 `reply`，否则拒绝写入」；本仓 `internal.rs` 回调此前零 `reply`。现满通道回 `Busy` 否则 `Ok` |
| `style::Underline` 五形状判定 | 采用（已修） | 三处 `matches!` 收敛为 `has_visible_underline`/`is_double_underline` 同源函数 |
| `Terminal::vt_write` 二进制安全 | 采用（已修） | `pty_write` 曾把 `>0xF7` 改空格，破坏 Kitty 直接 RGB 等二进制载荷；现仅剔除 NUL，与 `vt_write` 同策略 |
| 调色板与字节转浮点 | 采用（已修） | 渲染侧 `CATPPUCCIN_MOCHA_BACKGROUND` 与手写除法已删，以 `catppuccin_mocha_palette` 与 `byte_to_float` 为单源 |
| `selection` 交叉翻转归属 | 采用（已修） | 对标 termlib `applyHandleDrag` 五元组：回传新归属柄并收紧等值判据，调用方更新 `handleDragState` |
| `key::Encoder` 按键编码 | 采用 | 原阻塞点「Android 键码到上游 `Key` 的映射裁定与 JNI 桥形态」已解除：映射为无行为判断的机械对应表，桥形态与既有 `Query::EncodeMouseEvent` 同构（编码在 VT 线程，底层 `Terminal` 与编码器同处该线程）。行为契约见 `key-input-encoding` |
| `search::Search` 全文搜索 | 不采用 | DESIGN 要求「使用库（如 `regex`）实现文本搜索」；自研 `regex` 搜索合规，上游 `Search` 的视口导航语义与本仓批量查询不一致 |
| `selection/gesture` 手势、`select_line`/`select_output`、`format_selection_*` | 不采用 | DESIGN 禁双击三击多击选择；格式化器已选用 `fmt::Formatter`，双格式化器并存需注释说明而非迁移 |
| termlib `trimDetectedUrl`/括号计数/跨行拼接 | 不采用 | DESIGN「链接识别通过 `libghostty-vt` 查询，没有额外检查」即禁止移植；仅保留空白 trim 与协议白名单（属安全检查） |
| `on_bell`/`on_title_changed`/`on_pwd_changed`/`on_render_hold`/`on_desktop_notification`/`on_progress_report` 等回调 | 不采用 | Kotlin 侧零消费者；DESIGN「不做不必要功能」，有消费者时再注册 |
| `track_grid_ref`、`width_px`/`height_px`、光标色实时查询、`set_mode`、`vt_write_until_ground` | 不采用 | 按需取用；现有轮询与原子镜像满足当前消费者，DESIGN 要求最小实现 |
| `Terminal::paste` 粘贴 | 待办 | 生产粘贴未走上游粘贴事件与 `is_safe` 拒绝流程；阻塞于模式查询归属裁定与禁确认对话框下的等价路径，另立变更 |
