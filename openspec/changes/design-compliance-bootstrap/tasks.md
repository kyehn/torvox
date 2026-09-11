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
- [ ] 4.4 真实终端输入验证 `login` 登录与 `nix build`（应用域现状见下；需 Shizuku 桥接后续项）
- [ ] 4.5 推送到 `main`

## 5. 应用域实证结论（2026-09-11 设备端）

- 安装链全通：应用内安装器 `OK shell=bin/login installed=true`，脚本启动器被选中并经解释器直调启动（`SPAWN_SCRIPT`，单测 39/39）。
- 失败点唯一：`login` 内 `exec proot-static` 报 EACCES（`proot-static` 为 ET_EXEC，应用域无执行许可；`ptrace` 同样受限）。
  此前 `errno=26` 为误读：`126` 系 shell 惯例退出码（`wait` 日志已修正措辞，以 PTY 标记为准）。
- 上游无可直接下载的静态 PIE `login`/`proot`（Go 版 login 与 Android-libc proot 均不存在，PR490 仅为版本升级，均已实证）。
- `run-as` 域（即 shell 权限域）全链可用：`login`→`proot`→`login-inner` 已跑通，仅缺用户 profile。
- 结论：应用内可用 nix 须经 shell 权限域执行，即 Shizuku 桥接（DESIGN 已要求 Shizuku 开关，立为后续独立变更）。
