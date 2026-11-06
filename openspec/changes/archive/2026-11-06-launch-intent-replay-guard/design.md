# Design

## 问题定位

`MainActivity` 的 `launchMode` 是默认的 `standard`，启动 Intent 会在三种情形下到达
`handleLaunchIntent`：

| 情形 | 入口 | 「是否新投递」 |
| --- | --- | --- |
| 冷启动（桌面图标、通知、快捷方式） | `onCreate` | 是（`savedInstanceState == null`） |
| 应用已在运行（通知点击、快捷方式） | `onNewIntent` | 是（恒真） |
| 配置变更重建（密度、语言） | `onCreate` | 否（`savedInstanceState` 非空） |

第三种情形带的是**同一条** Intent：`getIntent()` 仍是当初那一条。Failsafe 分支早已
按 `freshIntent` 门控（`decideFailsafeIntent`），而通知切换与打开设置两个分支
没有门控，于是重建时把一次性请求重放了一遍。

设置覆盖层则是另一半：`showSettings` 由 `remember` 持有，组合随 Activity 销毁即
丢失；重建后的初值取自 `openSettingsRequests > 0`，而那个计数来自启动 Intent。于是
「是否打开」取决于 Intent 而非用户当下的操作：手动打开的设置页在重建后消失，带
extra 启动的设置页在用户关掉后又被重建弹回。

## 方案选择

**门控放在 `MainActivity`，语义判定放在 `TerminalViewModel`。**

- 「是否新投递」只有 Activity 知道（`savedInstanceState` 与 `onNewIntent`），
  故门控必须在 Activity 侧；
- 但「陈旧请求要不要留下待补切的请求」是运行期状态，属 ViewModel：它在
  `requestSessionSwitch` 里统一处理，陈旧请求直接丢弃——不切换，也不写
  `pendingSessionSwitchId`，否则重建后 Surface 一交付就会把用户拽回旧会话。

**覆盖层状态改用 `rememberSaveable` 而非把计数搬进 ViewModel。**

- 计数搬进 ViewModel 只能让计数跨重建存活，覆盖层的实际开合仍会丢：用户关掉设置页
  后计数还是 1，重建后照样被打开；
- `rememberSaveable` 把**实际状态**存进实例状态包：重建前开着就还开着，关着就还关着，
  与它怎么被打开无关。启动 Intent 只在本次新投递时触发一次打开，语义单一。

## 与既有处置的一致性

Failsafe Intent 的纪律是「重建时的重复投递不重放状态改写」。本次把通知切换与打开
设置纳入同一条纪律：同一条 Intent 的三项状态改写，要么都按新投递判定，要么都不判。
否则用户会看到「快捷方式没重复建会话，但通知却把会话拽回去了」这种半重放。

## 验证方式

- ViewModel 层单测（mock 运行期）：新投递 + Surface 就绪即切换；新投递 + Surface
  未就绪则挂起、交付后补切；重建时的重复投递既不切换也不挂起。
- 覆盖层状态由 `rememberSaveable` 承担，属 Compose 状态，无 JVM 单测接缝；其语义
  由 `app-lifecycle` 的规约条款约束。
