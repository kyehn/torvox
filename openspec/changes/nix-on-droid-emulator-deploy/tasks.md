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
- [ ] 3.6d `nixos-rebuild switch --flake /home/kudzu#default` 完成（含下载、顶层组装、激活；当前正在二进制替换阶段）。冻结事项：在此项关闭前不得再跑 GC——已抓取未激活的闭包路径无 root 保护，会被回收导致重下。
- [ ] 3.7 激活验证：`/nix/var/nix/profiles/system` 指向 kudzu 闭包；home-manager 落盘检查（只读 `run-as` 巡检，不执行新二进制）

## 4. 收尾（待 3.6–3.7）

- [ ] 4.1 将完整验证命令链记入部署 runbook（不提前写结局）
- [ ] 4.2 本 change 归档（`openspec/changes/archive/`）
