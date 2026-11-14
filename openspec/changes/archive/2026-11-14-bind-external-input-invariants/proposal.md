# Proposal

## Why

`upstream-alignment` 的复查台账把 `Lord1Egypt/AndroidThothTerm` 的安全契约登记为
「仅参照，建议落为规范正面不变量」：外部 Intent 数据不得键入 PTY、外部路径逐段校验
且检查失败即拒绝。本仓的规范目前只有禁止项（`prohibited-features`），没有对应的正面
不变量条目；实现大体满足，但有两处缺口：

- 外部 Intent 指定的引导归档路径原先原样转交安装服务：归档内容会被写进 `files/usr`
  并以 shell 身份执行，等于让任何应用借本应用装进自己的二进制（该入口虽仅 debug 构建
  响应，导出的启动 Activity 仍对设备上的其他应用开放）。
- 外部引导包地址与未知日志等级取值被原样写进日志，与仓内既有的「只记
  `scheme://host`」「不记 URL」口径不一致。

## What Changes

- 代码：安装服务按规范路径的包含性判定引导归档的来源路径，站外路径一律拒绝，
  检查失败亦拒绝（`isInsideAppPrivateDir`）。
- 代码：抽出 `urlOrigin` 单一实现，外部 URL 只记来源部分；未知日志等级取值不再回显，
  且降为 debug。
- 规格：`app-lifecycle` 增「外部 Intent 的数据只走结构化通道」，`documents-provider`
  增「外部寻址一律按根内包含性判定」，`bootstrap-install` 增「引导归档的来源路径
  必须限定在应用私有目录内」。
- 规格：`upstream-alignment` 的复查台账把该行由「建议落为正面不变量」改为「已采纳」，
  并写明三条落点。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `app-lifecycle`：新增外部 Intent 输入的不变量。
- `documents-provider`：新增外部寻址的根内包含性判据。
- `bootstrap-install`：新增引导归档来源路径的边界。
- `upstream-alignment`：复查台账该行改为已采纳并写明落点。

## Impact

- `android/app/src/main/java/terminal/emulator/installer/BootstrapInstallService.kt`：
  引导归档来源路径判据。
- `android/app/src/main/java/terminal/emulator/util/UrlOrigin.kt`、
  `android/app/src/main/java/terminal/emulator/MainActivity.kt`、
  `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt`：日志口径。
- `android/app/src/test/java/terminal/emulator/installer/BootstrapArchivePathTest.kt`、
  `android/app/src/test/java/terminal/emulator/util/UrlOriginTest.kt`：回归用例。
- `openspec/specs/`：归档时同步上述条目。
