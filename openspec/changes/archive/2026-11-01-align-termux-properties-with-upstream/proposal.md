# Proposal

## Why

`hide-soft-keyboard-on-startup` 的默认值仍是「该键还是空转键时」定下的 `true`：
上游 termux 的默认是 `false`（启动即弹键盘），而 `docs/specification/DESIGN.md` 明确要求
「启动后自动弹出输入法」。三者对不上的直接后果是：默认配置下冷启动永远不弹输入法，
用户必须先轻点终端才能打字。

同时，`docs/STATUS.md` 要求「`termux.properties` 的所有相关代码必须根据
`docs/specification` 与 termux-app 重新检查和修改，并且每个键都需要充分测试」。
本轮逐键比对了上游 `TermuxPropertyConstants` / `TermuxSharedProperties` 与本仓实现，
除上述默认值外其余 19 个键的默认值、取值集合与生效口径均已对齐，缺的是**逐键的测试证据**。

## What Changes

- `hide-soft-keyboard-on-startup` 的默认值改为 `false`，与上游 termux 一致，并让默认
  行为满足 `DESIGN.md` 的「启动后自动弹出输入法」。
- `hide-soft-keyboard-on-startup = true` 的语义随之明确为「启动时主动收起输入法」，
  而不再是「不主动请求」。
- 为 `termux.properties` 的 20 个键补齐逐键测试：默认值、有效取值、非法取值回退默认
  并留痕，三类断言各覆盖一遍。
- 同步 `openspec/specs/termux-configuration` 中「启动时的软键盘」条款。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `termux-configuration`: 「启动时的软键盘」条款的默认值与默认场景发生变化。

## Impact

- `android/app/src/main/java/terminal/emulator/settings/TermuxProperties.kt`：
  `hideSoftKeyboardOnStartup` 的默认值与字段注释。
- `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesParserTest.kt`：
  默认值断言与逐键测试。
- `openspec/specs/termux-configuration/spec.md`：同步条款。
- 无新增依赖；原生侧与 JNI 接口不受影响。
