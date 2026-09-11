## 1. exec-bin trampoline（已完成）

- [x] 1.1 `resolve` 纯函数：`/nix/store` INTERP 由目标路径反推 prefix 并链式加载，
      其余 ELF 走系统 linker，`#!` 递归分发，`/system` 直调（`exec-bin/src/main.rs`）
- [x] 1.2 宿主单测：合成 ELF 夹具（nix-INTERP/普通 ELF/静态非 PIE/脚本/系统路径），
      9/9 通过（`cargo test -p exec-bin`，含既有集成测试）

## 2. Android 接线（已完成）

- [x] 2.1 `ExecBin.ensureInstalled`：`assets/bin/<abi>/exec-bin` 同步到
      `files/exec-bin`，`MainActivity.onCreate` 调用

## 3. 设备实验（待执行，需新 APK + fork bootstrap 安装）

- [ ] 3.1 安装：新 APK 安装 + `kyehn-bootstrap` 经应用内安装器安装
     （`bootstrap-install-offline.yml`，约 20min 解压）
- [ ] 3.2 E1 直接 exec：`$PREFIX/bin/login --dry-run; echo RC=$?`
- [ ] 3.3 E2 系统 linker：`linker64 $PREFIX/bin/login --dry-run; echo RC=$?`
- [ ] 3.4 E3 exec-bin：`files/exec-bin $PREFIX/bin/login --dry-run; echo RC=$?`
- [ ] 3.5 E4 完整 login：`files/exec-bin $PREFIX/bin/login`（记录 proot 步骤裁决）
- [ ] 3.6 E5/E6 exec-bin 起 bash 与 proot `--version`
- [ ] 3.7 E7 完整会话 + `nix build`（按 `--dry-run` 打印的 proot 命令行）
- [ ] 3.8 每步截图（debug overlay） + 转录对照，结论写入本文件

## 4. 收尾

- [ ] 4.1 删除临时 debug overlay，取证截图留档
- [ ] 4.2 全量门禁（cargo test / gradle unit / spotless / detekt）后提交推送
