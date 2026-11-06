# Proposal

## Why

系统通知的 id 在整个应用内共享一张表，同 id 即同一条通知。前台服务的常驻通知占用
id 1，而桌面通知直接拿会话 id 当通知 id——首个会话的 id 恰好也是 1：

- 桌面通知投递时顶掉常驻通知，用户在通知栏里看不到「退出」与「唤醒锁」入口，常驻
  会话失去唯一的常驻提示；
- 常驻通知因会话数变化而刷新时顶掉桌面通知，用户刚收到的提醒凭空消失。

后者在真实使用里最常见：应用退到后台跑一个长任务，完成时收到 OSC 9 提醒，随后再开
一个终端会话——会话数变化触发常驻通知刷新，那条提醒就没了。

## What Changes

- 桌面通知的系统通知 id 改为落在独立号段：由会话 id 派生，与前台服务的常驻通知 id
  分区。同一会话仍得到同一 id，同一会话的后续通知仍顶替先前那条。
- 桌面通知的 PendingIntent 请求码改用同一 id，与前台服务的请求码（0..3）也分开。
- 规约 `desktop-notification` 增补一条要求：系统通知的标识必须与常驻通知分开。

## Capabilities

### Modified Capabilities

- `desktop-notification`：桌面通知的系统通知 id 与常驻通知分区。

## Impact

- Kotlin：`runtime/DesktopNotificationPlayer.kt`（新增 `desktopNotificationId`，通知 id 与
  PendingIntent 请求码改用它）。
- 行为变化仅限后台桌面通知：不再与常驻通知互相顶替。
- 不改依赖、不改协议、不改设置项、不改原生侧。
