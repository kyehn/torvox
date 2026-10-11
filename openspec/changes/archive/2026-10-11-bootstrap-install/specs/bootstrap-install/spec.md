# bootstrap-install Specification

## Purpose

bootstrap 经 staging 目录的原子安装行为：解压防护、清单建链、原子换入、postinst 执行。

## ADDED Requirements

### Requirement: staging 解压与滑移防护

安装 MUST 先清空 staging 区再解压，现有 prefix 存活到原子换入为止；归档条目 MUST 拒绝绝对路径与 `..` 逃逸及落 staging 根自身；解压设 1 GiB 总量上限与清单 1 MiB 上限，归档体积设 1 GiB 硬上限。

#### Scenario: 失败保留旧 prefix

- **WHEN** 解压或建链失败
- **THEN** 丢弃 staging 区，现有 prefix 保持可用

#### Scenario: 恶意条目被拒

- **WHEN** 归档含绝对路径或 `..` 逃逸条目
- **THEN** 安装失败并清理 staging，不写 prefix

### Requirement: 清单建链

`SYMLINKS.txt` 缺失 MUST 失败；符号链接路径与目标 MUST 同受逃逸校验，绝对目标只允许落在规范 prefix 内；`EXECUTABLES.txt` 可执行项 MUST 赋予执行位。

#### Scenario: 缺清单失败

- **WHEN** 归档无 `SYMLINKS.txt`
- **THEN** 安装失败

#### Scenario: 链接逃逸被拒

- **WHEN** 链接路径或目标逃出 staging 或规范 prefix
- **THEN** 安装失败并清理 staging

### Requirement: 原子换入与回滚

换入 MUST 经同文件系统 rename；既有 prefix MUST 先移入随机后缀备份再换入，失败 MUST 恢复备份；成功后备份保留由用户手动删除，从不自动删除；不写任何安装标记，状态只认启动入口存在性。

#### Scenario: 旧目录备份保留

- **WHEN** 安装成功且既有 prefix 存在
- **THEN** 旧目录以随机后缀保留，新目录就位

#### Scenario: 换入失败回滚

- **WHEN** staging 换入失败
- **THEN** 备份恢复为 prefix，旧引导仍可用

### Requirement: postinst 幂等执行

换入后 MUST 运行 `var/lib/dpkg/info` 下 postinst（锁防重叠、陈旧锁删除重试、EEXIST 视为他者持有不报成功）；缺目录 MUST 视为成功；失败取前三段 stderr 摘录上报。

#### Scenario: 缺目录视为成功

- **WHEN** `var/lib/dpkg/info` 不存在
- **THEN** postinst 阶段成功

#### Scenario: 失败摘录上报

- **WHEN** postinst 失败
- **THEN** 返回前三段 stderr 摘录，状态为 ERROR
