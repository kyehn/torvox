## MODIFIED Requirements

### Requirement: 清单建链

`SYMLINKS.txt` 缺失 MUST 失败；符号链接路径与目标 MUST 同受逃逸校验，绝对目标只允许落在规范 prefix 内；`EXECUTABLES.txt` 可执行项 MUST 赋予执行位。

绝对目标 MUST 重写到本机 prefix 后再建链：官方包 SYMLINKS.txt 里的旧式绝对路径
（`…/files/usr/…`）取该分段之后的后缀拼到本机 prefix，落盘的链接 MUST 指向重写后
的路径——校验与建链 MUST 用同一个路径，否则链接会指向归档里的旧 prefix 而断裂。

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
