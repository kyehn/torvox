## Context

`ensureMkshPromptRc` 已在 `$HOME/.mkshrc` 落盘短提示符（含 `. /system/etc/mkshrc` 与 `PS1='$ '`），但子进程环境缺少 `ENV` 指向，mksh 不加载该文件。`build_env` 是子进程环境的唯一来源，受 `DESIGN.md` Bootstrap 节白名单约束：只允许设置声明过的变量。

## Goals / Non-Goals

**Goals:**

- `ENV=$HOME/.mkshrc` 进入白名单并由 `build_env` 注入，mksh 获得短提示符。
- bash 行为不变（bash 忽略 `ENV`，设备实验已确认）。

**Non-Goals:**

- 不改动 `ensureMkshPromptRc` 的 rc 内容与写入逻辑。
- 不引入白名单外的任何变量，不做 shell 类型探测与条件注入。

## Decisions

- 值取 `$HOME/.mkshrc` 而非硬编码绝对路径：与 `HOME` 同源，`build_env` 已持有 `ShellEnv.home`，直接拼接即可。
- 无条件注入而非按 shell 区分：bash 忽略 `ENV`，条件分支只会增加复杂度；且启动入口失败不得 Fallback，环境必须确定。
- 条目写入 `DESIGN.md` 白名单：`build_env` 的注释以该节为规范来源，白名单是唯一允许设置变量的依据。

## Risks / Trade-offs

- 若用户删除 `$HOME/.mkshrc`，`ENV` 指向不存在的文件：mksh 回落默认行为，与现状一致，无回归风险。
- `ENV` 对非 mksh 非 bash 的 shell（如 bootstrap 自带的 sh 符号链接指向 mksh 时）同样生效，方向均为加载短提示符，符合预期。
