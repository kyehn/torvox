# 变更：exec-bin 应用域启动器（fork Go login 不经 Shizuku 可用）

## Why

fork（kyehn/nix-on-droid）自带 Go 版 `login` 为动态链接（`/nix/store/...-glibc/.../ld-linux`
INTERP），此前仅 Shizuku-root 链式加载可跑；应用域（`untrusted_app`，Enforcing）直接
`execve` 被拒（`execute_no_trans` EACCES/126，设备实证）。用户要求：使用 fork 原生 Go
`login`（不得重编为 static-PIE 替代），经 exec-bin 在应用域启动并跑通 `nix build`，
验证只认真实终端输入 + 真实截图。`exec-bin` crate 此前只构建进 `assets/bin` 从未被
安装/调用（死代码），本变更让它成为负载路径。

设备实证前置结论（2026-09-11，见 design-compliance-bootstrap §5-§6）：

- `files/` 与 `/data/local/tmp` 内二进制不可直接执行；APK `lib/` 内可执行。
- 系统 linker 只做文件映射（`execute` 而非 `execute_no_trans`），可加载静态 PIE 与
  bionic 动态二进制；glibc-INTERP 二进制需 prefix 自带 loader 链式加载
 （`ShizukuGate.chainLoadPrefix` 已在 Shizuku 路径证明）。
- fork 闭包内无 `libtermux-exec`，故无 LD_PRELOAD 钩子可用；`login` 结尾为原生
  `syscall.Exec(proot)`，其在应用域的行为由设备裁决（实验 E4）。

## What Changes

- `exec-bin`：由裸 `execvp` 转为 INTERP 感知 trampoline（`resolve` 纯函数，宿主单测覆盖）：
  `/nix/store` INTERP → 由目标路径反推 prefix（首个含 `nix/store` 的祖先），经系统
  linker 用 prefix glibc loader 链式加载（`--library-path` 为枚举到的全部
  `nix/store/*/lib`）；其余 ELF 走系统 linker；`#!` 脚本递归分发到解释器；
  `/system` 路径直接执行。exec-bin 永不直接 exec 应用数据文件。
- Android：`ExecBin.ensureInstalled` 在 `MainActivity.onCreate` 把
  `assets/bin/<abi>/exec-bin` 同步到 `files/exec-bin`（字节比对，失败大声打日志，
  不崩溃启动）；Maestro 实验流以绝对路径调用。
- 实验矩阵（全部 Maestro 真实终端输入 + 截图，`adb shell` 只做安装/推送/截图读取，
  不执行验证命令）：E1 直接 exec login `--dry-run`；E2 经系统 linker；
  E3 经 exec-bin `--dry-run`；E4 完整 login（记录 proot 步骤的设备裁决）；
  E5 exec-bin 起 prefix bash；E6 exec-bin 起 proot `--version`；E7 按 `--dry-run`
  打印的 proot 命令行经 exec-bin 进完整会话并跑 `nix build`。
- 截图取证：Vulkan/wgpu 表面不进 screencap 合成（模拟器限制，与内容无关），临时
  Compose debug overlay 显示 scrollback 末 5 行使终端文字可截图；取证后删除，
  不进发布。

## Capabilities

### Modified Capabilities

- `shell/bootstrap-environment`：启动语义新增 exec-bin 应用域启动路径（与 Shizuku
  桥接并列，不互相替代）。

## Impact

- 发布体积 +~0.5MB/ABI（已在包内的 asset 被启用，无新增依赖）。
- 现有 Shizuku 路径、failsafe 路径零改动；实验失败项如实记录，不静默降级。

## Open Questions

- E4 若裁决 login 的 `syscall.Exec(proot)` 在应用域被拒，完整会话仍需 Shizuku；
  E7（脚本直调 proot）为备选完整路径，按实验结果再立后续变更。

## Non-Goals

- 不重编 fork login（不用 static-PIE 替代）；不编译 LLVM/重型工具链。
- 不改动 prefix 用户数据；不手动解压 bootstrap（走应用内安装器）。
- 不重写 git 历史。
