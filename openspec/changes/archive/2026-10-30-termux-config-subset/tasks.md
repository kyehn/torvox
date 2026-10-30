## 1. 规范

- [x] 更新 `docs/specification/PROHIBITED.md`
- [x] 更新 `docs/specification/DESIGN.md`
- [x] 新增 `docs/specification/config.md`
- [x] 更新 `openspec/specs/shell-entry/spec.md`
- [x] 新增 `openspec/specs/termux-config/spec.md`

## 2. 实现

- [x] Kotlin `TermuxProperties` 解析器
- [x] `TerminalConfig` 新字段
- [x] `NativeBridge`/`Bridge` 入参
- [x] Rust `Session::spawn`/`GhosttyTerminal` 默认光标样式

## 3. 验证

- [x] `TermuxPropertiesTest`
- [x] `cargo test`
- [x] `:app:testDebugUnitTest`
