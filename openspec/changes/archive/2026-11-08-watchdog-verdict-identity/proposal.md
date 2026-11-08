# Proposal

## Why

会话存活看门狗落地后，逐行复核发现它自身带进三处真实缺陷，三处都在「判定成立之后」的
处置路径上：

- 回调在闸门锁内调用，而回调要取会话锁；看门狗的停止又在持有会话锁的路径上调用，它要
  取闸门锁。两把锁的获取顺序相反，构成 AB-BA 死锁：看门狗线程持闸门锁等会话锁，会话
  线程持会话锁等闸门锁，整个会话子系统冻住——正是本能力要治的「终端会话概率卡死」。
- 闸门只比对「本代是否仍然有效」，而渲染线程重启会换上新的看门狗实例。旧实例的迟到
  判定仍会通过闸门，把新渲染线程标记为死亡，或关闭一个已经恢复的会话。
- 挂起关闭路径直接销毁原生会话，不先停渲染线程。VT 层与输出投递受阻层的挂起恰恰意味
  着渲染线程仍卡在原生代码里，它随时可能回到那段代码继续触碰 bridge 与 surface——
  此时销毁即 use-after-free。既有守卫只看「上一次 join 是否超时」的标志，而该路径从
  不调用停止逻辑，守卫形同虚设。

三处都不是「日志噪声」，是会导致用户可见故障（会话整体冻死、恢复后的会话被误关、
原生资源被提前释放）的正确性缺陷。

## What Changes

- 挂起回调改为携带触发它的看门狗实例；处置方在会话锁内复核「本实例仍是该会话当前
  的看门狗」，复核未通过即忽略该判定。复核与状态变更在同一次会话锁获取内完成。
- 挂起回调移到闸门锁之外调用，锁序固定为「会话锁 → 闸门锁」。
- 挂起关闭路径先停止该会话的渲染线程，渲染线程未退出时由既有守卫跳过销毁，原生会话
  待进程消亡时回收。
- 补三组回归钉：闸门锁下不取会话锁（无 AB-BA 死锁）、回调携带触发实例、陈旧看门狗
  的判定被忽略。
- 不改变判定阈值、分级恢复动作、关闭节流窗口与用户可见的会话语义；不新增权限、通知
  或后台服务。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `session-liveness`：新增「判定按看门狗实例复核」「回调不得持有闸门锁」「挂起关闭路径
  先停渲染线程再销毁原生会话」三条要求。

## Impact

- `android/app/src/main/java/terminal/emulator/monitor/RenderWatchDog.kt`：回调签名与调用
  时机。
- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`：看门狗摘除与
  身份复核、挂起关闭路径的停止时序。
- `android/app/src/test/java/terminal/emulator/monitor/RenderWatchDogTest.kt`：回调签名与
  两条回归钉。
- `android/app/src/test/java/terminal/emulator/runtime/SessionHangVerdictTest.kt`：陈旧
  判定被忽略的回归钉。
- `openspec/specs/session-liveness/spec.md`：归档时同步三条要求。
