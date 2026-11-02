# prohibited-features Specification

## Purpose

`docs/specification/PROHIBITED.md` 明列的功能不得进入产品，也不得以实验开关、
设置项、说明文字或残留代码的形式变相实现。本能力把禁止项转成可验收的边界。

## Requirements

### Requirement: 不实现禁止的终端能力

应用 MUST NOT 实现选中菜单的 ◀ / ▶ 锚点移动项、粘贴确认对话框、用户可编辑的
环境变量或 `termux.env`、会话数据持久化与恢复、背景图片/透明背景/背景模糊、
无障碍朗读、内嵌 proot、内嵌 bootstrap 或预装发行版、桌面环境或 X11。

#### Scenario: 粘贴直接写入终端

- **WHEN** 用户粘贴包含换行的文本
- **THEN** 文本直接写入终端，不弹出确认对话框

#### Scenario: 进程重启不恢复旧会话

- **WHEN** 应用进程被系统回收后重新启动
- **THEN** 应用不恢复旧会话数据，也不从持久化存储重建终端状态

#### Scenario: 不提供环境变量编辑入口

- **WHEN** 用户浏览设置与应用功能
- **THEN** 不存在环境变量编辑项、接收用户环境变量的入口或 `termux.env` 逻辑

#### Scenario: Shell 的标准环境不受影响

- **WHEN** 应用启动 Shell
- **THEN** Shell 仍拿到 `TERM`、`HOME`、`PATH` 等标准环境变量；它们只用于启动 Shell，
      不承载用户设置，也不在应用内部传递数据

### Requirement: 不实现禁止的外部集成

应用 MUST NOT 提供 Model Context Protocol、termux-api、termux-exec、termux-am、
Termux:Tasker、Termux:Boot 或 Shizuku 集成。实现中 MUST NOT 保留这些功能的占位
入口、协议处理器或仅为未来预留的配置项。

#### Scenario: 不暴露外部服务入口

- **WHEN** 用户浏览终端菜单与设置
- **THEN** 不出现 MCP、termux-api、termux-exec、termux-am、Tasker、Boot 或 Shizuku
      的启用入口

#### Scenario: 不预置未声明的协议处理器

- **WHEN** 审阅网络协议、Intent 与后台任务入口
- **THEN** 不存在为上述禁止集成服务的处理器或占位实现

### Requirement: 不实现禁止的设置能力

设置页 MUST NOT 提供光标闪烁开关或速度调节、Shell 启动入口状态、修饰键栏布局编辑
器、自定义终端主题、固定/取消固定侧边面板的按钮，或实体键盘快捷键的设置界面。

内置主题的预览、Shell 启动入口的路径与参数字符串、以及
`termux.properties` 中 termux-app 兼容的固定会话快捷键不属于上述禁止项：后者是配置
契约里的固定快捷键（`shortcut.create-session` 等），不是用户可配置的绑定界面。它们仍
分别受主题、Shell 与配置能力的约束。

#### Scenario: 不提供光标闪烁设置

- **WHEN** 用户打开终端设置
- **THEN** 不存在光标闪烁开关或闪烁速度调节条

#### Scenario: 不提供修饰键布局编辑器

- **WHEN** 用户打开修饰键栏设置
- **THEN** 不存在布局编辑、按键增删排序、尺寸调整、预览拖动或重置按钮

#### Scenario: 不提供自定义主题编辑

- **WHEN** 用户浏览主题设置
- **THEN** 只能预览内置主题，不存在新建、编辑或删除自定义主题的功能

### Requirement: 不解析禁止的配置文件

应用 MUST NOT 读取或解析 `colors.properties` 与 `termux.float.properties`。文件存在
MUST NOT 改变配色、窗口行为或任何其他运行结果。

#### Scenario: 存在 colors.properties

- **WHEN** 用户文件目录中存在 `colors.properties`
- **THEN** 文件内容不被解析，终端仍只使用应用主题

#### Scenario: 存在 termux.float.properties

- **WHEN** 用户文件目录中存在 `termux.float.properties`
- **THEN** 文件内容不被解析，应用不出现浮动窗口行为
