# Design

## Context

基线见 `docs/specification/DESIGN.md` Bootstrap 节与 `docs/research/00-汇总与可参考点收敛.md`。实现位于 `android/app/src/main/java/terminal/emulator/installer/`。

## Goals / Non-Goals

**Goals:**

- staging 解压与 Zip 滑移防护有规范锚点。
- SYMLINKS/EXECUTABLES 清单建链语义有规范锚点。
- 原子换入（旧 usr 随机后缀备份保留、失败回滚）有规范锚点。
- postinst 幂等执行与锁语义有规范锚点。

**Non-Goals:**

- 不改安装行为；不引入校验文件与状态标记；不改预设版本。

## Decisions

- 安装状态只认启动入口存在性，不写标记文件：与 `shell-entry` 探测语义同源。
- 旧目录随机后缀保留由用户手动删除：DESIGN 已定，不自动删除。
- 失败只记异常类名：URL 与路径可能嵌入异常消息，防泄露。

## Risks / Trade-offs

- [重命名跨文件系统] staging 与 prefix 同处 filesDir，同文件系统 rename 为原子操作。
- [符号链接跟随] 递归删除不跟随链接，防删目标内容。

## Migration Plan

- 纯加法规范，无迁移；回滚为整组 revert。

## Open Questions

- 无。
