## 1. 安装与检测（已完成，有据）

- [x] 1.1 Maestro 经"从文件安装"按钮完成离线安装（311MB，OCR 确认 headline + 进度）
- [x] 1.2 `isInstalled()`/`needsInstall()` 识别 store `bash-interactive`（`hasShellBinary`，单测正反例通过，14/14 green）
- [x] 1.3 修复版 APK dex 级验证含 `hasShellBinary`，`install -r` 保留数据升级

## 2. 执行死结取证（已完成，有据）

- [x] 2.1 `SPAWN_LINKER` + `spawn OK` 后子进程速死 → `fast-death respawn` 降级 `/system/bin/sh`
- [x] 2.2 直接执行动态 `nix` 与静态 `proot` 均 RC=126（模式位正常，DAC 排除，MAC 拒绝）
- [x] 2.3 `run-as` 上下文反例：proot banner + 翻译后 `nix 2.34.8` 正常（ptrace 可用，死结仅限应用进程上下文）
- [x] 2.4 结论记入长期规范（装载器死结段落，`c16f6f5`）

## 3. 部署通道（进行中）

- [x] 3.1 kudzu `flake.nix` 修复（显式 `x86_64-linux`），`nix flake show` 解析通过
- [x] 3.2 toplevel 闭包宿主侧构建成功（2.17GiB，含 `switch-to-configuration` 与 `nixos-rebuild`）
- [x] 3.3 flake 压缩包送达模拟器并解包至 proot 可见路径（`$PREFIX/home/kudzu`， guest `/home/kudzu`）
- [x] 3.4 取证：60s `connect-timeout` 解决 flake input 获取；`--option substitute true` 恢复二进制替换（77 路径/45.9MB，而非 2550 源码构建）
- [x] 3.5 设备 store GC 回收 306MB（首轮），预建 `files/usr/build`，删除已用暂存释放 311MB
- [x] 3.5b ENOSPC episode：替换关闭时 2550 源码构建耗尽磁盘，switch 死于 `stdenv-linux`（审查员预言证实）；GC 回收 2.7GiB（6716 死路径）后空闲 3.44G，闭包 2.17G 可容纳，离线扩容方案待命后撤销
- [x] 3.6a flake 属性修正（`#default`；nixos-rebuild-ng 会自加 `nixosConfigurations.` 前缀，显式全路径反被双重前缀）
- [x] 3.6b UID 映射（login-inner `setUser` 等价操作：guest `passwd`/`group` 内 65534→10215，`run-as id` 实测值）
- [x] 3.6c `id` shim（求值期 `builtins.exec ["id"]` 在 guest 内解析到不可执行的 `/system/bin/id`；以 store bash 为 shebang 的设备侧 shim 遮蔽之，`id -u`/`id -g` 均返回 10215 已验证； guest 内无现成 coreutils/busybox 可用）
- [x] 3.6d `switch-to-configuration switch` 完成离线激活（2026-09-08）：宿主构建 kudzu toplevel（2.1GiB）→ file 缓存推送 → 设备 `nix copy --from` 导入 → switch 激活 → `/nix/var/nix/profiles/system` 指向 kudzu 闭包（system-1-link）。`nixos-rebuild` 前端未跑（模拟器无外网，flake inputs 不可 fetch；求值留待有网环境）。教训：设备 `nix-collect-garbage` 误删 profiles 引用的 toplevel（437 路径/1.9GiB），switch 前不得再跑 GC。
- [x] 3.7 激活验证：`/nix/var/nix/profiles/system` 指向 kudzu 闭包 ✓；home-manager 落盘 ✓（`.nix-profile` 存在；switch 有非致命 home-manager 告警）。
- [x] 3.8 全静态 PIE 链（2026-09-08，nix-on-droid 9 提交）：`buildGoModule` 忽略 `buildFlags`（源码实证）→ `GOFLAGS` 经 preBuild 传 `-buildmode=pie`（login/login-inner 全静态 PIE 零依赖）；proot `-static`→`-static-pie`；`SYMLINKS.txt` 相对化（`realpath -s -m --relative-to`，绝对被安装器拒，错相对致 guest ELOOP）；`first_run` 禁用（离线必败且阻塞 shell）；`fallback_shell` 移顶层；`--guest` 显式分支（proot 直通 host 路径使启发式失效）；proot 失败 fallback 直接链；cwd 自动绑定；登录 shell 用 `/system/bin/sh`（glibc 链在 app 域无解）。
- [x] 3.9 干净按钮安装验证（2026-09-08）：`pm clear` 后从文件安装最终包 → shell 存活 → IME 输入 `echo` 回显 scrollback=11。
- [x] 3.10 模拟器联网（2026-09-09）：宿主 NAT 不转发（网关通、外网不通）→ 宿主 CONNECT 代理（10.0.2.2:18882，经网关 TCP 可达已验证）+ 设备 nix 经 `https_proxy` + `NIX_SSL_CERT_FILE`（store 内 nss bundle；bootstrap 自带 cert 路径在 guest 不可见）→ flake 拉取成功。另发现：终端降级 shell 被 SELinux 禁写（读+执行正常）。
- [x] 3.11 设备 nix-2.34 无 `builtins.exec`（实证 `hasAttr` 为 false）→ login 注出 `NIX_ON_DROID_UID/GID`，users-groups 改读 env（空回退 65534）；kudzu flake.lock 跟进 unstable（含修复 rev）；id shim（guest /bin/id → 10209）覆盖求值期调用。
- [x] 3.12 新 toplevel 离线激活（2026-09-09）：宿主构建（含修复 rev）→ file 缓存导入 → gcroot 保护 → `switch-to-configuration switch` → `profiles/system`（system-2-link）指向新闭包，home-manager 落盘。教训：toybox tar 随机丢小文件（narinfo/nar 须 diff 补齐）；store 只读属性清库前须 `chmod -R u+w`；DB 脏后宿主建空库推送比 repair 可靠。
- [ ] 3.13 `nixos-rebuild switch` 前端：求值/dry-build/换根构建全通；ng 须用 lock 版 rev（最新 rev 无 substitutes 被迫源码构建）；sandbox=false 时 HOME 须指不存在路径；当前唯一卡点是 5.8G 盘 ENOSPC（bootstrap 1.3 + toplevel 2.1 + ng 工具链增量放不下；已清 git 缓存仍差 ~1G）。待更大 data 分区后重跑，命令链见 runbook。

## 5. 生产代码清理（已完成）

- [x] 5.1 移除 SecondStageRunner.kt 中的 nix store 检测和 NIX 环境变量注入
- [x] 5.2 移除 TerminalRuntime.kt 中的 nix store PATH/ENV、前缀完成检查、findPrefixShell nix 检测
- [x] 5.3 移除 BootstrapInstaller.kt 中的 nix store bash-interactive 检测和 /nix/ 符号链接例外
- [x] 5.4 移除 pty.rs 中的 nix login --config 标志注入
- [x] 5.5 移除 SettingsScreen.kt 中的 nix-on-droid 前缀 shell 检测
- [x] 5.6 移除所有语言字符串资源中的 launch_location_status_nix
- [x] 5.7 清理 pty.rs、ffi.rs、text_utils.rs 中的 nix-on-droid 注释引用
- [x] 5.8 构建验证通过，APK 安装到模拟器并正常运行
- [x] 5.9 Git 提交推送（`6cb5058`）

## 6. 收尾（待 5 完成后重新评估）

- [x] 6.1 将完整验证命令链记入部署 runbook（`runbook.md`：构建/安装/shell/联网/部署判定标准、空间红线、硬顶清单，不写结局推论）
- [ ] 6.2 本 change 归档（`openspec/changes/archive/`）
