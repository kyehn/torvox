# kudzu 模拟器部署 runbook（命令链与判定标准）

> 只记录可复现的命令与通过标准，不写结局推论。每条跑完对照判定，失败停在当条。

## 0. 前置（宿主）

```sh
adb devices  # 须见 emulator-5554 device
git -C /tmp/nix-on-droid log --oneline -1  # 确认修复 rev
```

## 1. 构建 bootstrap

```sh
cd /tmp/nix-on-droid
nix build --impure --print-out-paths .#bootstrap-zip
```

判定：`readelf -h bin/login | grep Type` 为 `DYN`；`readelf -d bin/login | grep NEEDED` 无输出；
`SYMLINKS.txt` 中 `grep -c '^/'` 为 0；awk 抽查无 `../` 逃出 link 目录深度。

## 2. 按钮安装

```sh
adb push bootstrap.zip /sdcard/Download/nix-bootstrap.zip
# 应用内：drawer → Settings → 滚到底 → OfflineInstallButton → Downloads → 选包
```

判定：`run-as com.termux ls files/usr/bin/login` 存在；`usr-staging` 消失。

## 3. shell 存活与交互

```sh
adb shell am force-stop com.termux
adb shell am start -n com.termux/terminal.emulator.MainActivity
adb shell "ps -A -o PID,PPID,NAME" | grep ' sh$'   # 子进程存活
adb shell input text 'echo ALIVE_<date +%s>'; sleep 1; adb shell input keyevent 66
adb logcat -d | grep scrollback | tail -1  # scrollback>0
```

## 4. 联网（NAT 不转发时走代理）

```sh
adb shell "ping -c1 -W3 10.0.2.2"                       # 网关须通
echo OK > /tmp/probe.txt && python3 -m http.server 18881 --directory /tmp &
adb shell "echo 'GET /probe.txt HTTP/1.0' | nc -w 8 10.0.2.2 18881"  # 须见 PROBE_OK
python3 /tmp/connect-proxy.py &                          # CONNECT 代理 :18882
curl -x http://127.0.0.1:18882 https://api.github.com  # 须 200
```

设备 nix 环境（每次 run-as/proot 调用前 export）：
`PROOT_TMP_DIR=$PREFIX/tmp HOME=$PREFIX/../home USER=nix-on-droid`
`https_proxy=http://10.0.2.2:18882 http_proxy=...`
`NIX_SSL_CERT_FILE=<store 内 nss bundle>`（bootstrap 自带 cert 在 guest 不可见）
`NIX_ON_DROID_UID/GID=$(run-as id 实测值)`，`TMPDIR=/tmp`（guest 可见值，勿用 host 路径）。

判定：`nix flake metadata github:nix-community/nix-on-droid/master` 成功 resolve。

## 5. 部署 kudzu

```sh
# 宿主构建 + file 缓存 + 推送（495M 级）
nix build --impure '.#nixosConfigurations.default.config.system.build.toplevel'
nix copy --to file:///tmp/kudzu-cache /tmp/kudzu-toplevel
# 设备：copy → gcroot → switch-to-configuration switch → profiles/system 指向闭包
```

判定：`readlink profiles/system` 指向 kudzu toplevel；`ls home/.nix-profile` 存在。

## 6. 空间红线（5.8G 盘）

- switch 前 `df` 留 ≥2.5G；file 缓存 copy 完即删。
- `nix-collect-garbage` 会删 profiles 引用的 toplevel——跑前先给闭包建 `var/nix/gcroots/` 显式链接。
- store 路径只读：清库前 `chmod -R u+w store`。
- toybox tar 随机丢小文件：传完后 `comm` 对比宿主/设备文件列表，缺失打包补传。
- narinfo/nar 缺失导致 file copy 报 `no substituter`：先补文件再 copy。

## 7. 已知硬顶（莫再试，按序）

- `nixos-rebuild switch` 前端全量构建：5.8G 盘装不下（bootstrap 1.3 + toplevel 2.1 + 工具链增量），须更大 data 分区或精简闭包（精简≈阉割 home-manager，不建议）。
- ng 须用 lock 版 rev（`github:NixOS/nixpkgs/<lock-rev>#nixos-rebuild-ng`）：最新 rev 无 substitutes，会触发 Python 等源码构建。
- sandbox=false 时 `HOME` 须指不存在路径（否则 nix 报 homeless-shelter 纯度错误）。
- 求值另需 guest `/bin/id` shim（输出设备 UID）+ `/bin` 绑定在 PATH 前。
- 设备 nix-2.34 无 `builtins.exec`：求值靠 `NIX_ON_DROID_UID/GID` env（login 注出）。
- 终端降级 shell 被 SELinux 禁写：读+执行正常，写操作走 run-as。
