# bootstrap-install Specification

## Purpose

bootstrap 经 staging 目录的原子安装行为：解压防护、清单建链、原子换入、postinst 执行。

## Requirements

### Requirement: staging 解压与滑移防护

安装 MUST 先确认 staging 区不存在；已存在时 MUST 拒绝执行，MUST NOT 清空、覆盖或复用。
现有 prefix 存活到原子换入为止；归档条目 MUST 拒绝绝对路径与 `..` 逃逸及落 staging 根自身；
解压设 1 GiB 总量上限与清单 1 MiB 上限，归档体积设 1 GiB 硬上限。

#### Scenario: staging 已存在时拒绝

- **WHEN** 安装开始前 `/data/data/com.termux/files/usr-staging` 已存在
- **THEN** 安装拒绝执行，不删除或覆盖该目录，也不触碰现有 prefix

#### Scenario: 失败保留旧 prefix

- **WHEN** 解压或建链失败
- **THEN** 丢弃本次创建的 staging 区，现有 prefix 保持可用

#### Scenario: 恶意条目被拒

- **WHEN** 归档含绝对路径或 `..` 逃逸条目
- **THEN** 安装失败并清理 staging，不写 prefix

### Requirement: 清单建链

`SYMLINKS.txt` 缺失 MUST 失败；符号链接路径与目标 MUST 同受逃逸校验，绝对目标只允许落在规范 prefix 内；`EXECUTABLES.txt` 可执行项 MUST 赋予执行位。

绝对目标 MUST 重写到本机 prefix 后再建链：官方包 SYMLINKS.txt 里的旧式绝对路径
（`…/files/usr/…`）取该分段之后的后缀拼到本机 prefix，落盘的链接 MUST 指向重写后
的路径——校验与建链 MUST 用同一个路径，否则链接会指向归档里的旧 prefix 而断裂。

不含 `files/usr/` 分段的绝对目标分两种：**沙箱根**路径（Nix 引导包的
`/nix/store/…`——引导环境把 prefix 的顶层目录绑成沙箱里的同名路径）MUST 把根
重新落到 prefix，判据是「引导包确实提供了那个顶层目录」；其余绝对目标 MUST
原样保留，由规范路径校验决定接受与否。

#### Scenario: 缺清单失败

- **WHEN** 归档无 `SYMLINKS.txt`
- **THEN** 安装失败

#### Scenario: 链接逃逸被拒

- **WHEN** 链接路径或目标逃出 staging 或规范 prefix
- **THEN** 安装失败并清理 staging

#### Scenario: 绝对目标重写到本机 prefix

- **WHEN** SYMLINKS.txt 的目标形如 `/data/data/com.termux/files/usr/bin/gawk`，
      而本机 prefix 在 `/data/user/10/com.termux/files/usr`
- **THEN** 链接 `bin/awk` 指向本机 prefix 下的 `bin/gawk`，不是归档里的旧路径

#### Scenario: 沙箱根目标落回 prefix

- **WHEN** SYMLINKS.txt 的目标形如 `/nix/store/…-bash/bin/sh`，且引导包提供了
      `nix/` 顶层目录
- **THEN** 链接指向本机 prefix 下的 `nix/store/…-bash/bin/sh`，MUST NOT 指向
      prefix 之外（那会让安装因「不安全的目标」失败——Nix 变种即装不上引导包）

#### Scenario: 未提供该顶层目录时保持原样

- **WHEN** SYMLINKS.txt 的目标形如 `/etc/hosts`，而引导包没有 `etc/` 顶层目录
- **THEN** 目标 MUST 原样保留，由规范路径校验拒绝（安装失败，而不是装出断链）

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

stderr 与 stdout MUST 由守护线程排空到 EOF（提前停止会让子进程卡在写满的管道上），
但保留量 MUST 有上限——诊断只取前几百字符，保留量却按子进程全部输出增长时，一个崩溃
后循环刷错误的脚本就能把应用进程读成 OOM。排空结果 MUST 以线程安全方式交接给等待方：
`join` 超时（孙进程仍持有管道）后仍会读取，非线程安全的容器不能跨线程使用。

#### Scenario: 缺目录视为成功

- **WHEN** `var/lib/dpkg/info` 不存在
- **THEN** postinst 阶段成功

#### Scenario: 目录列举失败视为失败

- **WHEN** `var/lib/dpkg/info` 存在但内容列举失败
- **THEN** postinst 阶段失败并上报，MUST NOT 按「没有脚本」静默成功

#### Scenario: 失败摘录上报

- **WHEN** postinst 失败
- **THEN** 返回前三段 stderr 摘录，状态为 ERROR

#### Scenario: 崩溃脚本刷爆 stderr 不 OOM

- **WHEN** 某个 postinst 向 stderr 写出远超上限的数据
- **THEN** 排空 MUST 仍读到 EOF 以让子进程退出，但保留量 MUST 不超过上限，
  MUST NOT 因诊断缓冲把进程读成 OOM

#### Scenario: 排空超时后安全交接

- **WHEN** 子进程已退出但孙进程仍持有管道，等待方 `join` 超时
- **THEN** 等待方读取已收到的前缀 MUST 是线程安全的，MUST NOT 出现数据竞争

### Requirement: 离线安装的中转包必须清理

离线安装（本地路径选包）MUST 把引导归档复制到应用数据目录下的中转位置，并 MUST 在复制
与安装**全过程**结束后删除该中转副本：复制中途失败留下的半截内容同样必须删除。

中转包落在用户数据树内，系统从不清理该目录，故残留即永久占用。

#### Scenario: 使用结束后中转包已删除

- **WHEN** 中转归档复制成功且安装流程结束（无论安装成败）
- **THEN** 中转位置 MUST NOT 留下任何文件（用例 `PreservedArchiveTest`）

#### Scenario: 复制失败不留残件

- **WHEN** 复制中转归档失败
- **THEN** 中转位置 MUST NOT 留下半截内容（用例 `PreservedArchiveTest`）
