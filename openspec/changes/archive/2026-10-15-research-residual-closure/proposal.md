# Proposal

## Why

`docs/research/` 的 93 份研究报告（用户指定 commit 的编号文档集 31 份 + 当前工作区
63 份）已逐份通读，全部可行动项拿回仓库现状核对。绝大多数条目已被
`openspec/specs` 的 31 个能力覆盖、被 `PROHIBITED`/`DESIGN` 明禁、或属 `STYLE`
「不实现未声明功能」范畴，逐份处置台账见 `tasks.md`。核对后仍余 4 处真实缺口：

1. **release 构建档在清单合并时丢失**。原根 `Cargo.toml` 的
   `[profile.release]`（`lto = true`、`codegen-units = 1`、`opt-level = "z"`、
   `strip = true`）在「清单合并为单包并删除根清单」那次提交中未迁移到
   `native/Cargo.toml`，Android `.so` 现按 cargo 默认档构建（无 LTO、不 strip）。
   `docs/research` 的 pebrel 研究报告把「profile release optimization strategy
   （size LTO + hot path O3）」列为对 mobile Rust 的可借鉴项。
   该段里的 `panic = "abort"` **不得恢复**：`jni_export_guard` 的 panic 处理链
   依赖 unwinding（复审记录 §13.4 以「未设 `panic` 键」为 SIGABRT 排除依据）。
2. **OSC 覆盖无逐项核对表**。生态深度研究报告 §14 把 terminator 的
   OSC checklist（0/1/2、4、7、10/11/12、52、133、1337、777/9）列为「逐项核对
   libghostty-vt 覆盖」的 P1 参考点。核对结论散落在测试断言、函数注释与
   `upstream-alignment` 的回调判定表里，没有一张集中表，后续复查无法一次看全。
3. **切换会话的状态清空无规范条款**。实现已具备（清选区、关搜索、滚动归零、
   丢弃异会话渲染残留、重置输入目标），`docs/persistence/ecosystem.md` 的
   ghostty `attachSession` 对照项仍写「值得对照自查」——实现到位而契约缺失，
   回归时无判据。
4. **`docs/research` 逐份结论未沉淀回持久参考库**。研究目录的编号文档集已被
   后续提交删除，当前工作区的研究报告结论（Rust/GPUI、libghostty-vt 同引擎、
   同 wgpu 栈、conformance 外部真相候选等）尚未进入 `docs/persistence/ecosystem.md`
   的参照面表。

## What Changes

- **恢复 `native/Cargo.toml` 的 `[profile.release]`**：`lto = true`、
  `codegen-units = 1`、`opt-level = "z"`、`strip = true`；不设 `panic` 键
  （默认 unwind 是 `jni_export_guard` 的前提）。dev/bench 档不设——默认档
  满足测试与基准，`STYLE` 要求最小实现。
- **新增 `upstream-alignment` 能力「OSC 覆盖逐项核对」**：一张表列全
  terminator checklist 各 OSC 段的本仓行为（上游解析并观察 / 上游处理并有测试 /
  内嵌方所有走原生 setter / 本仓收割应答 / 不接），每项给文件行号依据。
- **新增 `session-list` 能力「切换活动会话清空全部会话态」**：切会话 MUST
  清空选择态、搜索态、滚动偏移、输入目标，并丢弃异会话渲染残留。
- **更新 `docs/persistence/ecosystem.md`**：ghostty `attachSession` 对照项
  由「值得对照自查」改为「已核对，五项均有实现」；参照面表补入本轮核对过的
  五份外部研究报告结论。

## 不做什么

- 不恢复 `panic = "abort"`（guard 依赖 unwind）；不新增 dev/bench profile。
- 不实现任何研究文档列出而 `DESIGN` 未声明的能力（type-ahead、vi/copy mode、
  OSC 7 cwd 消费、OSC 133 语义、synchronized output、命令面板、分屏、bell
  通知、Sixel/ZMODEM、远程会话、TalkBack 朗读等）——逐份判定见 `tasks.md`。
- 不引入任何外部测试语料（vttest/esctest/contour vtconformance/qwertty 差分
  语料/konsole autotests）：采纳门槛已在 `upstream-alignment` 与
  `terminal-state-regression` 逐项登记，本变更只补 OSC 核对表。
- 不修改 `scripts/` 与 `.github/`（AGENTS.md 禁改区）：`TESTING.md` 要求的
  aislop/jscpd 门禁与 rapidocr OCR 自动化两项登记为待维护者裁定，
  `tasks.md` 给出完整落地方案。

## Impact

- Affected specs: `upstream-alignment`（新增 OSC 核对）、`session-list`
  （新增切会话清空条款）。
- Affected code: `native/Cargo.toml`（构建配置）。
- Affected docs: `docs/persistence/ecosystem.md`。
- 行为变化仅一处：release 构建体积与链接时间（用户可感知的 `.so` 更小）。
