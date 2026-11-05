# Proposal

## Why

用户提供的六份设备 logcat 是本轮唯一的一手证据来源。第三轮审计之后，应用侧的条目已经
收敛，但新一轮取证又暴露两件事：

1. 应用自身条目里出现了一个数量级级别的刷屏：命令通道丢弃告警在单份日志里占了两万余
   行，淹没其余全部诊断。经复核，这条告警对应的行为（PTY 输出在通道满时被直接丢弃）
   已在既往改动里换成逐帧背压，本轮日志来自改动之前的构建，属**已被修复的旧行为**。
2. Kitty 图像协议的两类上游告警此前只有归因、没有可复核事实：客户端把图像按「文件」
   介质传来时上游打不开路径，按「共享内存」介质传来时上游按平台限制直接拒绝。两者的
   判据都在上游源码里，本轮把源码位置与判定条件钉下来。

`logcat-hygiene` 的「外部与上游条目必须逐条归因并留档」要求每条 W/E/F 归到一类并
写明结论。本轮把六份日志的逐标签计数、归因与处置写成聚合留档，并补上「介质路径的
解析基准由上游决定」这一条此前没有写进规约的事实。

## What Changes

- 新增留档 `logcat-audit.md`：六份日志的逐标签聚合计数与归因结论，逐类写明「应用
  自身／上游终端／平台／外部程序」以及处置。
- 规约 `logcat-hygiene` ADDED「图像传输介质的路径解析基准由上游决定」：文件与临时
  文件介质由上游按终端进程自己的工作目录解析路径，共享内存介质在不支持 POSIX 共享
  内存的平台被上游拒绝；应用侧不改写路径、不伪造文件存在。

## Capabilities

无规范增量之外的实现变更：本轮的实现改动（字形解析结论缓存、管线建立位置）已由
`2026-11-04-fix-glyph-resolution-cache-and-pipeline-setup` 单独记录。

## Impact

- `openspec/changes/archive/2026-11-04-logcat-fourth-pass/logcat-audit.md`（新增）
- `openspec/changes/archive/2026-11-04-logcat-fourth-pass/proposal.md`、`tasks.md`（新增）
- `openspec/specs/logcat-hygiene/spec.md`（新增一条 Requirement）
