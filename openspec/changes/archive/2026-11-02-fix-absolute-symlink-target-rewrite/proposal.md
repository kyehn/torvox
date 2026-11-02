# Proposal

## Why

`BootstrapInstaller.createSymlinks` 对绝对符号链接目标做了两套处理：

1. 校验时把官方包 SYMLINKS.txt 里的旧式绝对路径
   （`/data/data/com.termux/files/usr/...`）取 `files/usr/` 之后的后缀，
   拼到**本机 prefix** 上，再按规范路径判是否落在 prefix 内；
2. 建链时却用归档里的**原值**（`Os.symlink(target, ...)`）。

两者不是同一个路径，于是重写形同虚设：

| 归档里的目标 | 校验用的路径 | 实际建链的目标 |
| --- | --- | --- |
| `/data/data/com.termux/files/usr/bin/gawk` | `<本机 prefix>/bin/gawk` | `/data/data/com.termux/files/usr/bin/gawk` |

校验放行的是本机 prefix，落盘的链接却指向归档里的旧 prefix。在二级用户与多用户
设备上本机 prefix 位于 `/data/user/<id>/<pkg>/files/usr`，旧路径不在本机 prefix
内，链接必然断裂——`bin/awk` 之类的入口指不到实际文件，引导装得上却用不了。

链接目标由归档作者提供，本仓只做了逃逸校验；校验与写入用不同路径，使「绝对目标
只允许落在规范 prefix 内」这条约束在链子上并不成立。

## What Changes

- 抽出纯函数 `resolveAbsoluteSymlinkTarget(target, canonicalPrefix)`：绝对目标取
  `files/usr/` 之后的后缀拼到本机 prefix；不含该分段的绝对路径原样返回。
- 建链改用重写后的目标，与校验用的路径同源。
- 保持既有逃逸校验不变：重写不消解 `..`，越界仍由规范路径校验拒绝。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `bootstrap-install`：「清单建链」补绝对目标重写到本机 prefix 的口径。

## 不做什么

- 不改相对目标的解析（相对链接的父目录，仍按 staging 内结构解析）。
- 不放开绝对目标的逃逸校验：重写后的路径仍必须落在本机 prefix 内。
- 不改 `docs/specification/`（禁改区）与 `docs/STATUS.md`。

## Impact

- Affected specs: `bootstrap-install`（「清单建链」一条 MODIFIED Requirement）。
- Affected code:
  - `android/app/src/main/java/terminal/emulator/installer/BootstrapInstaller.kt`
    （`createSymlinks` 与新增的 `resolveAbsoluteSymlinkTarget`）
  - `android/app/src/test/java/terminal/emulator/installer/BootstrapInstallerTest.kt`
    （三条纯路径用例）
  - `android/app/src/androidTest/java/terminal/emulator/installer/BootstrapSymlinkInstrumentedTest.kt`
    （一条真机建链用例）
