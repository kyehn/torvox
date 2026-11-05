# Proposal

## Why

`docs/specification/DESIGN.md` 的软件节明确要求「处理 OSC 9 等通知，输出会话序号和通知。
应用位于前台时使用 Toast Notification。位于后台时使用“系统通知”，点击后切换到目标会话」，
而本仓既没有 OSC 9/777 的消费者（上游 `on_desktop_notification` 回调未注册），台账
（`openspec/specs/upstream-alignment/spec.md` 的「OSC 覆盖逐项核对」）还把 OSC 9 记为
「不接：零消费者」并写明理由是「DESIGN 未声明通知语义」——该理由与 DESIGN 的现状直接矛盾。
规格与实现两头落空：终端程序发出的通知（长任务完成、提醒类输出）在本仓完全静默，用户
只能回到终端窗口才看得见。

## What Changes

- 注册上游 `on_desktop_notification` 回调，把 OSC 9 与 OSC 777 的标题、正文按会话归属
  投递给宿主；负载是程序给出的原始字节，按替换字符口径解码，不得因非 UTF-8 崩溃。
- 通知投递与渲染生命周期解耦：通知走独立队列，前台由渲染线程取走、后台由会话推进泵取走。
  后台期间产生的通知不得滞留到用户回到应用才显示——那正是「后台用系统通知」的意义所在。
- 前台用 Toast、后台用系统通知；两者的内容都带会话序号；点击系统通知切换到目标会话
  （表面尚未就绪时须等待就绪，不得静默丢弃切换请求）。
- 新增能力 `desktop-notification` 承载上述行为；修正 `upstream-alignment` 中与 DESIGN
  冲突的判定：OSC 逐项核对表、上游回调取用登记、OSC 回调通道描述与 JSON 边界登记。

## Capabilities

### New Capabilities

- `desktop-notification`：终端程序发出的桌面通知（OSC 9/777）的采集、分派与点击切换。

### Modified Capabilities

- `upstream-alignment`：「OSC 覆盖逐项核对」表中 OSC 9 与 777 的行为归属、「上游 API 取用
  判定登记」中 `on_desktop_notification` 的结论、「不采用的上游回调不得留有通道描述」中
  OSC 回调通道的承载内容，以及「保留 JSON 的边界登记」中新增的通知队列一行。

## Impact

- 规约：新增 `openspec/specs/desktop-notification/spec.md`；`upstream-alignment` 四条要求的
  表述随本变更更新（归档时同步）。
- 原生侧：`native/src/terminal/ghostty_terminal/` 注册上游回调并把通知推入独立队列；
  `native/src/event.rs` 增通知事件；`native/src/android/ffi/` 增通知队列的轮询导出。
- Android 侧：`bridge/` 增通知事件解码与轮询；`runtime/` 增前台/后台分派与系统通知发布；
  `MainActivity` 处理通知点击带来的会话切换请求。
- 不改依赖：`POST_NOTIFICATIONS` 权限已在清单中声明且启动时申请，系统通知沿用既有权限。
