# Tasks

## 1. 变更文档

- [ ] 1.1 写 proposal/specs delta 并 `openspec validate 2026-10-11-adopt-upstream-key-encoder --type change` 通过

## 2. native 侧键编码查询

- [ ] 2.1 `commands.rs` 新增 `Query::EncodeKeyEvent`（键码、修饰位、文本、unshifted 码点、动作），`internal.rs` 在 VT 线程持有 `key::Encoder` 与 `key::Event` 并于每次编码前调 `set_options_from_terminal`
- [ ] 2.2 `public_api.rs` 新增 `encode_key_event`，按 `EncodeMouseEvent` 同款「查询失败回空 Vec」语义；`ffi.rs` 导出 `encodeKeyEvent`
- [ ] 2.3 cargo 测试：DECCKM 开关前后的方向键、backarrow key mode 的退格字节、modifyOtherKeys 下 Ctrl+字母、kitty DISAMBIGUATE 下 Escape、单独修饰键空输出（期望值取本变更 proposal 的实测表）

## 3. Kotlin 侧收敛

- [ ] 3.1 `TerminalInputEncoder.kt` 删手写编码表（`CSI_KEYS`/`csi27Sequence`/`arrowSequence`/`controlByteForKeyCode`/`escapeSequenceForKeyCode`/`modifierParam`），新增无行为判断的 `KEYCODE_*` → 上游 `key::Key` 机械映射
- [ ] 3.2 `set_utf8` 输入按上游文档过滤：C0 控制字符与 U+007F 传 `None`，不传平台私有区码位；未覆盖键码映射为 `Unidentified`
- [ ] 3.3 修饰位统一经 `KeyModifiers.ghosttyMods`；核实 `NativeBridge.getMode` 的其他消费者，无消费者则删除该通道
- [ ] 3.4 `ModifierBar` 方向键改走统一编码路径，`testDebugUnitTest` 与 `detekt` 通过

## 4. 规范对齐

- [ ] 4.1 `upstream-alignment` 的 `key::Encoder` 条目从「待办」改为「采用」并写明阻塞点如何解除；`Terminal::paste` 条目保持待办并指向本批另一变更
- [ ] 4.2 `modifier-bar-arrow-encoding` 的 Kotlin 侧自实现部分改指 `key-input-encoding`；行为要求保持不变
- [ ] 4.3 归档后核对 `openspec/specs/` 确实含新条目数，不以 `openspec validate` 与 markdownlint 通过即认定落地
