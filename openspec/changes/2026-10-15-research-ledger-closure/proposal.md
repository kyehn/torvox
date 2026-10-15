# Proposal

## Why

`docs/research` 的 93 份研究报告（用户指定 commit 的编号文档集 31 份 + 当前工作区
73 份）在归档变更 `2026-10-15-research-residual-closure` 里做过一轮逐份处置，但本轮
把两份文档集**重新完整读一遍**后，发现该台账自身有缺口，且有 1 处真实代码缺口：

1. **四份文档零台账覆盖**：`sudo-tee-hollow`、`techsenger-jeditermfx`、`tn5250j-tn5250j`、
   `z-hunter-Scanline-term`（成文早于台账归档提交，却不在 52 份枚举名单内）；
   `mostty`、`marc2332-term`、`OpenTTY-J2ME` 三份同样在归档前成文而漏登，其中 mostty
   还被「自研 VT 全部禁止」的群体口径**错裁**（它与本仓同为 libghostty 唯一 VT 核）。
2. **多份文档只有群体一句话裁定**：`pg83-shitty`、`pop-os-cosmic-term`、`lihop-godot-xterm`、
   `marshallku-copad`、`marc2332-term`、`midasdf-zt`、`lmorg-ttyphoon`、`lxqt-qterminal`、
   `mirbehnam-RtlTerminal`、`not-elm-orzma`、`nowledge-co-con-terminal`、`paulrobello-par-term*`、
   `veops-go-ansiterm`、`sanohiro-bcon`、`tenox7-aclock`、`tuzig-terminal7`、
   `vercel-labs-wterm`、`wilsonglasser-oryxis`、`migueldeicaza-SwiftTerm`。
   它们的**具体可执行项**（第三方测试语料、MCP/agent 面、copy-on-selection、bell cooldown、
   会话恢复、CRT 特效、DBus 控制、i18n、RTL 整形、O(1) row-map、单帧 deadline/quantum 等）
   从未逐条登记；而 `upstream-alignment` 的「已复查结论登记在本规范内」明文要求
   **MUST NOT 把结论只留在归档变更里**——这些行现状正是只存在于归档 tasks.md。
3. **一处真实代码缺口**：`par-term` 研究报告的「GPU surface clamping」只落实了 no-op
   configure 早退，缩放后的 surface 尺寸未钳制到 `max_texture_dimension_2d`。
   `attach_surface` 与 `reconfigure_swapchain` 各自现算缩放尺寸（同一段计算写了两遍，
   钳制补一条路径就会静默漏另一条）；超过设备上限时 `surface.configure` 在 wgpu 30 无
   返回值，失败只经 `get_current_texture` 的 Lost 状态异步上报——表现为永久黑屏
   且没有直接错误，属「不隐藏错误」意义上的盲区。
4. **两处持久结论漂移**：RTL/BiDi 在 `PROHIBITED` 无条目，台账口径「BiDi 已禁止」
   依据不足，正确口径是「未声明不立项」；SwiftTerm 与已采纳的 ghostty fuzz 语料在
   `upstream-alignment` 台账无行，违反该 spec 自己的可脱离归档复核条款。

## What Changes

- **新增 `render-stability` Requirement「Surface 尺寸钳制到设备上限」**：交换链尺寸
  MUST 由唯一纯函数计算（缩放后钳制到设备 `max_texture_dimension_2d`），两条挂载路径
  MUST 共用；超限钳制 MUST 可被宿主单测直接判定（判定表，不经 GPU）。
- **新增 `upstream-alignment` Requirement「研究报告结论台账补齐」**：把上述逐份结论
  写进规范正文（语料判定行 + 能力裁定行），使每条结论可脱离归档独立复核。
- **代码**：`native/src/render/context.rs` 新增 `surface_extent` 纯函数并替换
  `attach_surface` / `reconfigure_swapchain` 两处重复现算；`render/tests.rs` 增判定表用例。
- **`docs/persistence/ecosystem.md`**：参照面表补 mostty / hollow / jeditermfx /
  tn5250j / Scanline / pg83 / cosmic-term 等漏登行；「减码空间」一节补 jscpd 实测口径。

## 不做什么

- **不改 `scripts/`、`.github/`、`android/app/build.gradle.kts`**（AGENTS.md 禁改区，
  维护者已裁定「都不改，只登记」）：aislop/jscpd 门禁、rapidocr OCR 自动化、CI 产物
  哈希比对、16KB 页对齐四项维持登记在案，落地方案写入 tasks.md，不擅自实施。
- 不为单帧处理引入 deadline/quantum 预算（mostty §5）：本仓读线程已有界通道
  （`OUTPUT_CHANNEL_BOUND` × 8KB）封顶，帧预算属过度设计嫌疑，按台账口径登记不立项。
- 不引入任何外部测试语料：本轮新增的全部语料行判定结果都是「不采纳」，理由逐行写明。
- 不实现 `DESIGN` 未声明能力（MCP/agent、插件、mux、分屏、会话恢复、CRT 特效、
  输出分块、DBus 控制、i18n、RTL/BiDi 整形、copy-on-selection、Tek4010、5250 等）。

## Impact

- Affected specs: `render-stability`（新增钳制条款）、`upstream-alignment`（新增台账
  Requirement，把原只存在于归档的逐份结论写入规范）。
- Affected code: `native/src/render/context.rs`、`native/src/render/tests.rs`。
- Affected docs: `docs/persistence/ecosystem.md`。
- 行为变化：超过设备纹理上限的窗口尺寸不再注定永久黑屏（被钳制后正常出帧）；
  正常尺寸下行为逐字节不变（判定表钉住缩放与钳制两个不变量）。
