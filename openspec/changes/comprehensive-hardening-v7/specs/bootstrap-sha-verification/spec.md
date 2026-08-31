## Purpose

为 Bootstrap 安装增加可选的 sha256 sidecar 校验， best-effort 防止篡改或损坏的 zip 污染用户数据，同时不阻断 Termux 预设离线安装。

## ADDED Requirements

### Requirement: Sidecar 校验

系统 SHALL 在 `BootstrapDownloader` 下载完成后，若同 URL 存在 `.sha256` 文件则执行校验；校验失败 SHALL 删除 staging 目录并重试一次，仍失败则报告错误且不安装。

#### Scenario: 校验通过

- **WHEN** 下载 `https://example.com/bootstrap.zip` 且 `bootstrap.zip.sha256` 存在且匹配
- **THEN** 安装继续，校验痕迹写入安装日志

#### Scenario: 校验失败重试

- **WHEN** 首次校验不匹配
- **THEN** 系统删除 staging，重新下载并再次校验，若仍失败则终止并提示“校验失败”

#### Scenario: 无 sidecar 时直接安装

- **WHEN** URL 无 `.sha256`（如 Termux 预设离线包）
- **THEN** 系统跳过校验直接安装，不阻塞

### Requirement: 原子性与可观测性

系统 SHALL 保持 Bootstrap 的原子 staging 语义，校验过程不污染 `/data/data/com.termux/files`，且耗时与结果可通过 `BootstrapProgress` 观察。

#### Scenario: 校验耗时可追踪

- **WHEN** 开启 Bootstrap 安装
- **THEN** `BootstrapProgress` 阶段包含 `Verifying`，且校验失败时状态为 `Failed(ChecksumMismatch)`
