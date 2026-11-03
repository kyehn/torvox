# pty-output-backpressure Specification

## Purpose

PTY 输出从读取线程到 VT 引擎之间有两个有界通道（`output_rx` 与 VT 命令通道）。
本规范规定二者拥塞时的行为：**把压力传回上游，不丢字节**。终端状态必须与 PTY 流
逐字节一致——少一块，屏幕与真实输出就永久分叉，而分叉不可自愈、也不可诊断。

## Requirements

### Requirement: PTY 输出不得因下游拥塞而丢弃

VT 命令通道满时，会话 MUST 把**已成形**的载荷留到下一帧重试，并在同一帧内停止从
`output_rx` 取新数据。MUST NOT 丢弃该块，MUST NOT 跳过它去投递更新的块（命令通道是
FIFO，跳过去会让引擎按错误顺序解析同一段字节流）。

停手是背压的实现方式：`output_rx` 是有界通道，读取线程用阻塞 `send`，压力依次传到
PTY 的内核缓冲，最终让子进程的 `write` 阻塞。这是**期望**行为，不是故障。

#### Scenario: 命令通道拥塞

- **WHEN** 命令通道已满，收割到一块新的 PTY 输出
- **THEN** 该块 MUST 存入会话的单槽 `pending_pty_output` 并在下一帧优先补投；
      本帧 MUST 停止取新块；`poll_pty_output` MUST 返回真（本帧确实处理过 VT 数据）

#### Scenario: 补投仍失败

- **WHEN** 下一帧补投时通道仍满
- **THEN** 块 MUST 原样留在槽内继续等待，MUST NOT 取新块，MUST NOT 计入丢弃

#### Scenario: 补投成功

- **WHEN** 通道腾空后补投成功
- **THEN** 槽 MUST 清空，本帧继续按序处理新块

#### Scenario: 终端重置

- **WHEN** 用户触发 RIS 全重置
- **THEN** 槽 MUST 清空：重置的语义是抛弃旧终端状态，把重置前的字节补投进新网格
      等于用旧内容污染新会话

### Requirement: 重投必须复用成形结果

PTY 输出的成形（LF→CRLF、剔除 NUL）是有状态的：判定「本块的 `\n` 前面是否已有
`\r`」要用**上一块**的末字节。故重投 MUST NOT 再次成形。

#### Scenario: 重投跨块 CRLF

- **WHEN** 上一块以 `\r` 结尾，本块只有一个裸 `\n`，且首次投递因通道满失败
- **THEN** 交还的载荷 MUST 是 `\n`（不补回车）；重投 MUST 原样入队；
      若重投重新成形，会得到 `\r\n`——多出的回车把同一行拆成两行

#### Scenario: 丢弃计数不得虚增

- **WHEN** 载荷已交还调用方等待重试
- **THEN** MUST NOT 计入 `cmd_drop_log`：数据仍在应用手里，没有丢

### Requirement: 丢弃路径只保留给无重试槽的调用方

`pty_write` 保留给测试与一次性馈入。它 MUST 在通道满时按窗口累计告警（一个窗口一行、
带窗口内累计条数），MUST NOT 逐次告警——逐次告警正是本次要修的两万余行日志。

#### Scenario: 输出高峰

- **WHEN** 一段持续数十秒的输出高峰（例如 `yes`）期间命令通道反复拥塞
- **THEN** 应用侧 MUST NOT 出现逐块丢弃告警；会话路径不产生任何丢弃类日志
