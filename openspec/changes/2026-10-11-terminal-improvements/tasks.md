# Tasks

> **本清单不可执行。** 逐项核验于 2026-10-12：30 个未勾选项全部违禁或已被现有规范取代，
> 无一可按原样开工。原始条目保留以便追溯，每项附核验依据；不得据本清单动工。

## 1. DESIGN.md Updates

整节不可执行：`docs/specification/` 属禁止修改目录（`AGENTS.md`），`Update DESIGN.md` 无路径。

- [ ] 1.1 Update theme system: Replace hardcoded theme list with configurable themes supporting preview + custom builder
  - **违禁**：`PROHIBITED.md:50` 明禁「自定义终端主题」；`theme-configuration` 规约已写明
    "Custom themes are a prohibited implementation"。预览能力已由该规约「主题效果支持预览」覆盖，
    custom builder 无任何可保留部分。
- [ ] 1.2 Add font size pinch-zoom: 8-40 sp range, persist across restarts, consider actual cell rendering resolution
  - **已完成且口径不同**：`DESIGN.md:66` 已声明捏合缩放，`font-selection`「字号上下限按完整
    sp→px 系数换算」给出上界口径（256px 上限与可用列数取紧者）。硬写的 8-40 sp 与该口径冲突：
    上界须按设备密度与屏幕宽度换算，低密度设备为 30sp、高密度为 74sp–96sp，固定区间会同时
    违反两条换算约束。
- [ ] 1.3 Enhance bootstrap onboarding: Add feature intro screens + distribution selection (Alpine/Debian like ghostty-android-terminal)
  - **未声明**：`docs/specification/` 全文无引导屏与发行版选择条款，`STYLE.md:54`「不实现未在
    docs/specification/ 声明的功能」。另与 `PROHIBITED.md:20`「内嵌 bootstrap，预装发行版」张力。
- [ ] 1.4 Add config export/import: JSON format with theme, font, font_size, scrollback, opacity, autohide_keys, wakelock
  - **违禁**：`upstream-alignment`「跨 JNI 扁平数据 MUST 用直接量而非 JSON」与「保留 JSON 的边界登记」
    已把 JSON 限定在 `pollEvent` 一处；27 号收敛文档记录用户明确要求不生成 JSON。
    另 `opacity`（背景透明）与 `autohide_keys` 属 `PROHIBITED.md:17` 禁止项。
- [ ] 1.5 Add backup/restore system: Distro rootfs backup to /sdcard/RedTerm + settings restore
  - **未声明**：`docs/specification/` 无备份还原条款。写 `/sdcard/RedTerm` 另与
    `DESIGN.md:186`「软件不得修改 home、`usr` 仅 Bootstrap 阶段可作重命名目标」冲突。
- [ ] 1.6 Expand font list: Add JetBrains Mono, Fira Code, Source Code Pro, Ubuntu Mono, Cascadia Code as options
  - **违禁**：`DESIGN.md:77`「不硬编码」字体名；`font-selection`「字体列表取自渲染侧字体库」
    规定列表须为 `fonts.xml` 声明族加用户投放目录，硬编码五款字体需 vendor 字体文件
    （`BUILD.md:9` 亦禁 vendor）。
- [ ] 1.7 Implement sticky modifier keys: CTRL/ALT toggle (tap to activate, turns blue, subsequent taps = Ctrl+key)
  - **已完成**：`modifier-bar-sticky-encoding` 规约已覆盖粘滞编码与 Once 态消费，实现与单测已落地。
- [ ] 1.8 Add word-based input: Optional feature for rich keyboard features (word suggestions, completion)
  - **未声明**：`DESIGN.md:137`「支持全功能输入法，不限制输入法特性」是输入法侧放行，
    不含自建词库与候选 UI；自建候选属重复实现输入法职责。

## 2. New OpenSpec Spec Entries

整节不可执行：五项能力全部对应上文违禁或未声明项，开条目等于把违禁项写进 `openspec/specs/`。

- [ ] 2.1 Add `theme-configuration` spec: Theme system with pre-built + custom themes, preview, RGB builder
  - **部分已存在且越界**：该 capability 已在 `openspec/specs/theme-configuration/`，
    且规定固定 10 标识、无 custom builder。重复开条目会与现有规约冲突。
- [ ] 2.2 Add `font-handling` spec: Font picker, pinch-zoom, size range, persist across restarts
  - **已存在**：能力名实为 `font-selection`，见 1.2。
- [ ] 2.3 Add `bootstrap-onboarding` spec: First-run feature intro + distribution setup
  - **违禁**：见 1.3。
- [ ] 2.4 Add `config-export-import` spec: JSON format for settings save/restore
  - **违禁**：见 1.4。
- [ ] 2.5 Add `backup-restore` spec: Distro rootfs + settings backup/restore system
  - **违禁**：见 1.5。

## 3. Implementation

整节不可执行：五项实现任务全部依附于上文已否决的能力面，无一可独立成立。

- [ ] 3.1 Update `native/src/terminal/ghostty_terminal/` - theme-related commands if needed
  - **无待办**：主题取用已定（`theme-configuration` 走上游 ghostty 调色板与构建期 TOML），
    该侧无缺口。
- [ ] 3.2 Update `android/bridge/NativeBridge.kt` - new JNI exports for theme/font/backup
  - **违禁**：backup 导出不成立（见 1.5）；theme/font 导出已存在，无缺口。
- [ ] 3.3 Update `android/app/src/main/java/terminal/emulator/settings/` - UI components for new settings
  - **违禁**：新增项分别属未声明（引导、备份、词语输入）与违禁（自定义主题、JSON 存取）。
- [ ] 3.4 Update `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt` - handle new features
  - **无待办**：同 3.2。
- [ ] 3.5 Update Compose UI components for new settings screens
  - **违禁**：同 3.3。

## 4. Validation

- [x] 4.1 `openspec validate 2026-10-11-terminal-improvements --type change` passes
- [ ] 4.2 All new specs follow project conventions
  - **不可执行**：§2 五项能力全部否决，无新规约可写。校验通过只证明结构合法，
    不证明内容合规——两者在本清单里恰相反。
- [ ] 4.3 No breaking changes to existing functionality
  - **不可执行**：无实现可验。1.1、1.6、1.4 若实施即破坏现有 `theme-configuration`
    与 `font-selection` 的单源约束。
- [ ] 4.4 Backward-compatible config import from existing settings
  - **违禁**：随 1.4 一并不成立。

## 5. Testing

整节不可执行：七项测试全部在测不存在的能力，无一可写。

- [ ] 5.1 Test theme selection and preview
  - **已覆盖**：`theme-configuration` 现有规约已有对应断言，无需另立。
- [ ] 5.2 Test font size pinch-zoom across range
  - **已覆盖**：`font-selection`「字号上下限按完整 sp→px 系数换算」已给出
    密度放大收紧、低密度列数换算、原生上界不小于可选上界等场景。
- [ ] 5.3 Test bootstrap onboarding flow
  - **违禁**：见 1.3。
- [ ] 5.4 Test config export/import roundtrip
  - **违禁**：见 1.4。
- [ ] 5.5 Test backup/restore for distributions
  - **违禁**：见 1.5。
- [ ] 5.6 Test sticky modifier key behavior
  - **已覆盖**：随 1.7 落地，测试已存在于 `modifier-bar-sticky-encoding`。
- [ ] 5.7 Test word-based input (optional)
  - **违禁**：见 1.8。
