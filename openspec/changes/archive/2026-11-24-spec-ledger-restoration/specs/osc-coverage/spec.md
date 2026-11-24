# osc-coverage Specification

## Purpose

OSC 段的处理归属与核对表：每个常见 OSC 段在本仓的行为归属（上游解析并观察／上游处理且有测试／默认色归内嵌方走原生 setter／本仓收割应答／不接）与可复核依据。本仓不自建 OSC 解析器，核对表使各段的归属可脱离代码检索直接读到。

## ADDED Requirements

### Requirement: OSC 覆盖逐项核对

外部终端生态的 OSC checklist（xterm/ECMA-48 常见段：0/1/2、4、7、9、10/11/12、
52、104、133、1337、777）MUST 在本规范内有一张逐项核对表，写明每一段在本仓的
行为归属（上游解析并观察 / 上游处理且有测试 / 默认色归内嵌方走原生 setter /
本仓收割应答 / 不接）与可复核依据（文件与行号）。表 MUST 覆盖各段而非概括表述；
上游不处理或本仓不消费的段 MUST 注明理由，MUST NOT 留空。

生态参照面为 `8dmusichannels-star/terminator` 的 OSC 注释清单（该仓 CI 虚假绿灯、
零测试资产，仅其 checklist 构成对照）；判定口径与「上游 API 取用判定登记」一致——
本仓不自建 OSC 解析器（`output_processor.rs` 模块文档），除 OSC 52 读取请求的
收割外不再自研识别。

#### Scenario: 某段 OSC 的行为可脱离代码检索直接读到

- **WHEN** 复查某个 OSC 段在本仓的行为
- **THEN** 在「OSC 覆盖逐项核对」表内读到该段的行为归属与文件行号依据，
      MUST NOT 需要先翻归档变更或全仓 grep

#### Scenario: 上游不处理的段注明归属

- **WHEN** 某 OSC 段的语义上游不处理（如默认色查询）
- **THEN** 表内 MUST 写明其归属（内嵌方）与本仓的置色入口（原生主题 setter），
      MUST NOT 记为「上游处理」

核对表如下。

| OSC 段 | 本仓行为 | 依据 |
| --- | --- | --- |
| 0 / 2（图标名 / 窗口标题） | 上游解析；标题经 `Query::Title` 与快照 `title` 观察 | `internal/dispatch.rs` 的 `Query::Title` 与快照 `title`；`tests/grid_text.rs`、`tests/osc_notification.rs`、`tests/selection_search.rs` 的 OSC 0/2 标题可读用例（含跨分块重组） |
| 1（图标名查询） | 上游解析；本仓无消费者，不另设查询 | 标题查询已承载用途；`on_title_changed` 类回调不采用见「上游 API 取用判定登记」 |
| 4（调色板覆盖） | 上游处理且有测试钉住生效路径 | `internal/run.rs` 初始主题注释；`internal/tests.rs` 的 `osc4_palette_override_reaches_dumped_grid` |
| 7（工作目录） | 上游解析，本仓不消费；透传行为有测试 | `output_processor.rs` 模块文档与 `osc7_and_osc8_pass_through`；`on_pwd_changed` 不采用见「上游 API 取用判定登记」 |
| 8（超链接） | 上游解析；本仓经 `hyperlink_at` 查询网格单元 URI | `internal/selection.rs` 的 `hyperlink_at_impl`（4KiB 内联缓冲 + 越界精确重读） |
| 9（iTerm2 通知） | 上游解析并观察：本仓注册 `on_desktop_notification` 回调，通知按会话归属投递 | `DESIGN.md` 软件节要求处理 OSC 9 通知（前台 Toast、后台系统通知、点击切换会话）；回调注册见 `ghostty_terminal/internal/run.rs`，分派见 `runtime/DesktopNotificationPlayer.kt`，规约见 `desktop-notification` |
| 10 / 11 / 12（默认前背景色 / 光标色查询与设置） | 上游不处理，默认值归内嵌方；本仓经原生主题 setter 置默认色 | `internal/run.rs` 初始主题段落三处注释（默认色归内嵌方、OSC 4 覆盖归上游、故走原生 setter 而非手写 OSC 序列） |
| 52（剪贴板读写） | 写走上游回调；读取请求由本仓扫描器收割并拼帧应答 | `pty-lifecycle`「OSC 52 读取请求的收割与应答须对每个会话有界」；`output_processor.rs` `ReadScan` 状态机 |
| 104 / 110 / 111 / 112（色重置） | 上游处理；跨分块重组有测试 | `tests.rs` OSC color reset 跨分块用例 |
| 133（shell 集成标记） | 上游解析（若支持）；本仓不消费 | DESIGN 未声明 prompt 标记语义；`research-ledger`「能力裁定补齐行」的「输出分块与命令状态着色（OSC 133 语义）」行判「未声明，不立项」 |
| 777（rxvt 桌面通知） | 上游解析并观察：走上游 `on_desktop_notification` 回调（同 OSC 9） | 上游 `rxvt_extension.zig` 把 `777;notify;标题;正文` 归到 `show_desktop_notification`，与 OSC 9 同一条命令 |
| 1337（iTerm2 媒体 / 通知） | 不接：零消费者 | 上游 `iterm2.zig` 只实现 `Copy` 与 `CurrentDir`，通知类负载（`RequestAttention`）上游记为未实现；本仓不消费 |

#### Scenario: 新增 OSC 消费者时先补 DESIGN

- **WHEN** 拟为某个 OSC 段新增本仓消费者（如 cwd 展示、桌面通知）
- **THEN** MUST 先在 `DESIGN.md` 声明该功能再接线，
      MUST NOT 以「上游已解析」为由直接消费
