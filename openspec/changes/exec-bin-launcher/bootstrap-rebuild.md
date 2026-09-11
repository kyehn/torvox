# fork bootstrap 重建配方（路径已按 @01490c5 实体验签）

目标：复刻此前验证过的 torvox 路径 `bootstrap-x86_64.zip`（约 311M，
完整闭包，零 LLVM 编译），供应用内安装器安装后跑 exec-bin E 矩阵。

## 输入（已就位）

- 源码：`/tmp/kyehn-nix-on-droid`，`git checkout 01490c5`（本 loop 第 3 轮重克隆）。
- 实测存在：`overlays/bootstrap.nix`、`overlays/bootstrap-zip.nix`
  （zip 只是把 `bootstrap` 闭包打成 `bootstrap-<cpu>.zip`）、
  `overlays/session-login[-inner]/`（Go login）、`overlays/proot-termux/`。
- 选项锚点：`config.system.build.installationDir`
 （`modules/system/build.nix:16`，proot `-b` 绑定与 login 配置均由此派生）。

## torvox 定制（沿用已验证结论，逐项在构建副本中改，不入库）

1. `installationDir` 默认值 → `/data/data/com.termux/files`，
   `user.home` → 其下 `home`（上游默认指向 `com.termux.nix` 且为只读）。
2. 首登阻塞关：`build.initialBuild` 经 `mkForce` 关闭（离线首登不等待）。
3. `bootstrap.nix` 对绝对 `SYMLINKS.txt` 目标做前缀相对化（全相对 `←`）。
4. 构建全程加二进制缓存（2026-09-11 由 fork 源码
   `modules/config/nix.nix:179-190` 现查，禁止凭记忆填写）：
   `--option extra-substituters "https://cache.nixos.org
   https://seilunako.cachix.org https://nix-community.cachix.org"`
   `--option extra-trusted-public-keys "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY=
   seilunako.cachix.org-1:e/aJJI1S5hPY/BPeiVZcuPjt5ZjBRRo9dlYHmvwXPFM=
   nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="`，
   否则回退本地编译 LLVM 工具链。注意本 fork 用 seilunako +
   nix-community 缓存（非 nix-on-droid.cachix.org）。
5. 不重编 fork login（不用 static-PIE 替代）；不编译 proot（闭包内预构建
   musl static 已验证存在）。

## 出包后

- `adb push bootstrap-x86_64.zip /sdcard/Download/nix-bootstrap-x86_64.zip`，
  走应用内离线安装按钮（`bootstrap-install-offline.yml`），约 20min 解压；
  以 `files/usr/etc/termux/termux.env` 出现为准，不提前跑 E 矩阵。
