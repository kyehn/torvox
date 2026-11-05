# Proposal

## Why

本轮审查的两条硬要求是「尽量减少代码、降低 vendor 与硬编码」与「搜索并尽量采用有用
的外部依赖（排除下载量过少/长期未更新的库）」。此前这两项都只有零散的直觉判断，没有
可复核的量化结论：

1. **重复率无基线**。不知道哪些拷贝是真重复、哪些只是 import 块的天然相似，于是
   「重复率低」既不能被证明，也不能被门禁。
2. **依赖健康度无凭证**。直接依赖共 27 个（含 5 个 dev 依赖），却没有任何一份关于
   下载量、最近更新与维护状态的记录；「排除停更库」只能停留在印象。
3. **vendor 状态未核对**。「降低 vendor」需要先回答「仓里到底有没有 vendor 掉的上游
   源码」。

本轮把三件事都变成可复核的事实：用固定参数的重复率扫描给出基线并收敛其中属于真
重复的部分；用 crates.io 的下载量与最近更新时间逐个核对直接依赖；核对仓内是否存在
上游源码副本与子模块。

## What Changes

- 新增留档 `audit.md`：重复率扫描的方法、参数与前后对比；外部依赖的下载量与维护
  状态核对表；vendor 核对结论。
- 收敛四处真重复（已在同一批次实现中落地）：
  - 三处裸 JNI shell 探针用例共用的「建会话 → 切活跃 → 等提示符 → 销毁」夹具；
  - 两个 IME 用例各自实现的输入法可见性读数与「收起并确认隐藏」；
  - 基准里断言与采样各写一份的 `CellInstanceConfig`；
  - 两个会话回归钉共用的「用户写入持住主端写入锁」夹具，以及前景/背景「精确 u8」
    断言的换算。
- 规约 `code-health` ADDED 两条 Requirement：重复实现必须收敛为共享夹具；直接依赖
  必须处于活跃维护状态且替换自实现须先核对维护状态。

## Capabilities

- `code-health`（新增）：重复率与依赖健康度的可复核口径。

## Impact

- `openspec/changes/archive/2026-11-05-code-health-audit/audit.md`（新增）
- `openspec/changes/archive/2026-11-05-code-health-audit/proposal.md`、`tasks.md`（新增）
- `openspec/specs/code-health/spec.md`（新增两条 Requirement）
- 实现侧改动见提交 `refactor(test): 收敛仪器测试与 Rust 用例的重复夹具`。
