# Proposal

## Why

`extra-keys` 支持 termux 的 `KEYBOARD` 特殊键：键栏会画出 ⌨ 键帽
（`TerminalKeyCatalog` 登记了 `Special.KEYBOARD`），`ModifierBar` 也为它准备了
`onKeyboardClick` 回调，但该回调从未被接线——`TerminalScreen` 构造键栏动作集合时没有
传入它，`ModifierBar` 用的是默认空实现。

后果是配置了 `KEYBOARD` 的用户按下该键毫无反应：键帽在、按下有动画，却什么都不发生。
termux 的同名键会切换软键盘，本仓 `TerminalScreen` 的注释也明确声明了
「`extra-keys` 里写 `KEYBOARD` 也会走到这里」这一意图，故这是实现与自身声明分家的缺陷。

## What Changes

- 把「切换软键盘」的动作接到键栏的 `KEYBOARD` 特殊键上：与抽屉里的键盘按钮共用同一动作。
- `KEYBOARD` 回调改为必填参数（`ModifierBar`、`ModifierBarActions`、
  `buildModifierBarActions` 三处）：编译期即保证任何键栏都必须决定该键的行为，
  不再可能静默落进空实现。
- 规约 `modifier-bar-special-keys` 新增一条要求：键栏的 `KEYBOARD` 特殊键必须切换软键盘。

## Capabilities

### Added Capabilities

- `modifier-bar-special-keys`：键栏特殊键（`KEYBOARD` 等）的行为与接线要求。

## Impact

- Android 侧：`ui/ModifierBar.kt`（回调必填）、`ui/TerminalScreen.kt`（接线与记忆化）。
- 行为变化仅限 `extra-keys` 含 `KEYBOARD` 的场景：此前无反应，此后切换软键盘。
- 不改依赖、不改协议，不改其它特殊键（CTRL/ALT/DRAWER/SCROLL/PASTE）的行为。
