# Proposal

## Why

应用快捷方式「新建会话（Failsafe）」在应用**已启动**时点了毫无反应。

`MainActivity.handleLaunchIntent` 收到带 `com.termux.app.failsafe_session=true` 的 Intent
后只做一件事：`runtime.requestFailsafeSession()`——把一个布尔标记置真。而标记的消费者
只有两处建会话入口：`TerminalRuntime.start()` 与 `TerminalRuntime.createSession()`。

`start()` 在「已有会话」时直接早退：

```kotlin
synchronized(sessionLock) {
    if (sessions.isNotEmpty() || starting) return
    starting = true
}
```

应用已启动时走不到 `createSession()`——建会话要等用户点「+」或按快捷键。于是：

- 快捷方式点了没有任何会话被建出来（`DESIGN.md`「应用已启动和未启动时都能使用这种
  方式新建会话」不成立）；
- 标记滞留在运行期。用户下一次**任意**新建会话（点「+」）时，它会以
  `/system/bin/sh` 启动——一个用户并未要求的普通会话被换成了应急会话。

`app-shortcuts` 规格早已写明「应用已运行时触发快捷方式 → 应用创建一个使用
`/system/bin/sh` 的会话，已有会话不受影响」，实现与规格分叉。

## What Changes

- Failsafe Intent 的处置改为**按运行期状态分派**（`decideFailsafeIntent`）：
  - 应用未启动（无会话）：只置标记，首个会话由 `runtime.start()` 以 `/system/bin/sh`
    建出——行为不变；
  - 应用已启动且 Intent 为新投递：置标记**并**显式建一个新会话；
  - Activity 重建时重复投递的同一个启动 Intent：完全忽略（否则每次配置变更都多出
    一个应急会话，标记也会滞留）。
- 新会话的建出经 `TerminalViewModel`：Surface 未交付时（快捷方式新建的 Activity 实例
  尚未布局完、应用从后台被拉起）先挂起请求，待 Surface 交付后补建——请求既不落空，
  也不提前生效。
- 规约 `app-shortcuts` 增补一条要求：Failsafe Intent MUST 每次都导致一个 Failsafe
  会话被建出，且不得在 Activity 重建时重复建。

## Capabilities

### Modified Capabilities

- `app-shortcuts`：Failsafe Intent 的处置与「已启动时新建会话」的语义。

## Impact

- Kotlin：`TerminalViewModel.kt`（处置决策、挂起与补建）、`MainActivity.kt`
  （把 Intent 连同「是否新投递」交给 ViewModel）。
- 行为变化仅限 Failsafe 快捷方式与第三方 Failsafe Intent：应用已启动时它们现在会
  建出一个新的应急会话；普通的新建会话不再被滞留的标记劫持。
- 不改依赖、不改协议、不改设置项、不改原生侧。
