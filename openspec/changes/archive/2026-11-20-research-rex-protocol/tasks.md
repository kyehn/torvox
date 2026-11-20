# Tasks

## 一、研究

- [x] 1.1 核验协议手册正文：端点与子协议、hello／welcome 与 API revision 协商、request／response／event 形状、
      响应 `seq` 与事件 `seq` 的合并规则、保序方法清单、错误码表、断线重连与事件丢失语义、服务端关闭码
- [x] 1.2 核验数据面：`data.connect` 票据、HELLO／RESUME／WELCOME／REJECT 四种定长二进制消息、流块长度与
      边界无语义、精确字节计数续传、数据面关闭码与 REJECT 原因码
- [x] 1.3 核验终端流内容：`GHOSTSNP` 快照开头、`resize`／`theme`／`child_exited` 三类 APC 消息、nonce 不匹配时
      原样透传、block 持有者与尺寸生效规则、`com.superlogical.terminal.write` 不保序
- [x] 1.4 核验快照格式与上游约束：记录头布局、READY／FINISH／PAGE／Kitty 放置的 tag、上游解码器为同步读取器且
      把暂时读不到数据当致命 EOF、PAGE 后需预读下一条记录头
- [x] 1.5 核验本仓接缝：`Pty` trait 混合的本地 OS 能力、`Session::spawn_with_pty` 与 `PtyMaster::new` 的
      `pub(super)` 可见性、`Session::request_exit` 的本地进程组信号路径、`PtyMaster` 的写入串行化、
      本仓上游 `libghostty-vt` 的快照解码面（本仓未消费）
- [x] 1.6 核验协议真值归属的先例：`rexling-core/jni` 的 JNI 面与 `rexling-android` 的 Kotlin 侧分工、
      `rexpocket` 把协议与流状态放 Kotlin 侧的反例
- [x] 1.7 核验端到端行为：`rexling-core/probe/src/main.rs` 的探针流程（attach → own_block → resize → write →
      数据面断点续传 → 控制面换连接续用与 token 轮换）

## 二、报告

- [x] 2.1 写 `docs/research/rex-protocol-support-2026.md`：研究方法与坐标、Rex 协议一手事实、本仓差异表、
      必要性裁定与前提、六阶段方法步骤、协议真值归属、事实／推论／未知三分、不做清单、复现方式
- [x] 2.2 报告正文不写 git 提交号与被禁 API 名称，外部仓库坐标用标签或快照标识

## 三、登记

- [x] 3.1 `research-ledger` 索引表新增 `rex-protocol-support-2026.md` 行：实质、判定、依据
- [x] 3.2 `research-ledger`「在库报告的登记与计数」场景补记在库两份
- [x] 3.3 `research-ledger` 补齐行台账新增六行：Rex 客户端形态、会话传输面接缝、上游快照解码面、
      WebSocket／TLS 依赖、外部项目可复用资产、客户端保存 reconnect token 与远端会话标识

## 四、验证

- [x] 4.1 `semgrep scan --config .semgrep/rules.yml` 对新增文档零命中
- [x] 4.2 `markdownlint-cli2` 对新增文档零告警
- [x] 4.3 `openspec validate --all --strict` 通过
- [x] 4.4 受保护文件零改动：`Cargo.toml`、`build.gradle.kts`、`settings.gradle.kts`、`docs/specification/`、
      `docs/STATUS.md`、`docs/persistence/`
