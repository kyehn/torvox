## 1. exec-bin trampoline（已完成）

- [x] 1.1 `resolve` 纯函数：`/nix/store` INTERP 由目标路径反推 prefix 并链式加载，
      其余 ELF 走系统 linker，`#!` 递归分发，`/system` 直调（`exec-bin/src/main.rs`）
- [x] 1.2 宿主单测：合成 ELF 夹具（nix-INTERP/普通 ELF/静态非 PIE/脚本/系统路径），
      9/9 通过（`cargo test -p exec-bin`，含既有集成测试）

## 2. Android 接线（已完成）

- [x] 2.1 `ExecBin.ensureInstalled`：`assets/bin/<abi>/exec-bin` 同步到
      `files/exec-bin`，`MainActivity.onCreate` 调用

## 3. 设备实验（待执行，需重新配给：模拟器已被擦除）

- [ ] 3.0 环境重建（2026-09-11 17:5x UTC 实测：`run-as: unknown package:
  com.termux`，`pm list packages` 无 termux/shizuku，`/tmp` 本地产物亦被清空——
  为整机擦除，非单个卸载；`b740e69d7` 轮询永不到达，作废）：重装 APK
  （`android/app/build/outputs/apk/debug/app-debug.apk`，含 overlay + ExecBin）、
  重推 bootstrap zip（本地副本已失，需由 fork 重新出包或由设备端
  `/sdcard/Download` 残留确认——经查亦随擦除消失，按前者）、应用内安装器安装。
  Shizuku 保持未安装（本变更禁 Shizuku，E 矩阵只读应用域裁决）。

- [x] 3.0a 配给恢复：fork 已按 `bootstrap-rebuild.md` 重建——零改动源码、
      全程缓存命中（`seilunako.cachix.org` 264.5MiB，`--max-jobs 0`
      守住零 LLVM），`bin/login`（动态 glibc 9.5M）+ `bin/proot.new`
      （静态 musl）验包无误，已推送 `/sdcard/Download`；新 APK 已重装
      并经 `pm list` 确认 `package:com.termux`。
- [ ] 3.1 安装：`kyehn-bootstrap` 经应用内安装器安装
     （`bootstrap-install-offline.yml`，约 20min 解压）。尝试 1 未达安装：
     新机首启遭遇 `System UI isn't responding` ANR，对话框遮挡致
     `TerminalScreen` 断言失败；logcat 无应用 FATAL（非应用崩溃，系整机
     负载瞬态）。副产品：失败截图证明 debug overlay 可被 screencap 捕获
     （绿色 `loading...` 字样清晰）——截图取证链成立。已加
     `dismiss-anr.yml`（已提交），待其完成后重跑安装流。尝试 2 安装中：
     直接重跑仍因 Recent 空列表失败，截图证实 search-tap 误中历史建议行
     （文件行未渲染——MediaStore 未就绪）；遂加 MediaStore 就位门
     （`content query` 轮询，20s 确认索引），重跑 search-tap 命中文件行，
     `PickerActionHandler: onFinished(msf:1000000018)`（18:59:38）为凭，
     应用已返回设置页。截图另证 `Shizuku integration` 开关为 OFF
     （无 Shizuku 约束天然成立），MCP 亦 OFF。解压在 drain-fix APK 下运行中。
- [x] 3.2 E1 直接 exec：`$U/login --dry-run` → `E1-RC=126`（app_data_file
      直接 exec 禁止，预期内；overlay `sb=0 run=true` 为 failsafe 新会话）
- [x] 3.3 E2 系统 linker：`linker64 $U/login --dry-run` → `E2-RC=1`，login
      真机跑起（dry-run 打印 `rename .../proot.new to .../proot`），随后
      `main.go:150 failed to find target .../login-inner: no such file` fatal。
      证实 bionic 可直接加载 Go login（Go 二进制自带 PT_PHDR）。
- [x] 3.4 E3 exec-bin：`linker64 $E $U/login --dry-run` → `E3=134`
     （`Could not find a PHDR: broken executable?` + Aborted）。exec-bin
      自身为正常 NDK PIE（含 PT_PHDR，INTERP=/system/bin/linker64），故该
      abort 系 exec-bin 按既有设计经 bionic 去加载 prefix glibc `ld.so`
      所致（glibc ld.so 无 PT_PHDR，bionic 拒绝）。E3 截图 overlay 残留
      的 login-inner 行系 E2 输出，E3 本身未抵达 login。
- [x] 3.5 E4 完整 login：同因 `134` abort，会话死亡（`[Process completed
      (code 134)]` ×3 见 log），其后 `exit`/`E4-RC`/E5/E6 输入均进入死会话，
      overlay 停滞于 E2/E3 旧行。E4-RC/E5/E6 截图仅证明会话已死，无新裁决。
- [x] 3.6 E5/E6 exec-bin 起 bash 与 proot `--version`：因会话已死而无效，
      待新 failsafe 会话 + exec-bin 路由修复后重跑。
- [ ] 3.7 E7 完整会话 + `nix build`：前置两项——(a) exec-bin 路由修复
      （NixInterp 分支改经 system linker 直接加载目标，即 E2 实证路径，
      不再经 bionic 加载 glibc ld.so）；(b) 新 failsafe 会话中真实终端
      `ls` 确认 `bin/login-inner` vs `bin/login-inner.new` 形态（fork
      `default.nix:40` 的 pending_artifacts 含 login-inner，dry-run 却只
      打印 proot rename → 设备端极可能缺 `login-inner.new`，届时以前置
      `mv` 补齐再跑 login→proot→nix build）。adb 永不执行验证命令。
- [ ] 3.8 每步截图（debug overlay） + 转录对照，结论写入本文件

## 4. 收尾

- [ ] 4.1 删除临时 debug overlay，取证截图留档
- [ ] 4.2 全量门禁（cargo test / gradle unit / spotless / detekt）后提交推送
