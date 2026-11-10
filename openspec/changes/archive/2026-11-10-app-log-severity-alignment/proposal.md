# Proposal

## Why

应用自身的日志仍把三类正常路径记成 warn 或 error，真正的告警因此被淹没：

- 关闭路径中断渲染线程时，`runBlocking` 以取消异常收尾（cause 为 `InterruptedException`），
  此前落进通用异常分支，被记成 error 并附上完整栈——那是拆卸流程的正常一步。
- 平台收走原生窗口后渲染帧取不到纹理属正常退化，原生侧却把窗口判死记成 error、把重挂载
  记成 warn；宿主侧请求换新窗口一律记 warn，首次也不例外。
- 视图未附着时跳过选择菜单记 warn。

另有一处应用自身触发的框架告警：平台对同一 UID 同时只允许一个提示窗口，应用连续弹出
两条提示时第二条抛 `BadTokenException`，平台同时打出 W 级告警。

## What Changes

- 规约 `logcat-hygiene` 新增三条要求：渲染线程的中断是关闭信号、宿主未就绪的路径按正常
  退化记、同一时刻只允许一条提示窗口。
- 应用侧实现随之调整：中断按 debug 收尾；原生窗口判死记 debug、重挂载记 info；宿主请求
  换新窗口首次记 info、连续记 warn；视图未附着跳过菜单记 debug；提示改由单例投递器发出，
  新的一条先取消仍在显示的那条。

## Capabilities

### Modified Capabilities

- `logcat-hygiene`：应用自身正常路径的日志分级，以及提示窗口的独占约束。

## Impact

- `openspec/specs/logcat-hygiene/spec.md`
- `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`
- `android/app/src/main/java/terminal/emulator/runtime/ToastNotifier.kt`
- `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`
- `native/src/render/context/renderer.rs`
