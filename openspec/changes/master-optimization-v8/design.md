# Design: Master Optimization v8

> 日期：2026-09-01 | 状态：活跃 | 关联：`proposal.md`

## 架构决策

| ADR | 决策 | 依据 | 不做 |
|-----|------|------|------|
| ADR-0001 | 外部依赖优先 | shell-words 拆词、foldhash 高频、cosmic-text 排版 | 不自研分词/哈希/排版 |
| ADR-0002 | 保守抽取 | mouse encoder 保持 public_api 内联 | 不建 mouse_encoder.rs |
| ADR-0003 | 语义段轻量 | Rust 侧仅 Vec<SemanticSegment> 列范围 | 不引入新状态机 |
| ADR-0004 | CellRun 兼容 | 合并相邻同样式为 run 仍展开为 CellInstance | 不改 FFI |
| ADR-0005 | 单后端 | 模拟器 lavapipe Vulkan，不引 GL 退路 | 不启用双后端 |
| ADR-0006 | 超时固化 | lens 120s、CI 120s、cargo 8 线程 | 不拆 nextest |
| ADR-0007 | 确定性 | PTY 侧决定 flush，渲染侧不补 buffer | 不双 buffer |
| ADR-0008 | 文档即规范 | openspec/changes 为长期真相 | 不在 specification 外另建规范 |
| ADR-0009 | 代码精简底线 | net LoC ≤ 0，新增仅测试/注释/常量命名 | 不增抽象/依赖/配置 |

## 26 参考项目像素级复制策略

### P0（立即吸收，低成本高价值）

| # | 吸收项 | 来源 | Torvox 落点 |
|---|--------|------|-------------|
| 1 | 多击选择 tapCount | ghostty-android TerminalView:1051-1083 | TerminalSurface.kt 手势层 |
| 2 | Callback2 + onGetContentRect 菜单锚定 | termux-app TextSelectionCursorController:194-215 | SelectionActionCallback |
| 3 | 手柄拖动边缘滚动 + 宽字符吸附 | termux-app TextSelectionCursorController:218-306 | TerminalSurface.kt HandleDrag |
| 4 | wrap 感知拼接 + 列→char 换算 | termux-app TerminalBuffer:52-106 | TerminalViewModel |
| 5 | IME composing diff | warp WarpInputView:587-615 | InputCoalescer |
| 6 | bootstrap sha256 sidecar | warp bootstrap.rs:1-48 | BootstrapInstaller.kt |
| 7 | PTY 初始 winsize 先于首读 | warp WarpTerminalService:797-808 | TerminalRuntime spawn |
| 8 | ArgumentTokenizer 安全拆分 | termux-kotlin ArgumentTokenizer.kt | MCP cmd 工具 |
| 9 | MCP SO_PEERCRED 校验 | termux-kotlin AmSocketServer | mcp.rs UnixStream |
| 10 | 终端环境分层 EnvOp overlay | zed-port env.rs:19,52 | shell_env.rs |
| 11 | URL 超链接正则 + 尾标点清理 | zed-port terminal_hyperlinks.rs:20,177 | url_regex.rs |
| 12 | 前台进程组跟踪 + 组杀 | zed-port pty_info.rs:29-53 | pty.rs / session.rs |
| 13 | OSC 133 语义段模型 | termlib TerminalEmulator.kt:830-930 | Rust ghostty OSC 133 |
| 14 | CellRun 游程编码 | termlib CellRun.kt:25-74 | cell_builder.rs |
| 15 | 行级脏缓存 | zelland renderer/mod.rs row_cache | pass.rs / cell_builder.rs |
| 16 | 搜索收窄保持当前匹配 | GNOME Console kgx-tab.c:191-250 | TextSearchBar.kt |

### P1（近期，中等成本）

| # | 吸收项 | 来源 |
|---|--------|------|
| 1 | 自绘放大镜 | termlib MagnifyingGlass |
| 2 | 智能复制 smartCopy | Haven SelectionToolbar.kt:357-405 |
| 3 | 自定义主题链路 | ghostty-android ThemeStore:110 |
| 4 | LiveTest 协议 | fission fission-test-driver:37-609 |
| 5 | 无 TTY 确定性帧 + PNG 截图 | fission frame.rs:80-173 |

## 实施分解

### 阶段 0 — 基线冻结与债务对齐

- [x] 恢复 docs/reference 44 文件
- [x] 修复 cell_builder 索引 bug
- [x] cargo test 995+32 暖缓存基线（57s）

### 阶段 1 — 代码修复

- [x] DEFAULT_LANG en_US.UTF-8 → C.UTF-8（DESIGN spec §Shell）
- [x] 常量化 magic numbers（DEFAULT_CURSOR_BLINK_SPEED_MS, DEFAULT_FONT_CELL_SIZE, CLIPBOARD_POLL_INTERVAL_MS, DEFAULT_TERMINAL_ROWS/COLS）
- [x] GPU 测试 `#[ignore]` 门控（22 个 GPU-dependent 测试）
- [ ] 注释溯源（zelland/termlib/warp 引用）

### 阶段 2 — 文档与 OpenSpec

- [x] OPENSPEC-STATUS.md v1.1
- [x] 本计划 + 测试计划 + 验证协议
- [x] v8 proposal/design/tasks 文档

### 阶段 3 — 自动化验证

- [x] verify-build-artifacts.nu
- [x] verify-emulator.nu
- [x] verify-cjk.nu
- [x] verify-all.nu（一键执行）

### 阶段 4 — 三轮审阅循环

- [x] 第 1 轮 review（双空行修复）
- [x] 第 2 轮 review（全代码质量检查通过）
- [ ] 第 3 轮 review
