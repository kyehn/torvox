# Tasks

## 1. 先写红测（Shift 与 Tab/Enter 分流期望）

- [x] 1.1 在 `TerminalInputEncoderTest.kt` 增 Shift 用例：Shift+Tab → `CSI Z`、
      Shift+↑/↓/←/→ → `CSI 1;2A/B/C/D`、Shift+Home/End → `CSI 1;2H/F`、
      Shift+PageUp/PageDown → `CSI 5;2~`/`CSI 6;2~`、Shift+Insert → `CSI 2;2~`、
      Shift+ForwardDel → `CSI 3;2~`、Shift+F1..F12 → `CSI 1;2P`…/`CSI 15;2~`…、
      Shift+Enter → `CSI 27;2;13~`、Shift+Ctrl+↑ → `CSI 1;6A`、Shift+Alt+↑ → `CSI 1;4A`；
      每条期望值注释标明来源（xterm ctlseqs.txt / ghostty function_keys.zig /
      zed mappings/keys.rs / wezterm termwiz input.rs）。
      验证：`./gradlew :app:testDebugUnitTest --tests '*TerminalInputEncoderTest*'` 全红（新用例失败）。
- [x] 1.2 订正三条既有断言并在注释写明旧值为何错：`ctrl tab produces csi 9 mod`
      → `CSI 27;5;9~`；`alt tab produces csi 9 mod` → `ESC HT`；
      `encodeKeyEvent enter produces carriage return` 增 Shift 对照场景。
      验证：上述三条同样变红。

## 2. 编码器实现

- [x] 2.1 `TerminalInputEncoder.encodeKeyEvent` 增 `shiftActive: Boolean = false`，
      `modifierParam` 纳入 Shift 位并抽出命名常量 `UNMODIFIED_MODIFIER_PARAM = 1`；
      闸门由 `ctrl || alt` 改为三者任一。验证：`csiSequenceWithModifier` 对 Shift 生效，
      1.1 的 Shift 用例全部转绿。
- [x] 2.2 用 `CSI_KEYS: Map<Int, CsiKey>`（`CsiKey(plain, intro, final)`）替换
      `escapeSequenceForKeyCode` 与 `csiSequenceWithModifier` 两张平行表，
      带修饰序列由 `intro`/`final` 现场拼装；`KEYCODE_DEL` 的 plain 维持 `0x7F`、
      修饰态维持 `CSI 3;mod~`（由 `plain` 字段承载，不再靠第二条路径）。
      验证：全量 `TerminalInputEncoderTest` 无回归（含 F1–F12、方向、Home/End、
      PgUp/PgDn、Insert/Delete、App 光标模式用例）。
- [x] 2.3 `Bridge.processKeyEvent` 从 `modifierBits` 取 SHIFT 位并下传
      `shiftActive`。验证：`Bridge` 相关单测通过，掩码→编码器链路有集中断言。
- [x] 2.4 `ModifierBar.kt` 调用点补 `shiftActive = false`（键栏无 Shift 键）。
      验证：`ModifierBarRobolectricTest` 通过。

## 3. 收尾验证

- [x] 3.1 跑 `cargo test --lib` 确认 Rust 侧无回归（本变更不改 Rust）。
      验证：533 通过 / 0 失败（nix develop + lavapipe）。
- [x] 3.2 `./gradlew :app:testDebugUnitTest` 全量通过。
      验证：无失败、无 skipped。
- [x] 3.3 更新 `openspec/specs/modifier-bar-sticky-encoding/spec.md` 并
      `openspec archive fix-shift-modifier-encoding`。
      验证：`openspec list --specs` 显示新要求已合入，`openspec validate --strict` 通过。
- [x] 3.4 复核 `TerminalInputEncoder.kt` 无内联字面量函数名/第三方实现细节泄漏进注释，
      来源注释只指上游路径。验证：`grep -n "jscpd\|npx aislop"` 之外的人工复查 +
      `cargo clippy -- -D warnings`（Rust 侧未动，确认无新增告警）。
