# Proposal

## Why

可复核的真机 logcat 显示：能力探测（共享内存与临时文件介质）与 DECSACE 复位
两类协议动作到达终端后，上游逐条打警告。两者的共同根因是「客户端按协议在
探测终端能力」——终端无结果即客户端无法回退。Android 无 POSIX 共享内存、
客户端临时文件在子进程沙箱的私有目录里终端进程看不见，而 DECSACE 序列上游
未实现。这两条都不是应用逻辑错误，却持续淹没日志闸门；且前轮着重确认「应答
必须与上游逐字节一致」，本层代答需要规约钉住不得自创错误语义。

## What Changes

- 新增 VT 输入预处理（`input_filter.rs`）：丢弃 DECSACE（`CSI * x`），代答
  本环境无法服务的 Kitty 介质探测（错误应答、静默规则与上游逐字节一致）；
  能服务的介质一律交回上游。`docs/STATUS.md` 的三条已核实条目标注收口与归因，
  其余五条保留为未解决。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `kitty-graphics`：增补「无法服务的介质由本层按上游错误语义代答」要求。
- `logcat-hygiene`：「上游未实现的协议动作」场景增补本层代答的落地位置与判定口径。

## Impact

- `openspec/changes/2026-11-07-answer-kitty-probes-and-trim-decsace/`：提案、规约增量与tasks。
- `native/src/terminal/ghostty_terminal/internal/input_filter.rs`：新模块与 22 例单测。
- `docs/STATUS.md`：三条已核实条目标注收口与归因。
