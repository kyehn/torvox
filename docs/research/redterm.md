# RedTerm 深度研究报告

对 [GlobalTechInfo/RedTerm](https://github.com/GlobalTechInfo/RedTerm) 的深度调研结论，目的是回答：本项目有什么可参考的。调研方式为克隆仓库并通读源码、文档、测试与 CI。

## 概况

基于 proot 的 Android Linux 发行版终端 + SSH 客户端（GPL-3.0）。单 `:app` 模块，133 个 Kotlin 文件，minSdk 24 / targetSdk 37。Android 12+ 起 app 私有存储 `noexec`，可执行物全部借 proot 的 `-L` loader 加载；proot 以 pinned `termux/proot` commit 静态交叉编译（`-z max-page-size=16384`，16 KB 页对齐并在构建后验证）；OpenSSH 以 per-ABI assets 内嵌，无第二 SSH 协议栈。

核心架构事实：**终端引擎不自研**。直接复用 Termux 上游库（jitpack `terminal-emulator`、`terminal-view` v0.118.3），`TerminalBackend` 仅做 `TerminalSessionClient`/`TerminalViewClient` 回调适配（Choreographer 合帧 invalidate、粘贴单线程 executor、正则链接检测、F1–F12 编码）。全部工程量集中在应用层：发行版注册/安装/更新、rootfs tar 提取、SFTP 浏览、密钥环、备份还原、会话录制。

## 测试与工程实践

- 约 35 个 JVM 单元测试：`ProotLaunch` 脚本生成纯函数化可测（交互终端与非交互命令走同一路径的启动器，并有 `quoteForShell` 的 POSIX 单引号转义专项测试）；rootfs/tar 提取的权限保留（setuid/setgid/sticky 位保留、`0555` 目录须后置 chmod、硬链接物化为独立副本）；`SftpSession` 配 `FakeSftpServer` 假对端协议测试；`TerminalPalette` 纯函数
- `lint` 以 `abortOnError = true` 作为构建门禁；CI 为 GitHub Actions 多工作流（debug 构建、release 签名、F-Droid 自建仓库发布）
- AGENTS.md 详实记录构建主机的能力与约束（单核 5.8 GiB、`chroot` 需 sudo、tmpfs 容量），并写明「verify rather than assume」——注释必须保持为真、会话机制不得假设发行版、持久化一切超出 Activity 生命周期的状态、密钥不得进日志
- CHANGELOG 记录了一类反复出现的根因教训：安装路径曾把所有目录权限 flatten 成 0755，导致 `/tmp` 丢失粘滞位、pacman 每次事务报警；修复方式是权限保真 + 删除事后修复步骤（它只能与错误自洽）
- 前台服务 `TerminalService`：wake lock 用短超时（5 分钟）由 1 秒 tick 持续 re-arm，另对 `isHeld != true` 主动重取（部分 OEM 内核会在内存压力下释放 partial wake lock）；通知栏聚合 `/proc/<pid>/stat` 的会话 CPU%；通知只报会话数量而不猜测显示名——「唯一诚实的短答案是数量」

## 与本项目的路线对照

| 维度 | RedTerm | 本项目 |
| --- | --- | --- |
| 终端引擎 | 复用 Termux 库（Java/View 体系） | 完全自研（Rust + libghostty-vt + wgpu） |
| 应用层 | 厚：发行版管理/SSH/SFTP/文件管理器 | 薄：bootstrap 安装 + 终端 UI |
| 进程模型 | proot 进发行版，noexec 用 `-L` loader 绕过 | PTY 直起 `/system/bin/sh` 或 prefix 内 shell |
| VT/渲染 | Kotlin 持网格，View invalidate | Rust 持网格，wgpu 直绘 ANativeWindow |

两项目功能域几乎不重叠：内嵌 proot 在本项目的禁止清单中，bootstrap 仅安装 Termux prefix（zip + `SYMLINKS.txt`/`EXECUTABLES.txt` 清单，权限由清单承担），SSH/SFTP 均在范围外。因此 RedTerm 的主体实现对本项目没有直接移植对象，参考价值集中在**外围工程实践与防错模式**。

## 可参考点

1. **wake lock 自检重取**：本项目 `TerminalForegroundService` 已有半程续期（`scheduleWakeLockRenewal`），但缺 RedTerm 的第二重保险——续期时若 `isHeld != true` 立即重取。OEM 内核内存压力下 partial wake lock 会被静默释放，症状是「屏幕关闭后会话随机停滞」。一行式自检即可加固，成本低。
2. **启动路径单一事实源**：RedTerm 的 `ProotLaunch` 让交互终端与非交互命令共用同一个启动器脚本生成函数，杜绝「终端里能跑、App 调用就失败」（bind 缺失、PATH 不同、环境修复未 source）。本项目的会话启动（`session.rs`、`shell_env.rs`）与 second stage 环境装配应对照自查是否存在两套并行的环境拼装路径。
3. **假对端测试模式**：`FakeSftpServer` 让协议解析在 JVM 单测里对假服务器跑完整握手。本项目 `mock_pty.rs` 已是同思路（mock PTY 侧），该模式值得在更多边界测试延续——凡「对端行为」依赖处，用假对端固化协议语义。
4. **错误注释纪律**：AGENTS.md 的「注释必须保持为真」「分支不得依赖显示字符串」两条规定，与本项目注释极简原则同源，可作为评审口径。

## 不适用项（明确排除，防误参考）

- **`script` 会话录制**：RedTerm 用 `script -q -a -f` 挂 pty 录制 ANSI 流，是对「终端库无 raw output hook」的无奈替代。本项目 Rust 侧天然持有 PTY 读端，不存在该缺口，且录制功能规格未要求。
- **AppLog 持久化日志**：DESIGN 明确「不将日志写入文件，不保存日志」，logcat 可见即可。
- **259 项调色板纯函数**：本项目主题来自 alacritty-theme 完整主题文件，16 色由主题自带，无需插值生成。
- **SessionDescriptor 身份与显示名分离**：本项目会话归属 Rust 侧，session-list 规格已定为「序号 + 标题」，Kotlin 侧无会话身份分支需求。
- **Choreographer 合帧 invalidate / 链接正则检测**：渲染与链接识别均在 Rust/libghostty-vt 侧完成。
- **发行版管理、proot、内嵌 OpenSSH/SFTP、文件管理器、备份还原**：均在范围外或禁止清单内。
- **CPU% 通知、快捷设置磁贴、桌面小部件**：DESIGN 通知栏要求仅「会话数量 + 退出按钮」。

## 已核实无缺口的项

- **16 KB 页对齐**：RedTerm 显式 `-z max-page-size=16384` 并构建后验证；本项目 `libnative.so` 经 readelf 核实 LOAD 段对齐已为 `0x4000`（16384），满足 Android 15+ 要求，无需改动。
- **通知诚实性**：本项目通知仅报会话数量与退出按钮，不猜显示名，已与 RedTerm 修后的行为一致。
- **bootstrap 权限保真**：本项目走 Termux 上游 zip 格式（`SYMLINKS.txt` + `EXECUTABLES.txt` 清单），权限由清单承担，无 tar 模式位保真问题；zip 滑移防护、`SYMLINKS.txt` 有界读取、失败 staging 清理均已具备。

## 行动建议

仅一项落地：为 `TerminalForegroundService` 的 wake lock 续期路径补充 `isHeld` 自检重取（对应上文可参考点 1）；其余参考点属评审口径，不产生代码变更。不引入任何 RedTerm 功能域组件。
