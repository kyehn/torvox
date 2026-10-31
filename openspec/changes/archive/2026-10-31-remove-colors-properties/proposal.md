# 移除 colors.properties 解析

## Why

`docs/specification/config.md` 与 `PROHIBITED.md` 明确禁止解析 `colors.properties`，
代码却实现了完整的读取、19 槽覆盖与原生覆盖通道。用户要求删除该能力，使实现回到
「只读 `termux.properties` 三个允许键」的规范口径。

## What Changes

- 删除 Kotlin 侧 `ColorsProperties`、`ColorsPropertiesParser` 与仓储中的
  `colors.properties` 读取。
- 删除渲染契约的 `colors` 字段、`Bridge.setPaletteOverride` 与 JNI 入口。
- 删除原生侧覆盖通道：`setPaletteOverride`、`Command::SetColors`、`ACTIVE_THEME`、
  `Palette::with_overrides` 及相关测试。
- 终端配色只来自内置主题；`colors.properties` 存在也不读取、不生效。

## Impact

- 受影响能力：`termux-configuration`（删除 colors.properties 覆盖要求，读取要求收窄为
  单文件）。
- 用户可见：此前通过 `colors.properties` 做的颜色覆盖不再生效；删除文件或改主题的
  行为不变。
