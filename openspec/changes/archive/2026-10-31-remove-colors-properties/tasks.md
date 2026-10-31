# Tasks

## 1. Kotlin

- [x] 1.1 删除 `ColorsProperties`、`ColorsPropertiesParser`、`parseHexColor`
- [x] 1.2 `TermuxPropertiesRepository` 只读 `termux.properties`
- [x] 1.3 删除 `RenderContract.colors` 与 `applyRenderContract` 的覆盖下发
- [x] 1.4 删除 `Bridge` / `NativeBridge` 的 `setPaletteOverride`

## 2. Native

- [x] 2.1 删除 JNI `setPaletteOverride` 与 `ACTIVE_THEME`
- [x] 2.2 删除 `Command::SetColors`、`GhosttyTerminal::set_colors`
- [x] 2.3 删除 `Palette::with_overrides`、`OVERRIDE_SLOTS` 与相关测试

## 3. 测试与文档

- [x] 3.1 删除颜色覆盖相关测试，保留光标样式校验
- [x] 3.2 `docs/specification/config.md` 对照现状更新
- [x] 3.3 同步 `openspec/specs/termux-configuration` 与差异审计
