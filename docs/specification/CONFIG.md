# 配置文件

## termux.properties：路径与读取

- 主路径：`/data/data/com.termux/files/home/.termux/termux.properties`
- 次路径：`/data/data/com.termux/files/home/.config/termux/termux.properties`
- 不合并，符号链接与非常规文件跳过且不再检查次路径，文件不存在时全部取默认值。
- 不创建、不写入、不修改任何文件，配置仅在应用启动时读取一次，不监听文件变化。
- 键的默认值、精度、范围、可选值、效果和 termux-app 保持一致，配置非法时回退默认值

## termux.properties：键

只允许下面声明的键，未知项跳过

- `default-working-directory`
- `terminal-transcript-rows`
- `terminal-cursor-style`
- `fullscreen`
- `use-fullscreen-workaround`
- `ctrl-space-workaround`
- `hide-soft-keyboard-on-startup`
- `extra-keys-text-all-caps`
- `terminal-margin-horizontal`
- `terminal-margin-vertical`
- `terminal-toolbar-height`
- `extra-keys-style`
- `extra-keys`
- `shortcut.create-session` / `shortcut.next-session` / `shortcut.previous-session`
- `back-key`
- `volume-keys`
- `soft-keyboard-toggle-behaviour`
- `bell-character`

## colors.properties

- 路径：`/data/data/com.termux/files/home/.termux/colors.properties`
- 只在应用启动时读取一次，不创建、不写入、不修改；符号链接与非常规文件跳过，读取失败按无覆盖处理
- 语义：对当前终端主题的部分覆盖，未写的键沿用主题原色；背景被覆盖而光标未指定时，
  光标色按背景的感知亮度取白或黑
