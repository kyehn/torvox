# Design

## Context

基线见 `proposal.md` Why 与 `docs/specification/DESIGN.md` 终端/Shell 节。五组并行核验已把 25 仓下载到 `$TMPDIR/ecosystem`、对号核对远端并 `fetch` 到最新后逐文件取证：ghostty-android-terminal 像素 winsize（`pty_jni.c:85-110`）、前台组优先收尾（`ptyHangupForeground:356-380`）、无变化 resize 跳过（`TerminalSession.java:400-417`）、搜索 `MAX_MATCHES 50000` 环形加 `search_dirty`（`terminal_jni.c:37/90/1865-1878`）、Termux 菜单分层/LD 去化/退出码、monet 全会话刷新、terminator 设置二分与 seccomp 安全 fork、charon 双模 IME 与搜索/kitty/鼠标编码、rafa `WcWidth` 守卫等 P0 证据成立；复验纠正此前附录 2 处误判（`MAX_MATCHES` 真实存在、`kotlin-app` 仅 2 个单测文件），`Rin` 实为 `vte` 包装、charon 包路径为 `com.cocakova.charon` 等路径口径以研究报告 §20 收敛，不触及规范台账。另有在途变更 `2026-10-11-complete-ime-editor-contract` 已覆盖 IME 填坑，不在本次实现范围内。

## Goals / Non-Goals

**Goals:**

- PTY 生命周期三项行为有单测锚点：像素 winsize 组装、no-op 跳过、前台组优先收尾。
- 设置下发二分语义有集成锚点：live 全量推送、spawn-time 仅新建。
- 搜索导航上下文与不重扫有单测锚点；宽表孤立代理项有边界单测。
- 研究报告 §19 收敛 2 处口径差异（`MAX_MATCHES` 真实存在、`kotlin-app` 仅 2 个单测文件）并新增 §20 五组复验证据，每项给出修正后表述与证据位置。

**Non-Goals:**

- 不实现录制回放金标语料（`terminal-state-regression` 已按 TESTING 不做回归，不引入）。
- 不引入 Monet 取色、自研 URL 扫描、正则搜索、会话持久化等禁区功能。
- 不改 `upstream-alignment` 台账行与门槛（见提案说明）。
- 不动 IME 编辑契约实现（在途变更负责）。

## Decisions

- PTY 像素字段由 Kotlin 侧实测单元格度量下发、Rust 侧组装 `winsize`：与既有 `applyGridResize` 同源，避免两处各算。替代方案为 Rust 侧缓存度量，因 Surface 重建时序不可靠而否决。
- 收尾采用挂断前台组优先、有界轮询后升级：对齐 ghostty `ptyHangupForeground` 与 puppy 宽限序列的交集（HUP 加 200ms 宽限加 KILL），避免平台差异，各平台只调超时常量。
- 设置二分沿用 terminator 与 monet 双证据模式：`SettingsRepository` 区分 live 与 spawn-time，live 经既有 Rust 下发通道扇出全部会话。替代方案为重启生效，因 DESIGN 要求快速启动与无卡顿而否决。
- 搜索导航复用已缓存命中表并保留上下文：与 ghostty 定位留上下文同形，但上限与字面语义仍走本仓 `regex` 路线，不移植大小写折叠表。
- 宽表守卫只加长度检查，不改宽表本身：anoterm 原则与 rafacodephi 补丁的交集即边界守卫，行为面最小。

## Risks / Trade-offs

- [像素度量时序] Surface 未就绪时度量为零导致像素字段为零 → 下发前以最后一次有效度量兜底，无效时跳过像素更新只传行列。
- [前台组不存在] 无前台进程组时 killpg 返回 ESRCH → 视为已退出，直接进入 shell 回收，不报错。
- [设置扇出放大] 全量推送在多会话时放大 JNI 调用 → 复用会话列表打开时的节流语义，合并 2 秒窗口内的重复推送。
- [搜索表过期] 导航复用命中表期间新输出到达 → 内容版本号变化即失效缓存，下一次导航前重扫。

## Migration Plan

- 纯加法规范，无存量迁移；实现分四组按 PTY、设置、搜索、CJK 顺序落地，每组自带单测与文档。
- 回滚策略为整组 revert，不部分回滚。

## Open Questions

- 无。
