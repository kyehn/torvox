# Design

## 决策一：审计留档只留聚合结论，不留原始日志

原始日志只在分析期间使用，不入库、不保留副本。仓内只留：逐标签计数、归因类别、
处置结论与可复核的上游事实。理由与首轮一致：原始行含时间戳、PID、设备标识与
下载链接，无法脱敏到可公开的程度，而结论本身可被任何人用同样的两份日志复核。

## 决策二：上游协议条目按「两层事实」登记，不写成应用缺陷

`upstream-alignment` 已有口径：判断 VT 特性是否可用必须区分终端上游 master 与
绑定层所钉提交。本轮两条缺口在两层都缺，故记为上游缺口：

| 缺口 | 终端上游 master | 绑定层所钉提交 | 应用侧义务 |
| --- | --- | --- | --- |
| DECSACE（`CSI Ps ; Ps ; Ps SP x` 选区擦除） | `src/terminal/stream.zig` 的 CSI 分派无 `'x'` 分支，无 DECSACE 字样 | 同左 | 不自研 VT 解析绕过；协议未定义应答，故不伪造应答 |
| kitty 共享内存介质（`t=s`） | `src/terminal/kitty/graphics_image.zig` 的 `readSharedMemory` 对 Android 直接返回 `UnsupportedMedium`（注释：Android 不支持 POSIX 共享内存），且 `shm_open` 需要 libc | 同左 | 不自建共享内存通道绕过；协议定义了应答，故按 kitty 规范回明确失败 |

取证方式：上游 master 取当日 `main` 分支源码，绑定层取 `libghostty-vt-sys` 构建时
vendored 的终端源码。两处事实都可复现。

## 决策三：kitty 介质错误的两种来源必须分开说

`EINVAL: unsupported medium` 有两个来源，日志文案相同：

1. **介质未启用**——应用侧可修。终端初始化时默认只开 direct 介质，客户端用
   `t=f`/`t=s` 时上游按 limits 拒绝。此前已修：初始化时启用 file、shared memory、
   temporary file 三种介质并给出临时目录。
2. **平台不支持该介质**——应用侧不可修。共享内存介质在 Android 上由上游无条件
   拒绝（见决策二）。

日志无法区分二者，因此归因必须写明「上游按 limits/平台判定拒绝」，并单列应用侧
已做的启用动作，避免把仍存在的上游限制误记为应用缺陷、或把已修项重复计入。

## 决策四：日志纪律写成规范而不是只写在实现注释里

本轮前，应用的日志分级、诊断限频、外部条目归因三件事都只存在于实现注释与
归档变更的散文里：实现改了，没有规范约束，后一次改动很容易把它改回去。故新增
`logcat-hygiene` 能力承载这三条。

## 验证

- `openspec validate --all --strict`：34 项主规范 + 本变更的增量全部通过。
- 规范条目与实现逐条对照：分级（`native/src/terminal/session.rs`、
  `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`）、
  限频（`TerminalRuntime.kt` 的 500ms 窗口）、介质启用
  （`native/src/terminal/ghostty_terminal/internal.rs`）均有对应实现。
- 本轮不引入实现改动，故无新增构建产物需要回归。
