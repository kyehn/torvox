# Proposal

## Why

25 个 Android 终端仓库已逐个下载到 `$TMPDIR/ecosystem`、对号核对远端并 `fetch --depth 1` 到最新（HEAD 无变化即缓存为最新），经五组并行只读深研与关键 P0 证据亲自抽查，与本项目基线（`docs/specification/`）对比后，P0 级可落地参考点集中在 PTY 生命周期、设置下发、搜索导航、CJK 边界四处，且既有生态报告复核附录存在 2 处经复验推翻的口径（`MAX_MATCHES` 真实存在、`kotlin-app` 仅 2 个单测文件），需以规范 delta 收敛。

## What Changes

- 新增 PTY 生命周期行为：spawn 携带像素 winsize、无变化 resize 跳过、前台进程组优先收尾、IUTF8 语义核对。
- 新增设置下发行为：live 设置推送全部会话，spawn-time 设置仅影响新建会话。
- 修改搜索导航行为：定位留上下文、不重扫导航。
- 修改 CJK 宽表边界：行尾孤立代理项必须有长度守卫。
- 修正复核附录中 2 处与复验不符的结论口径（`MAX_MATCHES` 真实存在、`kotlin-app` 仅 2 个单测文件），不改门槛本身。
- 同步更新 `docs/research/终端模拟器生态深度研究报告.md` 附录（五组复验证据与 2 处口径修正），不碰 `docs/specification/`。

## Capabilities

### New Capabilities

- `pty-lifecycle`: PTY 生成与收尾的生命周期行为（像素 winsize、no-op 跳过、killpg 前台组优先、IUTF8 核对）
- `settings-propagation`: live 与 spawn-time 设置的下发范围（全部会话与仅新建）

### Modified Capabilities

- `text-search`: 搜索定位上下文与导航重扫行为
- `cjk-rendering`: 宽字符边界守卫

`upstream-alignment` 台账本次不改：`sohan-f` 含真 Monet 合成文件（`MonetTerminalColors.java` 22KB）而 `ownerroc` 同路径无该文件、仅 `monet_background` 开关，系同源换皮；其余口径差异位于生态研究报告正文，以 `docs/research/` 附录收敛，不触及规范门槛与台账行。

## Impact

- 影响 Rust 会话/PTY 层、Kotlin 设置下发链路、搜索导航与 CJK 宽表实现。
- 影响 `openspec/specs/` 中两项新增能力与两项既有能力行为增补；不引入 `PROHIBITED.md` 禁区功能，不改 `upstream-alignment` 台账行。
- 影响 `docs/research/` 生态报告附录；不修改 `docs/specification/`、`Cargo.toml`、`build.gradle.kts` 等受限文件。
