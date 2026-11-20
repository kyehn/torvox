# Tasks

## 一、研究

- [x] 1.1 核验本仓扩展接缝：模块公开度、`Pty` trait 与本地 OS 能力混合、`Session::spawn_with_pty` 与
      `PtyMaster::new` 的 `pub(super)` 可见性、`Session::request_exit` 的本地进程组信号路径、
      输出推进与背压单槽、Kotlin 侧写入与渲染两条独立通道
- [x] 1.2 逐仓核验 `leizhiyuan/rexpocket`（`core`／`app` 分层、`RexClient`／`StreamDecoder`／
      `TerminalStream`／`PocketSession`／`SshTunnel`、控制面与数据面协议）
- [x] 1.3 逐仓核验 `shulhi/rexling`（`rexling-core` 模块清单、`transport.rs` 的 `CONTROL` 与 `TERMINAL`
      常量、`snapshot.rs` 的有界快照缓冲、Android 侧无 library 模块）
- [x] 1.4 逐仓核验 `nimrc/bellterm`（`native` → `herdr-client` → `herdr-protocol` 依赖方向、boot ID 与
      projection／surface revision fencing、无自动重连与 replay、README 明示服务端发送渲染后 cell grid）
- [x] 1.5 逐仓核验 `0cv/herdr-mobile-relay`（协议版本、动作收据与阶段、设备角色与动作分类、
      `Inbound` 的乐观并发字段、帧契约测试）
- [x] 1.6 逐仓核验 `kosumic/whip`（`AppCore`／`HostRuntime` 分层、控制流与字节流分离、世代隔离、
      快照与事件对账、协议 crate 独立）
- [x] 1.7 核验两个对标参照：Termux 三个 `com.android.library` 模块与 `maven-publish` 发布坐标；
      termlib 的 `lib/build.gradle.kts`（`org.connectbot:termlib` 发布、Dokka、metalava、kover）与
      `TerminalCallbacks` 的 `onKeyboardInput` 传输分工
- [x] 1.8 核验本仓库化障碍：`native/Cargo.toml` 缺发布元数据且依赖 git 源上游；
      `android/settings.gradle.kts` 只有 `:app`／`:benchmark`／`:baselineprofile`；无 `maven-publish`

## 二、报告

- [x] 2.1 写 `docs/research/extensibility-and-remote-backends-2026.md`：研究方法与坐标、本仓扩展接缝、
      三种形态评估、外部项目逐仓评估、两个参照、能力模型对照、成本收益矩阵、事实／推论／未知三分的结论、
      推荐执行顺序、不做清单、复现方式
- [x] 2.2 报告正文不写 git 提交号与被禁 API 名称，外部仓库精确提交号记入本变更的 `design.md`

## 三、台账登记

- [x] 3.1 `research-ledger` 索引表新增 `extensibility-and-remote-backends-2026.md` 行：实质、判定、依据
- [x] 3.2 计数口径：新增「在库报告的登记与计数」场景，把在库报告与已退役的 112 份分列，并写明索引完整仍是
      其日后删除的前置条件（需求正文长度已达工具阈值 500 字，故新增句落在场景而非正文）
- [x] 3.3 补齐行台账新增四行：会话传输抽象面（未声明不立项）、库化交付（不采纳，待产品决策）、
      外部项目可复用资产（仅参照，不引入代码）、Herdr 服务端渲染模式与本仓模拟器重复（仅参照）

## 四、验证

- [x] 4.1 `semgrep scan --error --config .semgrep/rules.yml`：0 findings
- [x] 4.2 `markdownlint-cli2 "**/*.md"`：0 issues
- [x] 4.3 `openspec validate --all --strict`：通过
