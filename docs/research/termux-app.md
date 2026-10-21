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

### 可参考点

1. **KeyHandler 映射完整性自查**：以 Termux 的 KEYCODE→escape 表为清单，对照本项目 `TerminalInputEncoder` 与 Rust 侧 key encoding 逐项核对（F 键、小键盘、Break、Back→ESC），补缺即为 bug 修复。
2. **auto-repeat 的无障碍联动**：长按时长取系统值而非硬编码，用户在系统设置加大长按时长后修饰键栏 repeat 同步变慢。本项目若有 auto-repeat 应同源取值。
3. **双 client 生命周期模式**：会话回调持有两个 client（带 Activity 引用与仅 Service 引用），`onUnbind` 主动解引用。本项目会话归属 Rust，但 Kotlin 侧事件消费者的生命周期解耦可对照自查。
4. **主线程消息泵输入模型**：`MSG_NEW_INPUT` 合帧后单次 `onTextChanged`，天然合并不定长 PTY 突发。本项目渲染线程 latch 语义等价，无需改动，列为架构印证。
5. **`TerminalTestCase` 式仿真器直测**：本项目 Rust 侧 `ghostty_terminal/tests.rs` 已是同思路，其 `History`/`Resize`/`UnicodeInput` 用例名可作补测清单。

### 不适用项

- 插件体系（Termux:API / Boot / Float / Tasker / Widget，PROHIBITED 已明确禁止）
- `termux.properties` 与其设置项体系（本项目设置已另行定规）
- F-Droid/GitHub 双分发签名管理（本项目单 testkey）
- sharedUserId 与多 APK 签名兼容约束
