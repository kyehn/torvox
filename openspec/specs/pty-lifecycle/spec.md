# pty-lifecycle Specification

## Purpose

PTY 生成与收尾的生命周期行为：生成播种行列尺寸、resize 携带像素尺寸、无变化时跳过 resize、前台进程组优先收尾、IUTF8 语义与上游输入路径一致。本能力覆盖 Rust 侧 PTY 拥有者与 Kotlin 侧尺寸来源之间的可观测契约。

## Requirements

### Requirement: 生成播种行列尺寸且 resize 携带像素尺寸

生成时的内核 winsize MUST 按调用方给的 rows/cols 播种，使 shell 在首次 resize 前
不看到 0x0；像素字段在首次 `setPixelSize` 之前为 0，MUST NOT 在生成处按单元格度量
推算——生成调用的入口只给行列数。

非法尺寸 MUST 在播种前被显式拒绝：零行/零列不成网格，超出 `u16` 范围的行列 MUST NOT
被静默截断成合法网格（70000 列截断后为 4464 列，截断结果仍是合法尺寸，无日志可循）。
生成与 resize MUST 共用同一条判据——同一个尺寸在两条路径上不得得到两种结果。

resize 携带的 winsize MUST 含像素字段（列数乘单元格宽、行数乘单元格高），尺寸无变化的
resize MUST 跳过，不向 PTY 发送信号，避免无辜 `SIGWINCH` 导致行被擦除。

#### Scenario: 生成播种行列尺寸

- **WHEN** 以 24 行 80 列生成会话
- **THEN** 内核 winsize 的行列为 24 与 80，像素字段为 0

#### Scenario: 非法尺寸被显式拒绝

- **WHEN** 以 0 行/0 列或超出 `u16` 范围的行列生成会话或请求 resize
- **THEN** 操作显式失败，MUST NOT 把越界值截断成合法网格，且 MUST NOT 触及 PTY

#### Scenario: resize 携带像素字段

- **WHEN** 在已给定像素尺寸的会话上请求 resize
- **THEN** 透传的 `ws_xpixel/ypixel` 等于列数乘单元格宽与行数乘单元格高

#### Scenario: 无变化 resize 不发信号

- **WHEN** 请求的列行与上次生效值相同
- **THEN** 本次 resize 被跳过，终端内容不被擦除

### Requirement: 收尾先终止前台进程组

会话关闭 MUST 先向 shell 所在进程组（`getpgid(child)` 取到的组）发送挂断信号，
再回退关闭 shell 进程；挂断后 MUST 以有界宽限等待退出，超时再升级为强制终止，
避免前台子进程残留。子进程经 `setsid` 自成会话首进程，不存在独立的
`subtree_leader` 概念，规范 MUST NOT 再使用该词。

#### Scenario: 前台进程组先被挂断

- **WHEN** 关闭存在前台子进程的会话
- **THEN** shell 所在进程组先收到挂断信号（`kill(-pgid, SIGHUP)` 并补 `SIGCONT`），
  shell 进程随后被关闭；子进程与本进程同组时退化为直杀子进程

#### Scenario: 有界轮询后升级

- **WHEN** 挂断信号发出后进程未在 50ms（`TRAILING_EXIT_GRACE`）内退出
- **THEN** 会话关闭流程升级为强制终止（`kill(-pgid, SIGKILL)`，失败再直杀子进程）
  并继续回收资源

### Requirement: IUTF8 语义与宽字符退格一致

PTY 从端 MUST 置 `IUTF8` 标志，使内核按 UTF-8 处理擦除；CJK 字符退格 MUST 一次擦除一个完整汉字，与码点 1:1 语义一致。

#### Scenario: CJK 一次退一字

- **WHEN** 终端处于 UTF-8 模式且删除一个汉字
- **THEN** 一次退格擦除该汉字的全部字节，不残留半个字符

### Requirement: VT 应答回写逐帧有界

VT 引擎的应答缓冲（DECRPM/DSR/DA 等 `on_pty_write` 回调产物）无界，而每次回写是同步
写且可等到帧级排空期限，全程持会话锁。回写 MUST 逐帧有界：写完上限条即止，其余留队
下帧续写；首次写失败 MUST 停止本帧的剩余回写（MUST NOT 按同一失败条件逐条重试），
失败那条与未写部分 MUST 留待下帧。待回写队列 MUST 显式封顶，超出即丢最旧并记错误
（应答是请求-响应语义，不被消费的应答不会被回头读）。

#### Scenario: 病态输出下单帧回写条数有上界

- **WHEN** 一个输出块内含远超单帧上限条数的 DSR 查询
- **THEN** 本帧写出的条数恰为上限，其余留在队列中下帧续写，既不丢也不积压在本帧

#### Scenario: 首次写失败即停

- **WHEN** 回写过程中某一条因主端满而失败
- **THEN** 本帧不再尝试其余应答，失败那条回到队首待下帧重试

### Requirement: 拆卸可重入、读循环可中断且组杀不误伤本进程

拆卸路径 MUST 可重入：重复请求退出 MUST NOT 发出第二组信号、MUST NOT panic，
第二次调用与第一次的结果一致。

等待线程已回收子进程并置位退出标志后，拆卸 MUST NOT 再按该 PID 发信号；
`getpgid(child)` 返回 `ESRCH` 时同样 MUST 直接返回，避免 PID 被系统复用后误伤无关进程。

读取循环 MUST 在 PTY 消失或错误致命时自行停止，MUST NOT 对从端关闭导致的 `EIO` 无限重试；
瞬时错误重试的判定 MUST 由纯函数表达并有其单测。

拆卸 MUST 不因读循环阻塞：请求退出时先断开输出通道使阻塞的发送端立刻返回，
读线程与等待线程的 join MUST 有宽限期限，超时即分离线程而 MUST NOT 无限等待。

组杀 MUST 先确认目标组不是本进程自身的组，且组号下界 MUST 大于 1；
确认不了（本进程组号取不到）时 MUST 退化为直杀子进程。

#### Scenario: 重复拆卸

- **WHEN** 对同一会话连续两次请求退出
- **THEN** 只发一组挂断与强杀信号，第二次调用不 panic 且结果与第一次一致
      （`native/src/terminal/session/mod.rs` 的 `request_exit` 以 `exited` 标志短路，
      并把输出通道换新使后续取用仍安全）

#### Scenario: 子进程已回收后不再发信号

- **WHEN** 等待线程已回收子进程，随后才请求退出
- **THEN** 拆卸直接返回，不调用 `kill(2)`，避免命中复用后的 PID

#### Scenario: 从端关闭后读循环停止

- **WHEN** PTY 从端关闭使 `read(2)` 返回 `EIO`
- **THEN** 读取线程停止并退出循环，而不是持续重试
      （判定纯函数为 `native/src/terminal/session/spawn.rs` 的 `read_error_action`，用例
      `read_error_action_classifies_errno`）

#### Scenario: 拆卸不被阻塞的写路径拖住

- **WHEN** 子进程不读 stdin 而持续产生输出、输出通道已满
- **THEN** 请求退出仍先断开通道，读线程与等待线程在宽限期内 join

#### Scenario: 子进程尚未 setsid 时不组杀

- **WHEN** 子进程在 `setsid` 前仍与本进程同组
- **THEN** 本次收尾退化为直杀子进程，MUST NOT 以该组为信号目标
      （目标选择纯函数为 `native/src/terminal/session/lifecycle.rs` 的
      `exit_signal_target`，用例 `exit_signal_target_declines_group_kill_when_unsafe`：
      同组、组号 ≤ 1、本进程组号取不到三种退化都直杀子进程）

### Requirement: 网格 resize 被丢弃时不得把旧尺寸当权威

resize 的结果 MUST 区分两种情形：PTY 与网格都已缩放，以及 PTY winsize 已变、
而发往 VT 线程的网格命令被丢弃（通道满或 VT 线程卡住）。被丢弃时本会话缓存的
网格尺寸仍是旧值，调用方 MUST NOT 把它当作已生效的权威值发布
（`native/src/terminal/session/mod.rs` 的 `ResizeOutcome`）。

被丢弃 MUST 置脏，使下一次 resize 即使行列与上次相同也重试修复——这是
「尺寸无变化的 resize 跳过」的唯一例外，PTY 与网格的不一致 MUST NOT 静默留存。

#### Scenario: 网格命令被丢弃后可重试

- **WHEN** 通道满导致某次 resize 的网格命令被丢弃
- **THEN** PTY winsize 已变而网格尺寸仍为旧值，且会话已被置脏

#### Scenario: 同尺寸重试不被跳过

- **WHEN** 上一次 resize 被丢弃后，再次以相同行列请求 resize
- **THEN** 命令被重发而 MUST NOT 按「尺寸无变化」跳过

### Requirement: 子进程侧的 fork/exec 不变量

子进程路径 MUST 只使用 `fork()` 之前已分配的数据，MUST NOT 在 fork 之后分配堆内存或取
可能加锁的资源。

子进程 MUST 依次：关闭多余 fd、`setsid()` 并经 `TIOCSCTTY` 把从端设为控制终端、
把从端 `dup2` 到 0／1／2、以显式 `argv` 与 `envp` 调用 `execve`。主端 fd MUST 带
`FD_CLOEXEC`，MUST NOT 泄漏给子进程。

执行失败 MUST 可诊断：errno MUST 编码进退出码（`100 + errno`，errno 范围 1..=133），且 MUST 把
可读原因写进从端，MUST NOT 静默退出使调用方只看到「空会话」。

等待线程 MUST 只把 `100 + errno`（errno 1..=133）的退出码解释为 `execve` 失败；
其他退出码（例如 shell 的 `exit 254`）MUST NOT 记为执行失败。

termios MUST 置 `IUTF8`，且 MUST 关闭软件流控（`IXON`／`IXOFF` 清零）。

#### Scenario: 从端成为控制终端

- **WHEN** 会话生成成功
- **THEN** 子进程的 0／1／2 均指向从端，且该从端是其控制终端（`setsid` 后 `TIOCSCTTY`）

#### Scenario: 主端不泄漏给子进程

- **WHEN** 子进程完成 exec
- **THEN** 主端 fd 已按 `FD_CLOEXEC` 关闭，子进程内不可见

#### Scenario: exec 失败可诊断

- **WHEN** 目标二进制不存在或不可执行
- **THEN** 退出码含 errno 编码（由等待线程解码并在 logcat 可观测），
      且从端出现可读的错误行 `execve failed: errno=N`

#### Scenario: 普通非零退出不误报 exec 失败

- **WHEN** shell 正常执行并返回 `exit 254`
- **THEN** 等待线程按普通退出码处理，MUST NOT 输出 `possible execve errno` 错误日志

#### Scenario: 软件流控关闭

- **WHEN** 读取会话的 termios
- **THEN** `IUTF8` 置位，`IXON` 与 `IXOFF` 均为零

### Requirement: 主端满时写入按期限等待且不丢字节

PTY 主端带 `O_NONBLOCK`：子进程不读 stdin 时写入返回 `EAGAIN`。写入方 MUST NOT
因此丢弃剩余字节——丢弃会让粘贴被静默截断，也会把 VT 应答截断在转义序列中间，使
子应用永久等待一个永远不完整的回答。

写入方 MUST 等到主端重新可写，且等待 MUST 受一个**绝对墙钟期限**约束。期限 MUST
按调用场景给定：用户输入可等满排空上限（等的是子进程消费），逐帧回写的 VT 应答走
帧级短期限（全程持会话锁，等满会把渲染与看门狗一起按住）。

期限 MUST 只在**已有进展**时续期，且续期 MUST 由调用方显式选择：逐帧回写给的是
绝对预算，续期等于取消封顶。

主端挂断（子进程退出）时 MUST 立即报错，MUST NOT 继续空转到期限——否则整段期限
变成满速空转，既烧 CPU 又拖住持锁的调用方。

被信号打断 MUST NOT 记为失败，且 MUST NOT 消耗期限。

写入失败时 MUST 一并给出**已写出的字节数**：调用方据此只把未写出的部分退回队列，
否则「部分写出后超时」会让已发出的前缀被重发，子应用收到两条应答。

#### Scenario: 主端满时等满期限后如实报未写出的长度

- **WHEN** 主端缓冲已满，写入只写出前若干字节后持续 `EAGAIN`
- **THEN** 写入在期限到达时返回错误，并带上已写出的字节数；调用方只把剩余部分退回队列

#### Scenario: 主端挂断立即失败

- **WHEN** 子进程退出使主端挂断
- **THEN** 写入立即返回错误，MUST NOT 等到期限

#### Scenario: 信号打断不算失败

- **WHEN** 等待主端可写期间收到信号
- **THEN** 写入继续等待同一期限，MUST NOT 把本次打断报成失败

#### Scenario: 逐帧回写不续期

- **WHEN** 逐帧回写 VT 应答时主端持续满
- **THEN** 期限不被续期，本帧至多等待一个帧级期限即让出

### Requirement: 逐帧回写的期限覆盖等锁与写入整段

逐帧回写取得主端写入锁所耗的时间 MUST 从同一份期限里扣除，MUST NOT 让「等锁」与
「写入」各用满整份期限。两段各用满时最坏耗时是期限的两倍，与「逐帧回写全程持会话
锁、以墙钟为界」的约定不符——一次锁争用就能把逐帧取同一把锁的渲染与看门狗按住双倍
时间。

实现：`native/src/terminal/session/pty_master.rs` 的 `write_with_timeout_counted`
在取锁前记下起点，取到锁后把已耗时间从期限里扣掉再交给 `write_all`。

#### Scenario: 等锁与写入共用一份期限

- **WHEN** 逐帧回写 VT 应答时主端写入锁被用户输入占住一段时间
- **THEN** 等锁耗去的时间从写入份额里扣除；整段（等锁 + 写入）不超过一份期限

### Requirement: 持会话锁的应答写入以墙钟为界

任何**持会话锁**的 PTY 写入路径 MUST 使「等主端写入锁 + 排空」整段受一个**绝对墙钟
期限**约束：取锁 MUST 有界（期限内等不到即放弃本次写入），排空 MUST NOT 因部分写入
而续期。

本纪律适用于**全部**持会话锁的应答写入路径，不限于逐帧收割：OSC 52 剪贴板读取应答
同样持会话锁写 PTY，其取锁与排空 MUST 与逐帧收割同口径。

未写完的字节 MUST 退回续写队列，由后续帧补齐；期限内取不到锁时整条退回队列，且
MUST NOT 记为失败——写入只是被推迟，不是丢失。

实现：`Session::answer_clipboard_read` 经 `PtyMaster::write_with_timeout_counted` 写入
（`try_lock_for` 有界取锁 + 绝对预算排空），未写完的部分与取锁失败的整条都退回
`Session::pty_write_back_queue`，由 `Session::drain_pty_write_back` 逐帧续写。

#### Scenario: 用户输入占着主端写入锁时应答不拖住会话锁

- **WHEN** 用户输入写入正持有主端写入锁，OSC 52 读取应答在另一个线程上写 PTY
- **THEN** 应答写入 MUST 在一个帧级期限内返回，MUST NOT 等待写入锁释放到排空上限

#### Scenario: 取锁失败时应答退回队列而非丢弃

- **WHEN** 应答写入在期限内取不到主端写入锁
- **THEN** 整条应答 MUST 退回续写队列，MUST NOT 被丢弃，且 MUST NOT 记为失败

#### Scenario: 部分写出只退回未写出的部分

- **WHEN** 应答只写出前若干字节后主端持续满
- **THEN** 只有未写出的部分 MUST 退回续写队列，已写出的前缀 MUST NOT 被重发

#### Scenario: 放行后续写把应答补齐

- **WHEN** 应答已退回续写队列，主端随后恢复可写
- **THEN** 逐帧续写 MUST 把整条应答完整送达子进程，MUST NOT 截断在 base64 中间

### Requirement: 同一帧内的成批应答写入共享一份墙钟预算

持会话锁的成批应答写入（OSC 52 读取请求的洪泛削减回写）MUST 以**帧**为单位共享
一份绝对墙钟预算：每条写入只拿该预算的**剩余**部分，预算用尽即止本帧剩余写入。

按条各拿满额时，最坏耗时是「条数上限 × 单次期限」——16 × 50ms = 800ms，远超一帧
预算；渲染线程就此停摆，看门狗会误判渲染线程已死并关掉会话。条数上限与墙钟预算
各管一件事：前者封顶「写多少次」，后者封顶「这些写占住渲染线程多久」，两者缺一不可。

被削减的请求本就是「拒绝」（空串应答），不写即拒绝；未写完的部分 MUST 退回续写
队列，由逐帧收割补齐。

实现：`native/src/android/ffi/render/clipboard.rs` 的 `handle_clipboard_reads` 以
`pty::WRITE_BACK_TIMEOUT` 为帧级预算建立截止时刻，逐条扣减后作为 `budget` 传给
`Session::answer_clipboard_read`（该参数即「等主端写入锁 + 排空」整段的绝对墙钟
上限）。

#### Scenario: 子进程不读 stdin 时一帧的削减回写有界

- **WHEN** 远端持续发 OSC 52 读取请求且子进程不再读 stdin（每次写入都等满排空期限）
- **THEN** 一帧内削减回写占用的墙钟时间 MUST 不超过一份帧级预算，MUST NOT 随
  每帧回写条数线性增长

#### Scenario: 预算用尽即止而非逐条再等

- **WHEN** 帧级预算在本帧的削减回写中被耗尽
- **THEN** 本帧 MUST NOT 再发起削减回写，MUST NOT 以同一条款继续等待排空期限

### Requirement: 会话生成与切换后必须补齐 PTY 像素字段

会话生成成功后、切换到某会话后，终端 MUST 把当前 Surface 网格区的像素尺寸写入该会话
PTY winsize 的 `ws_xpixel`/`ws_ypixel`，MUST NOT 让像素字段停留在 0。补齐 MUST 发生在
网格对齐之后，MUST NOT 被随后的 resize 抹掉；高度口径 MUST 与网格计算一致（扣除修饰键
栏与输入法遮挡）。

像素感知的客户端经 `TIOCGWINSZ` 读屏幕像素尺寸，读到 0 便判定终端不支持按像素报告屏幕
尺寸并退出；后台会话在 Surface 尺寸变化时不被触及，切回来时像素字段仍是旧值。

#### Scenario: 新建会话后像素字段就位

- **WHEN** 应用在给定 Surface 尺寸下新建会话
- **THEN** 该会话 PTY 的 `ws_xpixel`/`ws_ypixel` MUST 为网格区像素尺寸，MUST NOT 为 0

#### Scenario: 切换会话后像素字段刷新

- **WHEN** 从后台切回一个在 Surface 尺寸变化期间未被触及的会话
- **THEN** 该会话 PTY 的像素字段 MUST 已更新为当前 Surface 网格区像素尺寸

#### Scenario: 像素字段在网格对齐之后写入

- **WHEN** 切换会话并对齐网格
- **THEN** 像素字段 MUST 在对齐之后写入，MUST NOT 被对齐动作抹回 0

### Requirement: OSC 52 读取请求的收割与应答须对每个会话有界

OSC 52 读取请求进入的会话队列 MUST 每帧被排空——**包括后台会话**：只排活跃会话时，
后台程序持续发读请求会让队列进一条、从不取出（内存无界增长），且那些程序永远收不到
应答。队列字段文档 MUST 只写「每帧排空」这一约束，MUST NOT 声称别的机制（如下游上限）
能替代它。

队列长度 MUST 有上限（`MAX_PENDING_CLIPBOARD_READS`），且该上限 MUST 同时约束到达
路径与退回路径：渲染暂停或削减时请求退回队列逐帧再答，只约束到达路径则退回的请求
逐帧净增，内存无界增长。超限 MUST 丢最旧并记限频告警。

削减分支（单会话未作答数超限）MUST 另设**每帧**回写上限与**帧级**墙钟预算：削减本身
要同步写 PTY，子进程不读 stdin 时每次写入可等到数十毫秒，而该代码跑在渲染线程上；
一个 8KB 输出块即可塞约 900 条读请求，不封顶等于把渲染线程整帧交给病态远端。超出
每帧上限或预算用尽的请求 MUST 退回队列待下一帧再答，MUST NOT 丢弃（远端在等应答）。

#### Scenario: 后台会话的读请求被应答

- **WHEN** 后台会话的输出里含超过单会话上限的 OSC 52 读取请求
- **THEN** 请求队列被排空，且 PTY 上出现空串应答（削减路径的外部可见结果）

#### Scenario: 削减回写每帧有上界

- **WHEN** 单帧内的削减回写次数远超每帧上限
- **THEN** 实际回写次数不超过上限，且至少发生过一次（削减分支并未整体失效），
  超出的请求退回队列待下一帧再答，MUST NOT 被丢弃

#### Scenario: 退回队列的请求受同一上限约束

- **WHEN** 渲染暂停或削减期间读请求持续退回会话队列
- **THEN** 队列长度 MUST 不超过上限，超出的按丢最旧处理并记限频告警

### Requirement: OSC 52 读取应答的负载须有上界

OSC 52 读取应答（宿主剪贴板 → PTY）的负载 MUST 不超过字节上限
（`MAX_CLIPBOARD_PAYLOAD_BYTES`，1MiB）；超限 MUST 截断到上限，截断点 MUST 落在 UTF-8
字符边界，并 MUST 记一条限频告警。

宿主剪贴板文本的长度不受本仓控制，而应答要整份 base64 编码后写进 PTY：无界负载会放大
内存占用与写入量，写不完的部分还要整条留在续写队列里。写入方向早已封顶，读取方向
不得是唯一一条无界的剪贴板载荷通道。

实现：`Session::answer_clipboard_read` 经 `crate::terminal::truncate_clipboard_payload`
截断（与写入方向同一上限与字符边界切点）。

#### Scenario: 宿主剪贴板文本超过上限

- **WHEN** 远端请求读取剪贴板且宿主剪贴板文本超过 1MiB
- **THEN** 应答负载 MUST 被截到上限且截断点落在字符边界，远端 MUST 收到合法的 OSC 52 帧

#### Scenario: 超限应答的截断出声

- **WHEN** 读取应答的负载被截断
- **THEN** MUST 记一条带窗口内累计次数的告警，MUST NOT 静默截断

### Requirement: OSC 52 读取应答的写回由进程内唯一写线程承担

远端发来的每条 OSC 52 读取请求 MUST NOT 各自占用一个线程。应答写回要等主端写入锁
（最坏 `WRITE_BACK_TIMEOUT`），而单会话每帧的未作答上限为 8：每请求一线程时，洪泛
每帧要付一次线程创建（实测 24.6–26.5µs）共 198–212µs 渲染线程时间，每秒新建 480 个
线程。进程内 MUST 至多有一个应答写线程，宿主答复后入队由它串行写回；写线程 MUST 在
首次登记读请求时启动。

待答槽位 MUST 记录登记时刻，并在超过 `CLIPBOARD_ANSWER_TIMEOUT` 后被清掉：槽位
留着既占本会话的「未作答」额度（后续请求全被洪泛削减），又让请求表随远端反复请求
无界增长。清扫 MUST 按固定节拍进行，MUST NOT 只在应答队列空闲时进行——宿主只答
部分请求时队列常非空，只在空闲时清扫会让其余槽位永不超期。槽位在宿主答复时立即
取走，故重复或迟到的答复 MUST 是无操作。

注册表锁 MUST NOT 跨过 JNI 调用：宿主答复的字符串解码 MUST 在取走槽位之前完成
（解码可能触发 JVM GC safepoint）。

#### Scenario: 读请求洪泛不把请求数变成线程数

- **WHEN** 远端在一帧内发出远超单会话未作答上限的 OSC 52 读请求
- **THEN** 进程内应答写线程数 MUST 保持为 1，MUST NOT 随请求数增长

#### Scenario: 超期未答复的槽位被清掉

- **WHEN** 宿主未在 `CLIPBOARD_ANSWER_TIMEOUT` 内答复某条读请求
- **THEN** 该槽位 MUST 在清扫节拍到达时被移除，该会话的「未作答」计数随之下降

#### Scenario: 部分答复不阻塞清扫

- **WHEN** 宿主对同一批读请求只答复了一部分，应答队列持续非空
- **THEN** 其余槽位仍 MUST 在超期后按清扫节拍被清掉，MUST NOT 因队列非空而永久驻留

#### Scenario: 重复或迟到的答复

- **WHEN** 同一请求收到第二次答复，或答复在槽位已超期清除之后到达
- **THEN** MUST 是无操作，MUST NOT 再产生一条 OSC 52 应答帧

### Requirement: 回滚行数在会话层按上游区间收窄

会话创建时传入的回滚行数 MUST 按上游 `terminal-transcript-rows` 的区间
（100..=50000）收窄：区间外的值 MUST 回退默认行数（2000），MUST NOT 钳到边界，
MUST NOT 原样放行。

上游的字节预算已解除（行数是回滚区唯一的约束），故行数一旦越界放行，回滚区可一路
长到把进程内存吃光。越界回退默认与上游 `getDefaultIfNotInRange` 同口径，也与
`docs/specification/CONFIG.md`「配置非法时回退默认值」一致。

收窄点是会话层（`native/src/terminal/session/mod.rs` 的
`validated_scrollback_lines`），故 JNI 边界与任何其他调用方共用同一条判据；JNI 边界
只负责把 `jint` 负值折成默认行数（`jint` → `u32` 的有符号转换不经 `as` 截断）。

#### Scenario: 越界回滚行数回退默认

- **WHEN** 会话以区间外的回滚行数创建（如 50001 行）
- **THEN** 该会话的回滚行数 MUST 为默认行数，MUST NOT 按传入值生效

#### Scenario: 区间内的回滚行数原样生效

- **WHEN** 会话以区间内的回滚行数创建（如 50000 行）
- **THEN** 该会话的回滚行数 MUST 按传入值生效，MUST NOT 被收窄成默认行数
