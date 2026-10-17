# Tasks

- [x] 查实 openDocument 存在性检查的安全前提：createDocument 的 sanitize 不拦点文件
- [x] 移除该检查，把创建交给模式位；只读仍自然失败
- [x] 补树 URI 的修改/复制/移动端到端断言
- [x] 补 MODE_CREATE 创建、只读拒绝、越权拒绝、点文件可创建（钉住旧前提不成立）
- [x] 改写 openDocument_never_creates_a_missing_document（其断言与现行为相反）
- [x] 删除 152 个 vendor 夹具与上游 LICENSE，验证与标签 5.5.0 逐字节一致（实删 154 个文件，另含 PROVENANCE.md，见补记）
- [x] flake.nix shellHook 按标签克隆语料到 target/xterm-js，失败不阻断进 shell（口径更正：未落地，见补记）
- [x] 语料定位收敛到 tests/common/mod.rs 一处，vt_conformance.rs 不再自写路径（口径更正：该文件从未存在，见补记）
- [x] 验证语料缺失时测试失败（exit 101）且报错可操作
- [x] 加 6 条仓库卫生 semgrep 规则（许可证 / git-cargo 命令 / git 提交号）
- [x] 修复规则暴露的既存违规：归档提案里的项目名小写形式
- [x] 删除/改写三处无意义测试（不崩溃即断言、名不符实、零增量包装）

## 归档后补记（第 2 节语料机制与史实不符）

第 2 节与上面三条勾选项描述的「devShell shellHook 按标签克隆到 `target/xterm-js`」与
「语料定位收敛到 `tests/common/mod.rs`」从未落地，据实登记：

- `target/xterm-js` 在全史中只出现在本提案自身（`git log --all -S "target/xterm-js"`
  只命中收录本文件的 `aeb67297`）；`flake.nix` 从未含 xterm.js 语料克隆。
- `native/tests/common/mod.rs` 从未存在（无新增/删除记录）。
- 实际的去 vendor 路径：先删夹具（`97009346`），再把语料内联进
  `native/tests/vt_corpus.rs`（`aeb67297`，提交题注即「把 VT 语料内联进源码」）。
  该文件与宿主 `native/tests/vt_conformance.rs` 在 36 分钟后随「上游 VT 语义不由
  本仓测试覆盖」一并删除（`a46e19ad`）。现况是 `native/tests/external_benchmarks/`
  运行期取数，落地形态见常驻规范 `upstream-alignment` 的同名 Scenario。
- 夹具实删 154 个文件：152 份用例（76 `.in` + 76 `.text`）加上游 `LICENSE` 与
  `PROVENANCE.md`。
- 本变更其余任务在实现中可验：SAF 模式位（`DocumentsProviderTest.kt`、
  `DocumentsProviderInstrumentedTest.kt`）、六条仓库卫生规则（现存于
  `.semgrep/rules.yml`，由 `scripts/check.nu` 调用；当时的 `rust-deny-patterns.yml`
  与 `check-rust.nu` 已于 2026-10-12 合并）、三处无意义测试的删除与合并
  （`emoji_glyph_no_panic_when_color_font_cannot_outline`、
  `dayThemeShowsDefaultNameInDayNightSection`、`session_spawn_and_exit` 均已不在）。
