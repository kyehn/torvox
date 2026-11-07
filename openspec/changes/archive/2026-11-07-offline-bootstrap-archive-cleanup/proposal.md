# Proposal

## Why

离线安装（本地路径选包）把引导 zip 复制到应用数据目录下的中转位置再解压。中转包落在
用户数据树内，系统从不清理 `filesDir`，故它必须在结束后被删除。原实现把删除放在
`runBlocking` 内部的 `finally` 里：复制阶段（`copyTo`）失败时尚未进入该 `finally`，
目标文件已写入半截内容却永久留下——引导归档有数百 MB，一次复制失败就长期占住用户数据。

复制失败与安装失败对「中转包必须清掉」是同一件事，此前只覆盖了后者。

## What Changes

- 离线安装的中转包改为「复制 → 使用 → 必删」的一处包装：删除动作覆盖复制与安装全过程，
  复制中途失败留下的半截内容同样被删除。
- 新增单测钉住三条路径：正常使用后删除、复制失败不留残件、使用失败后删除。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `bootstrap-install`：新增「离线安装的中转包必须清理」要求（复制与安装全过程结束后删除）。

## Impact

- `android/app/src/main/java/terminal/emulator/installer/BootstrapInstallService.kt`：
  离线安装流程改用共享包装
- `android/app/src/main/java/terminal/emulator/installer/BootstrapInstaller.kt`：
  新增 `withPreservedArchive`
- `android/app/src/test/java/terminal/emulator/installer/PreservedArchiveTest.kt`：
  中转包生命周期的三条回归钉
- `openspec/specs/bootstrap-install/spec.md`：中转包清理要求
