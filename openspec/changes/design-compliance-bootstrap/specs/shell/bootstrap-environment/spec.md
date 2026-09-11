## MODIFIED Requirements

### Requirement: 安装原子性

安装 bootstrap zip 时，系统 SHALL 解压到 staging 目录、按 `SYMLINKS.txt`（`old←new` 格式）创建符号链接，全部成功后原子重命名为正式 prefix；任一步失败 MUST 丢弃 staging 目录并保留旧 prefix，不得留下半安装状态。安装状态不得写入任何标记文件，只认启动入口与 `termux.env` 存在性。旧目录轮转 SHALL 为固定单备份：安装开始时删除已存在的上上次备份，旧 prefix 整体移入备份，成功后保留该备份由用户手动删除。

#### Scenario: 安装失败不破坏旧环境

- **WHEN** 解压/建链/重命名任一步失败
- **THEN** staging 目录被清理，旧 prefix 保持可用，安装报告失败

#### Scenario: 安装成功判定

- **WHEN** 安装完成
- **THEN** 启动入口与 `etc/termux/termux.env` 均存在，无任何标记文件参与判定

#### Scenario: 重装保留单备份

- **WHEN** 已有 prefix 上再次安装成功
- **THEN** 旧 prefix 整体位于固定备份目录，新 prefix 可用，不存在散列时间戳备份

### Requirement: 登录 shell 语义

`executable` 未指定时，系统 SHALL 按 `login`/`bash` 优先直查 `$PREFIX/bin` 中的可执行项（ELF 二进制，或首行 shebang 指向 `/system/bin/` 的启动脚本，后者经内核直接执行不走 linker 桥接），找不到时降级 `/system/bin/sh`。首行指向应用私有目录的脚本不计入（其解释器本身尚不可用）。启动入口本身执行失败时系统 MUST NOT 自动回退其他入口：输出保留显示，由用户确认关闭。

#### Scenario: 默认 shell 解析

- **WHEN** 用户未指定启动入口且 `$PREFIX/bin` 含 bash
- **THEN** 按 `login`/`bash` 顺序直查选中首个可执行项；`~/.bashrc` 生效，`~/.bash_profile` 不生效

#### Scenario: 脚本启动器可选中

- **WHEN** `$PREFIX/bin/login` 为 `/system/bin/` shebang 脚本
- **THEN** 其被选为启动入口并直接执行

#### Scenario: 启动失败不回退

- **WHEN** 选中的启动入口进程退出
- **THEN** 会话显示退出输出并等待用户确认，不自动改用其他入口重生
