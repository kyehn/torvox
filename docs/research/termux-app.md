# termux-app 深度研究报告

对 [termux/termux-app](https://github.com/termux/termux-app)（v0.118.3）的深度调研结论。termux-app 是本项目 DESIGN 明确指定的首要规范来源（布局、按键、交互均要求与 Termux 一致），因此本研究重点在核实「规范细节的权威出处」与「本项目已对齐项」，并提炼仍可参考的工程实践。

## 概况

四模块结构：`terminal-emulator`（Java 终端引擎，`TerminalEmulator.java` 单文件约 3600 行）、`terminal-view`（View 渲染层）、`termux-shared`（修饰键栏、主题、shell 环境、通知）、`app`（Service/Installer/Activity）。applicationId `com.termux`，`sharedUserId` 绑定全部插件，GitHub 构建以社区共享 testkey 签名。

## 关键实现要点

### 终端会话

`TerminalSession`：JNI `createSubprocess` 建 PTY；双向各一条 `ByteQueue`（进程→终端 64 KiB，终端→进程 4 KiB）；三线程分工——InputReader 读 PTY 写队列、OutputWriter 把队列写回 PTY、Waiter 阻塞 `waitFor` 上报退出码；仿真回调全部经主线程 Handler 消息驱动（`MSG_NEW_INPUT`/`MSG_PROCESS_EXITED`）。工作目录查询走 `/proc/<pid>/cwd` symlink。会话经 `finishIfRunning` 发 SIGKILL 结束。

### bootstrap 安装

`TermuxInstaller`：prefix 存在即跳过；staging 目录先清后用；zip 流式提取（`SYMLINKS.txt` 记录延迟建链，其余条目直接落盘）；完成才 rename 发布；非主用户与 files 目录不可达均有明确的失败对话框。

### 修饰键栏

`SpecialButtonState` 三态（`isCreated`/`isActive`/`isLocked`）：单击激活、长按锁定、读后自动失效（锁定除外）——本项目粘滞键语义的出处。激活反馈是 MaterialButton 文字变色，无缩放动画。auto-repeat 用定时器逐次触发，长按时长取系统 `ViewConfiguration.getLongPressTimeout()`（随无障碍设置变化），并有上下限钳制。默认布局为 2 行 7 列（`DEFAULT_IVALUE_EXTRA_KEYS`：上行 `ESC`、`/`、`-`（弹 `|`）、`HOME`、`UP`、`END`、`PGUP`；下行 `TAB`、`CTRL`、`ALT`、`LEFT`、`DOWN`、`RIGHT`、`PGDN`）。

### 其他权威常量

回滚默认 2000 行（钳制区间 100–50000）；`TERM=xterm-256color`；Android 7+ 因 DT_RUNPATH 显式移除 `LD_LIBRARY_PATH`；failsafe 模式保留宿主 `PATH`/`TMPDIR`。光标为块状并反色其下文字（`invertCursorTextColor`）。`KeyHandler` 持有完整 KEYCODE→escape 序列映射表（F1–F12、方向、Insert/Home/End/PgUp/PgDn、小键盘数字、Break/Back 映射 ESC）。

### 前台服务

`TermuxService` 持会话列表与后台任务；Activity client 与 Service client 分离（`onUnbind` 解除 Activity 引用防泄漏）；wake 与 wifi 锁成对持有；`START_NOT_STICKY`；`onDestroy` 清 TMPDIR 并 kill 全部会话。

### 测试

`terminal-emulator` 配 15+ JVM 测试（`WcWidth`、`KeyHandler`、`DecSet`、`OSC`、`DCS`、`APC`、`Resize`、`History`、`UnicodeInput`、`RectangularAreas`、`ScreenBuffer` 等），以 `TerminalTestCase` 为基座直接驱动仿真器断言屏幕文本。

## 与本项目的对照

### 已对齐项（DESIGN 依 Termux 定规，本项目已实现）

- 修饰键栏布局、粘滞语义、2 行 7 列
- 回滚 2K 行、块状光标不闪烁
- `TERM=xterm-256color`、`COLORTERM=truecolor`、bootstrap 环境变量集合（`LD_LIBRARY_PATH` 均不设）
- bootstrap zip + `SYMLINKS.txt` + staging + 原子 rename 安装流程
- 通知常驻（会话数量 + 退出按钮）、applicationId `com.termux`、AOSP testkey 签名

### 参考点归属

原「可参考点」五项已全部核结，条目随解决即从本文档移除：

1. KeyHandler 映射完整性：逐键对照后补齐 `KEYCODE_BREAK` → 上游 `key::Key::Pause`
   （此前无文本回退，整个键静默失效）；`KEYCODE_BACK` 归系统返回手势，不在键表职责内。
   见 openspec `key-input-encoding`「Android 键码映射覆盖 Termux 键表」。
2. auto-repeat 无障碍联动：长按阈值改取 `ViewConfiguration` 的系统值（越界退回 400ms），
   锁定与自动重复共用。见 openspec `modifier-bar-auto-repeat`。
3. 双 client 生命周期：会话归属 Rust，Kotlin 侧不持 Activity 引用，LeakCanary 用例在位。
4. 主线程消息泵：渲染线程 latch 与合帧唤醒等价，架构印证。
5. `TerminalTestCase` 式直测：`ghostty_terminal/tests.rs` 直接驱动 VT 断言网格。

### 不适用项

- 插件体系（Termux:API / Boot / Float / Tasker / Widget，PROHIBITED 已明确禁止）
- `termux.properties` 与其设置项体系（本项目设置已另行定规）
- F-Droid/GitHub 双分发签名管理（本项目单 testkey）
- sharedUserId 与多 APK 签名兼容约束
