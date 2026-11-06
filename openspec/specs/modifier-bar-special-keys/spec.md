# modifier-bar-special-keys Specification

## Purpose

键栏（`extra-keys`）除普通按键外还支持 termux 的特殊键：`CTRL`、`ALT`、`DRAWER`、
`SCROLL`、`KEYBOARD`、`PASTE`。这些键不产生终端输入，由键栏自身处理。

本能力规定 `KEYBOARD` 键的行为与接线要求：该键 MUST 切换软键盘，且键栏 MUST 显式
声明它的回调——不允许出现「键帽画得出来、按下却什么都不做」的静默失败。

## Requirements

### Requirement: 键栏的 KEYBOARD 特殊键必须切换软键盘

键栏渲染出的 `KEYBOARD` 特殊键 MUST 切换软键盘，行为 MUST 与会话抽屉里的键盘按钮
一致（`soft-keyboard-toggle-behaviour` 为 `enable/disable` 时翻转软键盘可用性，其余
模式翻转当次可见性）。MUST NOT 落进空实现：键帽可见而按下无反应即违反本要求。

#### Scenario: 按下 KEYBOARD 键切换软键盘

- **WHEN** 用户在 `extra-keys` 中配置 `KEYBOARD` 并按下该键
- **THEN** 软键盘按当前切换语义改变可见性或可用性

#### Scenario: 键栏必须显式声明 KEYBOARD 键的回调

- **WHEN** 构造键栏或键栏动作集合
- **THEN** MUST 显式给出 `KEYBOARD` 键的回调；缺失时构建 MUST 失败
