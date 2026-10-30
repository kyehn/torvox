# termux.properties 三个键支持

## Why

`PROHIBITED.md` 原禁止解析 `termux.properties`。用户要求支持 `default-working-directory`、`terminal-transcript-rows`、`terminal-cursor-style` 三个键，其余键与 `colors.properties`/`termux.float.properties` 继续禁止。

## What Changes

- Kotlin 侧新增 `TermuxProperties` 解析器，仅消费三个允许键，按路径/范围/样式规则校验。
- `TerminalConfig` 增加 `scrollbackLines` 与 `cursorStyle`，`buildConfig` 从配置读取并传入原生。
- 原生 `initSession` 增加 `scrollbackLines` 与 `cursorStyle` 入参，`Session::spawn` 与 `GhosttyTerminal` 应用默认光标样式。
- 更新规范与 OpenSpec 主规范。

## Impact

- 受影响能力：shell 启动目录、回滚行数、光标默认样式。
- 风险：配置文件不存在或非法时全部回退默认值；符号链接与超大文件被跳过。