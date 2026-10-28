## MODIFIED Requirements

### Requirement: postinst 幂等执行

换入后 MUST 运行 `var/lib/dpkg/info` 下 postinst（锁防重叠、陈旧锁删除重试、EEXIST 视为他者持有不报成功）；缺目录 MUST 视为成功；失败取前三段 stderr 摘录上报。

stderr 与 stdout MUST 由守护线程排空到 EOF（提前停止会让子进程卡在写满的管道上），
但保留量 MUST 有上限——诊断只取前几百字符，保留量却按子进程全部输出增长时，一个崩溃
后循环刷错误的脚本就能把应用进程读成 OOM。排空结果 MUST 以线程安全方式交接给等待方：
`join` 超时（孙进程仍持有管道）后仍会读取，非线程安全的容器不能跨线程使用。

#### Scenario: 缺目录视为成功

- **WHEN** `var/lib/dpkg/info` 不存在
- **THEN** postinst 阶段成功

#### Scenario: 失败摘录上报

- **WHEN** postinst 失败
- **THEN** 返回前三段 stderr 摘录，状态为 ERROR

#### Scenario: 崩溃脚本刷爆 stderr 不 OOM

- **WHEN** 某个 postinst 向 stderr 写出远超上限的数据
- **THEN** 排空 MUST 仍读到 EOF 以让子进程退出，但保留量 MUST 不超过上限，
  MUST NOT 因诊断缓冲把进程读成 OOM

#### Scenario: 排空超时后安全交接

- **WHEN** 子进程已退出但孙进程仍持有管道，等待方 `join` 超时
- **THEN** 等待方读取已收到的前缀 MUST 是线程安全的，MUST NOT 出现数据竞争
