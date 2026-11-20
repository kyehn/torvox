# Design

## Context

本仓是 Kotlin + Compose 壳 + Rust 核（wgpu/Vulkan 渲染、libghostty-vt 解析）的 Android 终端。
`DESIGN.md` 未声明远程／网络终端形态；`PROHIBITED.md` 明禁应用内插件运行时、脚本加载、事件总线与
多路复用、外部服务与 AI 助手集成、会话持久化。`native/Cargo.toml`、`settings.gradle.kts`、
`build.gradle.kts` 是受保护文件，改动须明确同意。

研究要回答的是产品级取舍，不产出实现。因此本变更只做「报告 + 台账登记」，不碰代码。

## Goals / Non-Goals

**Goals:**

- 用一手源码证据回答：支持 Rex／Herdr 时该走应用内扩展程序还是组件库化。
- 逐仓核验 `leizhiyuan/rexpocket`、`shulhi/rexling`、`nimrc/bellterm`、`0cv/herdr-mobile-relay`、
  `kosumic/whip`，并核验 Termux 分层与 termlib 库化形态作为参照。
- 明确区分事实、推论与未知项，给出成本收益结论与可执行顺序。
- 按 `research-ledger` 协议登记索引行与补齐行，使报告日后可删而不丢结论。

**Non-Goals:**

- 不实现插件运行时、不做库化拆分、不接任何远程后端。
- 不改 `docs/specification/` 与 `docs/STATUS.md`，不重开已裁定的能力面。
- 不引入外部代码，不复制上游许可证文本。

## Decisions

- **报告落点定为 `docs/research/`**：台账协议要求 `docs/research` 下每份报告有索引行；上一份报告放在
  `docs/` 根目录导致「未入索引」而被要求退役，故本次按目录约定落位。
- **报告正文不写 git 提交号**：`STYLE.md` 禁止文档写长／短提交号，`semgrep` 亦以规则拦截。精确提交号
  记录在本变更的归档记录里（归档台账是历史证据，允许保留提交号），报告指向该处。
- **只登记、不改写既有裁定**：插件系统、WebSocket 与网络终端形态、远程会话等能力面此前已判为未声明不立项，
  本次索引行引用这些判定，不借新报告重开门槛；新报告的增量只落在「七个外部项目的一手核验」与
  「会话传输接缝的具体形状」两点上。
- **结论采用「先接缝、后后端、库化暂缓」**：插件框架与组件库都不直接产生 Rex／Herdr 能力，而
  `Session::spawn_with_pty` 为 `pub(super)`、`Pty` trait 混合本地 OS 能力、`Session::request_exit` 走本地
  进程组信号，说明真正的技术前置是把字节读写与生命周期从本地 PTY 能力中拆出。
- **库化结论写为「暂缓」而非「不采纳」**：拆 Android library 模块与补 crate 发布元数据都要改受保护构建
  文件，未获明确同意；但「无外部使用者」是概率判断而非事实，故写为待产品决策并记录触发条件。

## 研究基线

研究克隆的外部仓库及其精确提交号（归档台账保留历史证据）：

| 仓库 | 提交号 |
| --- | --- |
| `leizhiyuan/rexpocket` | `bd7510130685d82eb26f2a581e780319a4648497` |
| `shulhi/rexling` | `8d602897d1f7facfff23d21d76d4313ab89a3c98` |
| `nimrc/bellterm` | `0d90a9666fe56b43df098cd544cc70532fdd05be` |
| `0cv/herdr-mobile-relay` | `6cbfbfc7df7960b4e245759eb1eb52d45fbe6459` |
| `kosumic/whip` | `4a54252c9c25fff7972a35472a6728f7e0a9d9c6` |
| `termux/termux-app` | `8629e632fcb95da272221be327db653fb24befe9` |
| `connectbot/termlib` | `23f65c6d8af34e2b58003e386190de2e0d1e911a` |

## Risks / Trade-offs

- 报告结论依赖外部仓库的当前形态；上游协议与目录会演进，故报告写明研究坐标并要求按坐标复核。
- 「无外部使用者」是概率前提；若本仓出现外部依赖者，库化结论应重估——报告已把触发条件写明。
- 本仓自己的 VT 核与 Herdr 的服务端渲染 surface 模式冲突，该冲突在报告中标为未知项，不在本次裁定。
- 报告建议的接缝重构属于未声明功能面，本次只记录不实施；实施前须补 `DESIGN.md` 声明。
