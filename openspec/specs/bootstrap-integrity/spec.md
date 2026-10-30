# bootstrap-integrity Specification

## Purpose

引导归档（bootstrap zip）在解压并执行其 postinst 脚本之前 MUST 经校验和验证。本能力确立「https 与体积区间不是完整性依据、校验和才是信任根」的原则，并明确规定仅对默认钉住的预设 URL 做校验、自定义 URL 必须明示未校验。

## Requirements

### Requirement: 钉住预设归档的校验和验证

下载器对**默认钉住的预设 URL**（`TermuxBootstrap.downloadUrl`）返回的归档 MUST 在落盘途中流式计算 SHA-256，并在收尾与 `TermuxBootstrap.sha256` 的已知值比对；不匹配 MUST 删除已暂存文件并返回失败，MUST NOT 继续交给安装器。

#### Scenario: 预设归档校验通过

- **WHEN** 下载默认预设 URL 且字节流 SHA-256 等于 `TermuxBootstrap.sha256(architecture)`
- **THEN** 返回成功，归档进入安装流程

#### Scenario: 预设归档校验失败

- **WHEN** 下载默认预设 URL 但字节流 SHA-256 不等于预期值（例如传输损坏或内容被替换）
- **THEN** 暂存文件被删除，下载返回失败，错误信息包含 `checksum mismatch` 且不暴露完整 URL

#### Scenario: 自定义 URL 不做静默降级

- **WHEN** 下载 URL 不等于钉住预设（用户在设置中编辑了引导 URL）
- **THEN** 跳过校验和比对，但 MUST 记录一条 warn 级日志说明 `bootstrap URL is not the pinned preset; checksum not verified`，MUST NOT 假装已验证

### Requirement: 校验和常量的单一来源与同步约束

已知校验和 MUST 仅在 `TermuxBootstrap.sha256` 一处声明，且 MUST 与 `RELEASE`/`VARIANT` 同版本；跟随版本时 MUST 同时更新 `RELEASE` 与全部架构的哈希，使只更新其一会导致安装立即失败——这是预期行为。

#### Scenario: 版本与哈希同步缺失即失败

- **WHEN** 仅更新 `RELEASE` 而未更新某架构哈希
- **THEN** 该架构的下载因校验和不匹配而失败（宁可装不上也不装未校验归档）

### Requirement: 完整性门控的正确表述

代码注释与文档 MUST NOT 将「URL 是 https」描述为完整性门控；https 仅防传输链路篡改，不保证内容可信（引导 URL 可由用户编辑）。

#### Scenario: 注释不谎称 https 为完整性依据

- **WHEN** 审阅 `BootstrapDownloader` 注释
- **THEN** 不出现把 https 检查称为「完整性门控」的表述；实际完整性门控为校验和

### Requirement: 校验和不得改为随归档发布的独立文件

本仓 MUST NOT 另外网络获取或读取随归档发布的独立校验和文件（`PROHIBITED.md` 已禁止），
MUST 采用内置钉住版本的已知哈希。

理由：独立校验和文件与被校验的归档走同一条网络路径，攻击者替换归档时一并替换它即可，
校验因此形同虚设；内置常量是唯一不随下载内容漂移的信任根。

#### Scenario: 校验来源为内置常量

- **WHEN** 审阅 `TermuxBootstrap` 与 `BootstrapDownloader`
- **THEN** 预期哈希来自 `TermuxBootstrap.sha256` 这一代码常量，不存在任何 `.sha256`
      文件的网络获取或读取逻辑
