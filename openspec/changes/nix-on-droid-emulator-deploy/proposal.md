## Why

nix-on-droid bootstrap 在 torvox 终端内不可执行（API 35 Enforcing 应用上下文：静态二进制无解释器可借道，glibc 二进制无兼容装载器；设备端三路径证伪：linker 间接速死、`fast-death respawn` 降级、直接执行 RC=126）。但 `run-as` 上下文可执行（SELinux 域不同）：proot + 翻译后 nix 2.34.8 实测可用。kudzu（`kyehn/kudzu`）的模拟器部署因此走 `run-as`+proot 通道，而非终端 shell。

## What Changes

- torvox：`isInstalled()`/`needsInstall()` 识别 store `bash-interactive` 布局（已合入，单测 14/14）；Maestro 离线安装流 + shell 探针流（已合入）；长期规范 `shell/bootstrap-environment` 记录装载器死结与证据（已合入）。
- kudzu：`flake.nix` 以显式 `x86_64-linux` 替代已删除的 `builtins.currentSystem`（已合入；`nix flake show` 解析通过，toplevel 闭包宿主侧构建成功）。
- 本机（模拟器）：`nixos-rebuild switch --flake /home/kudzu#default` 经 `run-as`+proot 执行（进行中）。

## Capabilities

### New Capabilities

- 模拟器内 `run-as`+proot 执行通道（proot 翻译后 nix 可用性的首次验证）。

### Modified Capabilities

- `shell/bootstrap-environment`：nix 执行方式需求加入装载器死结结论与三路取证。

## Impact

- 生产代码变更仅 torvox 检测谓词 + 单测；kudzu 仅 flake 一行；模拟器侧为数据文件（bootstrap、flake 压缩包），均可重建。
- 不碰终端渲染管线、不碰 shell 生成路径、不改任何已验证行为。

## Open Questions

- 激活（activation）阶段在 `run-as` 用户下的行为（profile 链接、home-manager 落盘）待 switch 完成给出证据。
- `NIX_PATH` 指向不存在目录的 nit：flakes 路径不受影响，暂缓。
