# Proposal

## Why

`logcat-hygiene` 要求逐帧诊断以窗口汇总输出，且该行 MUST 带上窗口内的事件计数。
宿主渲染循环里的失败诊断仍停在旧口径：瞬时渲染错误、渲染异常、错误后恢复与输出
泵失败。

根因是节流挂在「连续错误数」上，而成功一帧就把连续数清零。「失败—成功—失败」的
交替形态下每次都命中首条分支，于是每次都记一行：瞬时渲染错误每次一行、渲染异常
每次一行（还带完整栈）、恢复消息每次一行。渲染异常与输出泵失败更是逐帧可发生。

另外，输出泵失败此前完全不限频，桥接层持续抛异常时每帧记一条 error 加栈。

## What Changes

- 宿主渲染循环的四类失败诊断改用带计数的窗口闸门，行内给出窗口内累计的次数。
- 连续错误数只保留退避档位与致命上限两个用途，不再参与输出决策。
- 渲染异常与输出泵失败在窗口内只有一次时给完整栈，成堆重复时只给异常类型与次数。
- 移除标题查询外层那段永远不可达的 catch：查询失败由原生查询端口归为缺省值，
  取消信号原样上抛。
- `logcat-hygiene` 补四条场景（宿主渲染失败、错误后恢复、输出泵失败、标题查询）。

## Capabilities

### New Capabilities

无（不新增能力）。

### Modified Capabilities

- `logcat-hygiene`：给「应用自身的正常路径不得产生警告与错误」与「高频诊断日志必须限频」补场景。

## Impact

- `android/app/src/main/java/terminal/emulator/runtime/RenderErrorLedger.kt`：新增失败账本。
- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`：渲染循环改用账本。
- `android/app/src/test/java/terminal/emulator/runtime/RenderErrorLedgerTest.kt`：新增账本用例。
- `android/app/src/test/java/terminal/emulator/runtime/TransientRenderErrorSeverityTest.kt`：分级口径改为含本次。
- `openspec/specs/logcat-hygiene/spec.md`：归档时同步场景。
