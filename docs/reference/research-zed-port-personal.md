# 个人亲自研究：zed-android-port（增量核验篇）

> 定位：**增量篇**。主文档 `research-zed-port.md`（761 行）已覆盖整体架构/env overlay/超链接/鼠标 SGR/工作区——本文件只记录主文档之外的**亲自核验增量**与 torvox 落地结论；重复内容不在此复述。
> 亲自精读：`crates/util/src/env.rs`（全 98 行）、`crates/zdroid_runtime/src/adapters/bootstrap.rs`（319-455）、`crates/terminal/src/terminal.rs`（100-180 / 2270-2300 / 3437-3460）、`crates/terminal/src/pty_info.rs`（全 221 行）、`crates/terminal/src/terminal_hyperlinks.rs`（1-330）、`crates/zdroid_runtime/src/port.rs`（trait 63-250）。
> 仓库：https://github.com/GeneralKaos666/zed-android-port（快照行号）

---

## 1. kill_active_task 精确序列（torvox send_signal 参照）

实测（terminal.rs:2276-2288 + pty_info.rs:144-164）：

1. 前台组：`ProcessIdGetter::pid()`（pty_info.rs:38-52，:42 `tcgetpgrp(fd)`，0→回退 `pty.child().id()`）
2. `info.kill_current_process()`（pty_info.rs:144-149）：`killpg(前台组pid, SIGKILL)` 杀前台命令
3. `info.kill_child_process()`（pty_info.rs:156-158）：sysinfo `Process::kill()` **单进程 SIGKILL** 杀 shell

> **注意**：第二段不是 SIGTERM——SIGTERM 组杀在独立函数 `terminate_child_process`（pty_info.rs:160-164），仅 Drop 流程使用（主文档 research-zed-port.md:166 已述）。torvox `session.rs::send_signal`（:470-477）只 `kill(pid)` 单进程，**无前台组概念**——MCP `send_signal` 对运行中任务（sleep/less）收效有限；增强方向照抄 1+2。

- 测试：`terminal.rs:3437-3458`（`test_kill_active_task_completes_and_captures_output`，`echo; sleep 60` 后 `kill_active_task` 断言完成+输出捕获）——torvox pty.rs 可移植为 killpg 测试。

## 2. env overlay 应用顺序（pty.rs build_env 参考）

`insert_zed_terminal_env`（terminal.rs:123-161）：

1. android cfg 先抄白名单 `["HOME","PATH","SHELL","TMPDIR","LANG"]` 用 `entry().or_insert`（:145-149）
2. 再套 overlay：`EnvOp::Set` insert 覆盖 / `EnvOp::Remove` remove（:150-159）

torvox `pty.rs::build_env`（:676-715）现状：`base_env`（TERM/COLORTERM/TERM_PROGRAM/TERM_PROGRAM_VERSION/LANG/PREFIX/TMPDIR）+ LD_PRELOAD（:679-688，仅 $PREFIX）+ HOME/USER/SHELL/PATH/PWD/LINES/COLUMNS + env.extra 去重。**无 TERMUX__\*、无 SSL_CERT_FILE/CURL_CA_BUNDLE** → P0 缺口。

## 3. bootstrap.rs env_for_terminal 精确键表（:386-434）

- PREFIX(398)、TERMUX__ROOTFS(399)、TERMUX__PREFIX(400)、TERMUX__HOME(401)、TERMUX_APP__PACKAGE_NAME(402)、HOME=$termux_home(408)、LD_PRELOAD=/data/data/com.zdroid/files/usr/lib/libtermux-exec.so(414-416)；`cert_path.is_file()`（:419）才 push SSL_CERT_FILE(424-427)+CURL_CA_BUNDLE(429-431)。
- 注：`env_for_zed_process`（:319-383）走 PATH 前置 `.zed/bin:$PREFIX/bin`；终端走 env_for_terminal。

## 4. hyperlinks tab/超时细节（UrlDetector.kt 参考）

- `path_match`（terminal_hyperlinks.rs:252-330）：**手写逐 cell 构建 line，不压缩 tab 为一个空格**（:276-278 明确注释 bounds_to_string 会压缩），跳过 WIDE_CHAR_SPACERS；`hovered_point_byte_offset` 逐 cell 累加 → tor 的 UrlDetector 处理 CJK/tab 偏移参照。
- 超时护栏 `path_hyperlink_timeout`（:267-281）：正则匹配超过 ms 级超时即返回 None（:273）——性能护栏，tor 无此项。
- `first_unbalanced_open_paren`（:232-253）：剥离 `Update(` 前缀、保留文件名平衡括号。

## 6. 已落到 torvox 代码的注释

- `native/src/terminal/pty.rs` build_env 上方注释块：Termux 变量/SSL_CERT_FILE 缺口 + kill 链参考。
  - 行号版本已按 review-1 修正（bootstrap.rs:386-434、terminal.rs:123-161、kill_active_task :2278-2288 → pty_info.rs:144-158）。

## 完成状态

- [x] env.rs 全文（98）
- [x] bootstrap.rs env 两函数（319-455）
- [x] terminal.rs env 消费 + kill_active_task + 测试（3437-3458）
- [x] pty_info.rs 全文（221）
- [x] terminal_hyperlinks.rs 1-330
- [x] port.rs trait（75-241：RuntimeProvider 契约，termux/chroot/bootstrap 三 adapter 实现）
- [ ] mappings/keys.rs 417 行（子代理覆盖 + 骨架抽查）
- [ ] gpui_android core + Kotlin MainActivity（子代理覆盖，load 会抽读）
## deep-v5 增量（复核第 2 轮：crates/util/src/env.rs 98 行全读）

### EnvOp 模式（:19-25）+ 三个 OnceLock 注册表

- `EnvOp { Set(OsString), Remove }`（:19-25）：adapter env 契约的单一变更原语——adapter 返回 `Vec<(String, EnvOp)>`，PTY spawn 时应用。**torvox 对照**：ShellEnv 硬编码 + env.extra（pty.rs:868）——EnvOp 是"运行时覆盖"的更干净设计。P2 记录：torvox MCP 若需运行时 env 配置可参考。
- `TERMINAL_ENV_OVERLAY`（:42-61）：注册一次，重复注册 warn 丢弃（Activity 重建安全）。
- `WORKSPACE_ROOT`（:66-84）：workspace 根（UI 分组 + 缓存）。torvox 无此概念（终端无 workspace）。
- **`NPM_LIBTERMUX_EXEC_PATH`（:88-98）**：libtermux-exec.so 路径用于 **LD_PRELOAD 到硬编码 `/data/data/com.termux/` 路径的 bionic CLI（Bun 编译的 Termux npm 包：claude、codex）**——**与 torvox bootstrap 的 termux-exec 方案同源**，第三方项目（Zed 编辑器 Android 移植）实证该方案正确性（torvox postinst 修复即用此机制）。

## deep-v6 增量（复核第 3 轮：zdroid_runtime/adapters/bootstrap_install.rs:1-408）

### bootstrap 安装路径对照（torvox BootstrapInstaller 直接相关）

- `RELEASE_ASSET_NAME = "bootstrap-aarch64.zip"` + **后缀匹配**（`-zdroid`/`-r4` 变体，:33-41）——**torvox 需要支持 nix-on-droid bootstrap-aarch64.zip 下载源**（用户要求），zed 的变体匹配模式可参考
- **`.bootstrap-version` release-tag 幂等**（:55-74）：磁盘版本 == 最新 release tag → 跳过提取。torvox 注释引用了 warp 的 sha256 marker（P0 待办）——**两种幂等思路**：sha256（检测损坏）vs release tag（跳过重提取）；torvox 可两者结合（sha256 验完整性 + tag 跳过）
- **symlink manifest 重放**（:43-47）：zip 无法表达 symlink/mode bits（Android 提取不保留）→ manifest 重放。torvox symlink 白名单（提取时检查）为另一思路——zed manifest 方案更可靠（P2 记录）
