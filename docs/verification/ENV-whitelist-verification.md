# ENV 白名单注入修复 — 验证基线报告

- 分支：`fix-env-whitelist-verify`（基于 `origin/main`，本分支不含修复改动）
- 修复目标：native 侧 `build_env` 按白名单新增 `ENV=$HOME/.mkshrc`，使交互 mksh 加载应用写入的 `$HOME/.mkshrc`（`PS1='$ '`，termux 对齐短提示符）
- 报告性质：可复现验证基线（复现步骤 + 预期现象 + 验收标准 + 回归检查项），供修复后回归对照

---

## 1. 背景与根因（已设备取证，可信）

- **根因**：mksh（Android 的 `/system/bin/sh`）交互 shell 在未加载自定义 rc 时，回落到 AOSP 系统 rc `/system/etc/mkshrc`。该 rc 提示符为 `$HOSTNAME:${PWD} $ `，应用 home 为 `/data/data/com.termux/files/home` 时约 38~43 列（宽终端里仍占半屏）。
- **行编辑触发条件**：提示符占用大量列后，输入稍长的命令（如 `echo` + 60+ 字符）超出终端右缘，mksh 启用行内横滚（inline horizontal scroll）：发出 `\r` + 滚动窗口重绘 + 右侧 `<` 标记 + 超长退格串，屏幕表现为行首被裁、右上角 `<` 标记，持续破坏该行显示。
- **Kotlin 侧已就位**：`TerminalRuntime.ensureMkshPromptRc()`（`android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt:809`）会在会话启动时写 `$HOME/.mkshrc`：

  ```sh
  # terminal: termux-parity prompt (see TerminalRuntime.ensureMkshPromptRc)
  . /system/etc/mkshrc
  PS1='$ '
  ```

  但 native 侧 `build_env`（`native/src/terminal/pty.rs:895`）按规范白名单构造子进程环境，**未注入 `ENV`**，mksh 不会读取 `~/.mkshrc`，提示符保持长格式。
- **设备实验结论**（此前取证，本报告复测）：`ENV` 指向 `$HOME/.mkshrc` 时 mksh 才加载该 rc（提示符变为 `$ `）；bash 忽略 `ENV`（不受影响）。

## 2. 环境信息

| 项 | 值 |
| --- | --- |
| 模拟器 | emulator-5554（`sdk_gphone64_x86_64`，SDK 35，google_apis，x86_64） |
| 启动状态 | `adb devices` 在线，`sys.boot_completed=1`，无需重启 |
| 宿主分支 | `fix-env-whitelist-verify`（无修复改动，基线态） |
| shell | `/system/bin/sh` → mksh（AOSP 静态二进制，非符号链接） |
| 系统 rc | `/system/etc/mkshrc`（提示符 `$HOSTNAME:${PWD} $ `；root 会话为 `#`） |
| 应用 | `com.termux`（`terminal.emulator`）基线 debug APK，安装于模拟器（见 §4） |
| 取证工具 | `adb shell -tt`（设备侧真实 PTY）+ 原始字节流捕获 + pyte VT 屏幕模型还原 |

## 3. 复现步骤（设备级，shell 会话取证）

> 目的：在真实设备 PTY 上复现 mksh 行内横滚三类症状，并证明 `ENV` 注入为唯一差异变量。
> 宿主无可用 PTY，改用 `adb shell -tt` 强制设备侧分配 PTY，输入经标准输入管道转发，输出为原始字节流（与终端模拟器收到的流一致）。

```sh
# 1) 以 root 建立与应用完全一致的 home 并写入与 ensureMkshPromptRc 相同内容的 .mkshrc
adb root
adb shell "mkdir -p /data/data/com.termux/files/home"
adb shell "printf '# terminal: termux-parity prompt\n. /system/etc/mkshrc\nPS1='"'"'$ '"'"'\n' > /data/data/com.termux/files/home/.mkshrc"

# 2) 基线用例 A：不注入 ENV（当前 build_env 行为）
printf 'stty size\r\necho 1234567890-1234567890-1234567890-1234567890-1234567890-1234567890\r\nclear\r\nexit\r\n' \
  | adb shell -tt \
      "stty cols 80 rows 24; cd /data/data/com.termux/files/home; env HOME=/data/data/com.termux/files/home TERM=xterm-256color /system/bin/sh -i" \
  > caseA.raw

# 3) 对照用例 B：注入 ENV=$HOME/.mkshrc（修复后的预期行为）
printf 'stty size\r\necho 1234567890-1234567890-1234567890-1234567890-1234567890-1234567890\r\nclear\r\nexit\r\n' \
  | adb shell -tt \
      "stty cols 80 rows 24; cd /data/data/com.termux/files/home; env HOME=/data/data/com.termux/files/home TERM=xterm-256color ENV=/data/data/com.termux/files/home/.mkshrc /system/bin/sh -i" \
  > caseB.raw
```

原始流证据：`docs/verification/evidence/caseA_noENV_c80.raw`（等价物保留在 `/tmp/mksh_repro/caseA_noENV_c80.raw`，`*.raw` 被 .gitignore 忽略不提交）。

## 4. 三类症状取证结果

### 症状一 + 症状二：行首被裁（左缘裁剪）与右上 `<` 标记（行内横滚）

用例 A（无 `ENV`）在 80×24 PTY 键入 `echo` + 70 字符后，屏幕（pyte 还原，取 `clear` 前状态）为：

```
00|emu64xa:/data/data/com.termux/files/home # stty size
01|24 80
02|cho 1234567890-1234567890-1234567890-1234567890-1234567890-1234567890         <
03|1234567890-1234567890-1234567890-1234567890-1234567890-1234567890
04|emu64xa:/data/data/com.termux/files/home #
```

- 第 02 行：命令回显首字符 `e`（`echo` 的 e）**被裁掉**，仅显示 `cho …`——行首被裁（左缘裁剪）。
- 第 02 行右缘：`<` 标记出现在最右侧——行内横滚窗口标记（右上 `<`）。
- 原始字节流：`\r` + `cho 123…` + 空白 + `<` + 74 个连续退格 `\b\b\b…` + 尾部重打 —— 即 mksh 行内横滚的完整重绘序列。

对照用例 B（注入 `ENV=$HOME/.mkshrc`）同一操作：

```
00|$ stty size
01|24 80
02|$ echo 1234567890-1234567890-1234567890-1234567890-1234567890-1234567890
03|1234567890-1234567890-1234567890-1234567890-1234567890-1234567890
```

- 提示符为短提示符 `$ `（termux 对齐）；长命令完整回显，`echo` 首字母不丢，无 `<` 标记，无退格重绘。

### 症状三：`clear` 失效

- **字节层面**：用例 A 中 `clear` 命令正常发出 `ESC[2J` + `ESC[H`（即 console-clear 序列），逐字节捕获显示 clear 本身可执行（`/system/bin/clear` → toybox 存在）。行内横滚造成的损坏发生在**键入该行期间**；回车执行后 mksh 从新行重绘提示符，此时 clear 可把屏幕清空、新提示符回到首行首列。
- **渲染层面（应用内）**：损坏的行内横滚重绘（裁切 + `<` + 退格串）是终端模拟器必须正确消费的状态流；「clear 失效」的可见表现为应用渲染端对损坏行的残留处理（残影/错位）。此层面须以应用内截图取证，见 §4.1 应用级验证。
- **验收判据**：修复后（短提示符）不存在损坏行，`clear` 前后屏幕必须干净：清屏后首行首列为 `$ ` 提示符，无任何残留字符。

### 补充取证：Ctrl-C 中止长命令

中止场景下 mksh 输出 `^C\r\n` 后以 `130|` 状态前缀重绘提示符（`130|emu64xa:/data/data/com.termux/files/home # `，更长于普通提示符），随后 clear 在字节层面同样正常。回归时需覆盖该路径（见 §7）。

### 取证产物

| 文件 | 内容 |
| --- | --- |
| `evidence/caseA_noENV_c80.stage-mangled.txt` | 用例 A（无 ENV）clear 前屏幕——裁切与 `<` 标记 |
| `evidence/caseA_noENV_c80.screen.txt` | 用例 A 会话结束全屏 |
| `evidence/caseA_noENV_c96.screen.txt` | 用例 A 96×30 全屏 |
| `evidence/caseB_withENV_c80.stage-mangled.txt` | 用例 B（有 ENV）同一阶段——干净回显 |
| `evidence/caseB_withENV_c80.screen.txt` / `caseB_withENV_c96.screen.txt` | 用例 B 会话结束全屏 |

（原始字节流 `*.raw` 因 `.gitignore` 规则不提交，保留于 `/tmp/mksh_repro/` 和报告复现步骤可随时重生成。）

## 4.1 应用级验证（模拟器内 torvox 本体）

> 待基线 debug APK 构建完成、安装后在此补充：应用内启动（failsafe → `/system/bin/sh`，即 mksh 会话）的截图取证：长提示符、长命令回显裁切与 `<` 标记、`clear` 前后屏幕、`run-as com.termux` 检查 `$HOME/.mkshrc` 内容。

## 5. 预期现象（修复前基线，复现时对照）

| # | 操作 | 预期现象（基线态，未修复） |
| --- | --- | --- |
| P1 | 启动会话（failsafe / `/system/bin/sh`） | 提示符为长格式 `emu64xa:/data/data/com.termux/files/home $ `（约 38+ 列） |
| P2 | 输入 `echo` + 60+ 字符 | 键入行出现行内横滚：行首被裁、右缘 `<` 标记、退格重绘 |
| P3 | 回车执行后输入 `clear` 回车 | 屏幕清空后提示符回左上角；损坏行在应用渲染下可能残留碎片/残影 |
| P4 | Ctrl-C 中止长命令 | `^C` 后出现 `130|` 前缀长提示符，碎片残留 |

修复后以上均应变为：`$ ` 短提示符；长命令完整回显无裁切无标记；clear 干净。

## 6. 验收标准（修复后应满足）

1. **环境注入**：`build_env`（`native/src/terminal/pty.rs`）输出中新增 `ENV` 键，值为 `$HOME/.mkshrc`（与 `HOME` 同值，取自 `ShellEnv.home`）。
2. **白名单合规**：除新增 `ENV` 外，不引入其他未声明变量；`LD_LIBRARY_PATH` / `PWD` / `LD_PRELOAD` 仍禁止；宿主透传变量（`ANDROID_*` 等）行为不变；`build_env` 单元测试与 `env_steps.rs` BDD 步骤同步更新并通过。
3. **rc 加载**：交互 mksh 会话（`/system/bin/sh -i`，`HOME=/data/data/com.termux/files/home`）启动即加载 `$HOME/.mkshrc`，提示符变为 `$ `（termux 对齐）；该 rc 由 `ensureMkshPromptRc()` 保证写入且含 `PS1='$ '` 自愈标记。
4. **症状消除**：键入 `echo` + 70 字符（80×24 与 96×30 均测）不再出现——行首裁切、右缘 `<` 标记、`\r`+退格重绘；回显行首 `e` 完整。
5. **clear 正常**：执行 `clear` 后屏幕全清，新提示符位于首行首列，无碎片、无残影、无错位。
6. **bash 不受影响**：termux bootstrap bash 会话行为不变（bash 忽略 `ENV`；不引入对 bash 提示符的改动）。
7. **降级安全**：若 `$HOME/.mkshrc` 写入失败或缺失，mksh 回落系统 rc（长提示符），应用不崩溃、会话正常，仅回到基线现象。
8. **规范同步**：白名单文档（`docs/specification/DESIGN.md` Bootstrap 节，由实现分支负责）补充 `ENV` 声明。

## 7. 回归检查项（修复后逐项过）

- [ ] R1 双宽度（80×24、96×30）键入 `echo`+70 字符：回显行首完整（`echo` 首字母不丢），无 `<` 标记。
- [ ] R2 `clear` 回车：屏幕全清，首行首列为 `$ ` 短提示符，无残留。
- [ ] R3 Ctrl-C 中止长命令：`130|` 前缀提示符正常，无碎片残留。
- [ ] R4 连续多行长命令 + clear + 滚动回看：无错位、无叠加残影。
- [ ] R5 `adb shell run-as com.termux cat files/home/.mkshrc`：内容含 `PS1='$ '` 与 `. /system/etc/mkshrc`。
- [ ] R6 会话重启（force-stop 后重开）：`.mkshrc` 自愈逻辑生效，提示符仍为 `$ `。
- [ ] R7 横竖屏旋转 / 输入法弹出隐藏后：提示符仍为短格式，屏幕无错位。
- [ ] R8 `build_env` 输出审查：变量集合 = 白名单原变量 + `ENV`；无 `LD_*`；哈希/长度变化符合预期。
- [ ] R9 native `build_env` 单元测试与 `native/tests/bdd` env 步骤全绿。
- [ ] R10 语义对照：已在 §3 步骤 3 中验证 `ENV=$HOME/.mkshrc` 手工注入即恢复短提示符——修复应使真实会话环境与此等价，无需手工注入。

## 8. 用于回归的精确命令

```sh
# 基线检查（修复前应复现症状）
adb shell -tt "stty cols 80 rows 24; cd /data/data/com.termux/files/home; env HOME=/data/data/com.termux/files/home TERM=xterm-256color /system/bin/sh -i"

# 修复后等价环境检查（手工注入 ENV，作为"修复后环境应与之等价"的基准）
adb shell -tt "stty cols 80 rows 24; cd /data/data/com.termux/files/home; env HOME=/data/data/com.termux/files/home TERM=xterm-256color ENV=/data/data/com.termux/files/home/.mkshrc /system/bin/sh -i"

# 应用内检查 rc 内容
adb shell run-as com.termux cat files/home/.mkshrc
```