## Why

`docs/specification/TESTING.md` 的「覆盖范围」要求锁定回滚行序、输入回显与光标、简体中文显示宽度、
字重与颜色文本等终端整体状态。当前这些断言由 `native/src/terminal/testdata/` 的 10 份手写语料
（`snapshot_test.rs` 的语料运行器驱动）完成，输入全部由本项目作者自行构造：

- 语料输入的覆盖面等同于作者当场想到的转义序列，与真实应用输出（vim、tmux、zsh、fish、vttest）
  以及上游对解析器的边界构造没有交集。
- 终端把哪些码位当双宽、哪些区段的中日韩字符会被识别成宽字符，没有任何一处与权威数据对照。
- 手写区间表（`render/font/shaping.rs` 的 CJK span 判据）缺少哪些区段无法被发现。

同时，本项目以 `libghostty-vt` 作为终端状态的单一来源（`DESIGN.md`），而上游 Ghostty 与
Alacritty 各自维护着可直接复用的测试资产：前者为 libghostty-vt 三个解析入口提供手写种子语料，
后者为真实应用输出提供 PTY 录音及其网格期望。二者均为许可宽松的数据资产，且都不需要引入
第二套终端引擎，也不新增测试体系。

因此本次调研 kitty、rio、wezterm、alacritty、ghostty、xterm、foot、vttest 与 esctest2
九个上游项目的测试体系与测试数据，采纳其中可提高覆盖率或可减少本地实现的资产，并如实记录
不可采纳者的原因。

## What Changes

- 导入 ghostty `test/fuzz-libghostty/corpus/` 下手写的三组种子语料（`parser-initial`
  `stream-initial` `osc-initial`，共 94 份、实际内容 5.2KB，MIT）：作为「分块写入不变性」
  测试的输入集。上游各 fuzz 目标的首字节是目标选择器而非 VT 输入，导入时去除并在文档中记录。
- 导入 alacritty `alacritty_terminal/tests/ref/` 下通过实验比对的 27 份真实应用录音
  （`alacritty.recording`，Apache-2.0）及其由本引擎产生的期望屏幕，纳入语料运行器，
  作为跨引擎一致性锁定。跨引擎语料只断言屏幕文本、光标与尺寸；样式与回滚区另行说明。
- 语料运行器改为从期望文件读取终端尺寸与回滚上限，移除 `CORPUS_ROWS`/`CORPUS_COLS`
  全局硬编码，使不同尺寸的语料共用同一运行路径。`TestSnapshot` 增加 `scrollback` 字段，
  `SNAPSHOT_VERSION` 递增，存量 10 份期望文件随之重生成。
- 新增宽度分类一致性测试：对全部中日韩表意与音节区段共约 18 万个码位，断言引擎给出的
  单元宽度与 `unicode-width` 的权威分类一致。该 crate 已在依赖图中（`cosmic-text` 传递依赖），
  直接声明为零编译增量的开发依赖。
- 新增 `.gitattributes`：将所有语料目录标记为不做换行规范化，避免 `core.autocrlf` 改写语料字节。
- 新增两份资产的来源与许可说明文件。

不新增 crate features、不新增测试体系、不 import 任何上游代码，全部进入现有 `cargo test`。
`flake.nix` 不可修改（`AGENTS.md` 禁止），因此任何需要新增开发环境依赖的方案均不在本次范围内。

## Capabilities

### New Capabilities

- `upstream-vt-test-assets`：上游终端项目的测试资产以数据形式纳入本项目既有测试体系，
  覆盖分块写入不变性、跨引擎一致性与权威宽度分类三类断言。

### Modified Capabilities

- `terminal-state-regression`：语料尺寸与回滚上限由期望文件给出而非运行器硬编码；语料来源
  扩展为上游手写种子与真实应用录音；新增分块写入不变性与宽度分类一致性两类锁定。

## Impact

- `native/src/terminal/testdata/`：新增 `seeds/` 与 `conformance/` 两个子目录及对应期望文件，
  以及资产来源与许可说明。
- `native/src/terminal/snapshot_test.rs`：快照表示增加 `scrollback`，语料运行器按期望文件
  创建终端，原有分块处理逻辑与差异报告不变。
- `native/Cargo.toml`：开发依赖新增 `unicode-width`（已在 `Cargo.lock` 中，无新增编译单元）。
- `.gitattributes`：新增，语料目录整体 `-text`。
- 不新增第三方终端实现依赖，不修改生产代码。
