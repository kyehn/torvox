## 1. 安装器合规（已完成）

- [x] 1.1 删除安装标记全套并收敛 `isInstalled`/`needsInstall`（单测同步删除标记用例）
- [x] 1.2 原子替换改为固定单备份轮转
- [x] 1.3 启动入口支持 ELF 与 `/system/bin/` shebang 脚本（单测正反例通过）

## 2. 启动路径合规（已完成）

- [x] 2.1 删除启动失败自动降级 respawn，保留输出显示（`FastDeathRecoveryTest` 同步精简）
- [x] 2.2 Failsafe 快捷入口保留（DESIGN 要求）

## 3. 禁止实现删除（已完成：光标/自定义主题；启动目录与横向面板经查无存量代码）

- [x] 3.1 光标闪烁开关/速度/样式（含 Rust JNI 出口，默认关闭；渲染内部状态机保留为固定默认值）
- [x] 3.2 自定义主题支持（含编辑器对话框、存储、字符串与相关测试）
- [x] 3.3 启动目录设置（经查无面向用户的启动目录设置项，仅内部工作目录 plumbing）
- [x] 3.4 横向/平板面板固定（经查无相关代码）

## 4. 记录与验证

- [x] 4.1 更新 `shell/bootstrap-environment` 规约（无标记、无回退、单备份轮转）
- [x] 4.2 归档 `nix-on-droid-emulator-deploy`
- [x] 4.3 单元测试与构建全绿（Rust 1005/0、Kotlin 定向套件、spotless、detekt）
- [x] 4.4 真实终端输入验证 `login` 登录与 `nix build`（Shizuku 桥接已实现并打通，见 §5）
- [x] 4.5 推送到 `main`

## 5. 应用域实证结论（2026-09-11 设备端，Shizuku 桥接打通）

- 安装链全通：应用内安装器 `OK shell=bin/login installed=true`（torvox 路径 bootstrap：`com.termux` 路径、`initialBuild=false`、SYMLINKS 相对化、profile 守卫 + `passwd/group` 合成， cachix 订阅、全程无编译）。
- 应用域直接 exec `proot-static` 仍被拒绝（EACCES/126，`untrusted_app` W^X，`files/` 与 `/data/local/tmp` 均不可执行；APK `lib/` 内二进制可执行，实证 `EXIT=139` 而非 126）。
- Shizuku 桥接（DESIGN 开关）：设置开关 + 启动 gate 对话框（关闭/退出）+ 授权检查；`Shell.Custom(files/shizuku-login.sh)` 经系统 `app_process` + `rish_shizuku.dex` 由 Shizuku 服务器（root）以 PTY 为 stdio 运行 prefix `login`。`Shizuku login bridge active` 日志为准。
- 真实终端输入验证（Maestro `inputText` + 会话转储回读，`adb shell` 仅作读写通道、未执行验证命令）：`nix --version` → `nix (Nix) 2.20.5`；hermetic `nix build`（`builtins.derivation` + `--option build-users-group '' --option sandbox false`）→ 输出路径并 `cat` 得 `ok`，构建器在设备端真实执行。
- 否决项：上游无 Go 版 login（零 `.go` 文件，`login` 为 shell 脚本）；PR490 仅版本升级，无 Android-libc proot 可下载物；`rish` 须走官方 `app_process + dex` 通道，直接 exec `librish.so`（ET_DYN）段错误。
