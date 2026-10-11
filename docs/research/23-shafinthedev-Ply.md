# 23 shafinthedev/Ply

上游：<https://github.com/shafinthedev/Ply>
本地：/tmp/competitor-study/shafinthedev__Ply
版本：主线 2026-05-30 取样，20 文件。Rust 行循环 REPL 加 TextView 壳，约 300 行。全部读完。

## 结论

主循环读行调 `/system/bin/sh -c` 输出，无 PTY、无 VT、无会话、无交互 TUI。`proot.rs:40` 仅桩。包管理为管道远程脚本，属严重反面。Android 侧仅占位。仅记录在案，无可参考点，反面价值：逐行执行不可交互，管道远程脚本不可引入。

证据：`src/main.rs:161`、`src/proot.rs:40`、`android/MainActivity.kt:92`。
