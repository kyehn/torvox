## ADDED Requirements

### Requirement: 状态查询不得取会话锁

VT 线程维护的状态镜像（备用屏激活、鼠标上报开关、PTY 摄入标志）MUST 以共享
`Arc<AtomicBool>` 的形式由会话注册表条目持有，读方 MUST 直接读该原子量，
MUST NOT 经会话锁访问。

会话锁的持有时间无上界：PTY 主端为 O_NONBLOCK，写入在 `WouldBlock` 上 poll 到
子进程消费，且每有部分写入即重置期限（上限 `WRITE_DRAIN_TIMEOUT`，5s）。
一次「粘贴大段文本 + 子进程暂时不读 stdin」即可把会话锁按住数秒。

镜像的刷新方是 VT 线程（每帧一次），值的时效性由刷新频率而非读法决定：
读原子量拿到的是「最近一帧的值」，与经会话锁读到的是同一个值。

#### Scenario: 会话锁被占住时状态查询仍立即返回

- **WHEN** 会话锁被占住（PTY 写入在途），输入路径读取备用屏与鼠标上报状态
- **THEN** 两次读取 MUST 立即返回，MUST NOT 等待会话锁释放

#### Scenario: 注册表条目持有的是同一份镜像

- **WHEN** 会话注册表条目构造完成
- **THEN** 条目上的每个镜像 MUST 与会话暴露的镜像指针相等（`Arc::ptr_eq`），
      MUST NOT 是另建的副本——副本与 VT 线程的写入脱钩，状态永远停在初始值

#### Scenario: 未知会话不抛异常

- **WHEN** 状态查询指向未登记的会话 id
- **THEN** MUST 返回「假」而非抛异常：调用方把它读成「备用屏未激活」/
      「鼠标上报未启用」，两者都是安全回退

#### Scenario: 读取闭包接收注册表条目而非会话

- **WHEN** 状态读取辅助函数的读取闭包被定义
- **THEN** 闭包参数 MUST 是 `&SessionEntry`（持有共享原子量的注册表条目），
      MUST NOT 是 `&Session`——后者只能经会话锁抵达

### Requirement: 状态镜像须由终端层暴露为共享原子量

终端层维护的状态镜像 MUST 以「克隆 `Arc`」的方式暴露给会话层，
MUST NOT 以「读取当时的值再复制」的方式暴露。克隆得到的是同一份原子量，
故 VT 线程的后续写入对读方立即可见；复制得到的是快照，读方永远看到旧值。

实现：`Session::alt_screen_active_flag()` 与 `Session::mouse_tracking_active_flag()`
返回 `Arc::clone(&self.terminal.<镜像>)`。

#### Scenario: 终端层刷新镜像后读方立即看到新值

- **WHEN** VT 线程刷新了备用屏镜像，经会话层暴露的镜像读出的值
- **THEN** MUST 是刷新后的值，MUST NOT 是暴露时刻的快照
