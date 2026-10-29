# 引导归档校验和：以钉住的已知哈希为信任根

## Why

`BootstrapDownloader` 下载 bootstrap zip，随后 `BootstrapInstaller` 解压并**执行其中的 postinst 脚本**。当前唯一的完整性门控是「URL 必须是 https」加体积区间检查：

- https 只保证**传输**不被篡改，不保证**内容**可信。
- 引导 URL 是用户可编辑的设置项（`SettingsRepository.Keys.BOOTSTRAP_URL`），用户可把它指向任意 https 服务器——https 因此不构成信任根。
- 体积区间（`MIN_BOOTSTRAP_SIZE_BYTES`..`MAX_BOOTSTRAP_SIZE_BYTES`）挡不住任何东西：攻击者完全可以在合法体积内构造恶意归档。

也就是说：只要 URL 被改成攻击者控制的 https 地址，就能在本应用上下文内执行任意 postinst 代码。归档随后会被解压并**执行**，故校验和是唯一能证明确实拿到了声明版本的手段。

## What Changes

- 新增能力 `bootstrap-integrity`：归档 MUST 经校验和验证后才可解压执行；MUST NOT 把「URL 是 https」当作完整性依据；URL 可由用户编辑时 MUST 提示该风险。
- 校验和方案：**硬编码钉住版本的已知 SHA-256**，而非随归档发布的 `.sha256` sidecar。
  - 已知哈希仅在 `TermuxBootstrap.sha256(architecture)` 一处声明，与 `RELEASE`/`VARIANT` 同源；跟随版本时 MUST 同步更新，漏改任一架构即让该架构安装立即失败——这正是要的效果：宁可装不上，也不要装上未校验的归档。
  - 仅对默认钉住的预设 URL（`TermuxBootstrap.downloadUrl`）生效；自定义 URL 内容本仓无从预知，跳过比对但 MUST 记 warn 级日志，MUST NOT 静默降级、MUST NOT 假装已验证。
  - 采用内置哈希而非 sidecar 的原因：`PROHIBITED.md` 明确禁止「Bootstrap zip 的 sha256 sidecar 校验」；且内置哈希与 termux-app 自身 `downloadBootstraps` 任务的做法一致，避免了额外一次可被中间人篡改的网络请求（sidecar 本身也可能被投毒）。

## 实现

- `TermuxBootstrap` 提供 `sha256(architecture)`：aarch64 / x86_64 的已知 SHA-256，与 `RELEASE`/`VARIANT` 同源单一来源。
- `BootstrapDownloader`：对预设 URL 的归档，落盘途中用 `java.security.MessageDigest`（SHA-256）流式计算摘要，收尾与 `TermuxBootstrap.sha256` 比对；不匹配即删除暂存文件并返回失败（不做「无校验和就放行」的降级）。
- 校验和缺失或获取失败如实上报并中止，不隐藏错误（摘要本身不含凭据，可安全进入日志）；自定义 URL 分支显式记录 warn 日志，不与「已验证」混淆。

## Impact

- 受影响能力：bootstrap 安装（新增 `bootstrap-integrity` 规范）。
- 风险：钉住的哈希与预设 URL 必须一致；若发布侧更换版本而本仓哈希未同步，对应架构安装会失败——这是预期的保护行为，而非缺陷。

## 状态

- [x] 与上游确认权威哈希（termux-app `app/build.gradle` 的 `downloadBootstraps` 逐架构公布）
- [x] 实现下载侧流式校验（内置哈希，非 sidecar）
- [x] 补齐 `bootstrap-integrity` 规范（`openspec/specs/bootstrap-integrity`）
- [x] 单测：篡改字节 → 失败；自定义 URL → 记 warn 且放行
