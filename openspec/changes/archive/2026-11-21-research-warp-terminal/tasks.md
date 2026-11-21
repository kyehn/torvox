# Tasks

## 一、研究

- [x] 1.1 核验 Warp 工作区与 crate 分层：`app` + 82 个 crate、依赖方向、`publish = false`、许可证分工
- [x] 1.2 核验渲染与 UI 框架：`warpui`/`warpui_core`、Metal 主路径、wgpu 实验性、`winit`、字形/图片缓存与 damage 跟踪
- [x] 1.3 核验终端模拟层：`warp_terminal`（alacritty 血统 + `vte` 状态机）、`model/` 模块、`Mode` 枚举、Kitty/iTerm 图像、无 sixel
- [x] 1.4 核验输入与编辑：`vim` crate、`editor` crate（sum_tree）、`input_classifier`、修饰键与 keymap
- [x] 1.5 核验命令块与输出：`blockgrid`/`block_id`/`block_index`/`displayed_output`/`filtering`/`secrets`、`secret_redaction` crate
- [x] 1.6 核验 AI 与代理：`ai`/`ai_types`/`computer_use`/`computer_use_mcp`/`mcp`/`agents`/`code_review`
- [x] 1.7 核验云与同步与持久化：`cloud_objects`/`persistence`/`firebase`/`graphql`/`auth`/`billing`/`drive`/`notebooks`/`env_vars`/`launch_configs`
- [x] 1.8 核验远程与控制：`remote_server`/`local_control`/`ipc`/`session-sharing-protocol`
- [x] 1.9 核验搜索与补全与 Drive：`warp_search_core`/`warp_completer`/`fuzzy_match`/`lsp`/`repo_metadata`/`natural_language_detection`
- [x] 1.10 核验平台与构建：macOS/Linux/Windows/FreeBSD/Web、`flake.nix`、`rust-toolchain.toml`（1.94.0）、`deny.toml`、clang-format/wgslfmt
- [x] 1.11 核验测试与质量：`ref_tests`（40 组 golden）、`escape_sequences_tests`、13.7k 个 `#[test]`、cargo-deny
- [x] 1.12 核验许可证与治理：AGPL v3 主体 + MIT（warpui）、CONTRIBUTING/CODE_OF_CONDUCT/SECURITY、Factory 代理

## 二、报告

- [x] 2.1 写 `docs/research/warp-terminal-research-2026.md`：研究方法与坐标、Warp 形态与架构、逐项对照表、可采纳性判定、事实/推论/未知、不做清单、复现方式
- [x] 2.2 报告正文不写 git 提交号与被禁 API 名称，外部仓库坐标用快照标识；避免禁用词（fish/dash/zsh 小写）与 nix store 路径

## 三、登记

- [x] 3.1 `research-ledger` 索引表新增 `warp-terminal-research-2026.md` 行：实质、判定、依据
- [x] 3.2 `research-ledger`「在库报告的登记与计数」场景补记在库五份
- [x] 3.3 `research-ledger` 补齐行台账新增十二行：秘密脱敏、命令块、自研 VT 核、自研 UI 框架与 Metal 渲染、AI/代理与 MCP、云同步与 Drive 与环境变量编辑、远程与控制与会话共享、Vi 模式与内置编辑器、命令补全/LSP/仓库元数据、ref_tests 测试资产、escape_sequences_tests、多平台支持

## 四、验证

- [x] 4.1 `semgrep scan --config .semgrep/rules.yml` 对新增文档零命中
- [x] 4.2 `markdownlint-cli2` 对新增文档零告警
- [x] 4.3 `openspec validate --all --strict` 通过
- [x] 4.4 受保护文件零改动
