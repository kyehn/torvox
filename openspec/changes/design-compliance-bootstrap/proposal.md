# 变更：遵循设计基线并打通 nix-on-droid 启动

## Why

设计基线提交已原文合并到 `main`：`docs/specification/DESIGN.md` 为实际标准，新增最小体积、无回退、可删禁止实现等硬约束；旧生产代码（含安装标记、失败自动降级、禁止实现类设置项）与之冲突。同时主目标不变：nix-on-droid 经 bootstrap 正规安装后 `login` 成功登录且内含 `nix build` 可用，验证只认真实终端输入。本变更在遵循上游 nix-on-droid 包格式契约（`SYMLINKS.txt old←new`、`EXECUTABLES.txt` 权威列表、sh 脚本 `login` 调 `proot-static`）的前提下做通用实现，不加发行版特判。

## What Changes

- 安装器：删除安装标记全套（`VERSION_PIN_FILENAME`、`sha256Of`、`readVersionPin`、`writeVersionPin`、`needsInstall(sha)`）；`isInstalled` 只认启动入口 + `termux.env`；原子替换改为固定单备份轮转（装前删上上次备份，装后保留上一次由用户手动删）；权限只按包内 `EXECUTABLES.txt` 与 termux 同款目录位设置，不做硬编码特殊目录。
- 启动路径：启动入口按 `login`/`bash` 优先直查（ELF 或 `/system/bin/` shebang 启动脚本），失败不得自动回退，保留输出显示；用户主动的 Failsafe 入口保留（DESIGN 快捷菜单要求）。
- 删除禁止实现：光标闪烁开关/速度/样式、自定义主题支持、启动目录设置、横向平板面板固定；相关文档同步删除。
- 记录清理：本变更落地后归档 `nix-on-droid-emulator-deploy`（其内与新基线矛盾的旧结论不再作为依据）。

## Capabilities

### Modified Capabilities

- `shell/bootstrap-environment`：安装原子性（单备份轮转、无标记）、登录 shell 语义（无失败回退）。

## Impact

- 安装标记删除后重装判定只看启动入口与 `termux.env`；旧版写下的标记文件残留无影响（不再读取）。
- 失败自动降级删除后，坏 bootstrap 直接表现为会话退出与可见输出，不再静默切系统 shell。

## Open Questions

- Shizuku 集成开关、`fonts.xml` 缺失崩溃语义为独立后续项，不在本变更实施。

## Non-Goals

- 不重写 git 历史；历史清理仅指工作区记录文档。
- 不编译 LLVM/重型工具链；`proot-static` 只用上游预构建产物。
- torvox 路径 bootstrap 配方（`/tmp/nix-on-droid-torvox` 为一次性构建副本，不入库）：
  上游默认 `installationDir`/`user.home` 指向 `com.termux.nix` 且为只读选项，
  构建副本中直接改默认值、`build.initialBuild` 经 `mkForce` 关闭（离线首登不阻塞）、
  `bootstrap.nix` 对绝对 `SYMLINKS.txt` 目标做前缀相对化；另需
  `--option extra-substituters https://nix-on-droid.cachix.org`（及其公钥），
  否则会回退到本地编译 LLVM 工具链。
