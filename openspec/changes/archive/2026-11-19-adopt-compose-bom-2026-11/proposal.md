# Proposal

## Why

Compose BOM 升级到 2026.11.01 后构建整体失败：ui-android 1.13.0 的 AAR 元数据要求编译于
API 37.1 及以上，而工程声明的 `compileSdk` 仍是 37（android-37.0），`checkDebugAarMetadata`
直接拒绝；同时 Material3 1.5.0 弃用了 `Slider(value=…)` 重载，`-Werror` 下 Kotlin 编译不通过。
两项都不是可回退的偶发问题：只要依赖升级留在仓库里，构建就一直红着，后续任何验证都无从谈起。

## What Changes

- 依赖：compose-bom 2026.09.00 → 2026.11.01；lifecycle 2.12.0-alpha05 → 2.12.0-alpha06。
- 构建：`compileSdk = 37` 保持 API 级别不变，补 `compileSdkMinor = 1`——android-37.1 仍属
  API 37 同一级别的小版本，只满足 AAR 元数据的 compileSdkMinor 要求，不抬高 API 级别。
- 代码：`SettingsSliderRow` 从已弃用的 `Slider(value=…)` 重载迁到状态式 `Slider(state=…)`。
  状态在区间/步数变化时重建（状态只在首次组合时记下刻度与区间），外部值经副作用写回状态，
  拖动值先落状态再回调：拇指与读数始终同源。
- 规格：`build-and-validation` 的 Android 构建参数要求补记「API 级别不变时可声明同级别
  次版本号」的允许条件与判据，避免下次升级又把「37.1 ≠ 37」当成违规。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `build-and-validation`：Android 构建参数要求允许在 API 级别不变的前提下声明次版本号
  （`compileSdkMinor`），并写明触发条件与判据。

## Impact

- 代码：`android/app/build.gradle.kts`（依赖版本与 `compileSdkMinor`）、
  `android/app/src/main/java/terminal/emulator/ui/SettingsComponents.kt`（滑块迁移）。
- 依赖：Compose BOM、lifecycle runtime/viewmodel/runtime-compose。
- 构建环境：需要 android-37.1 平台（AGP 在 SDK 管理器可用时自动补齐；本仓已安装）。
- 行为：字号调节条的档位网格与取值区间语义不变——区间收紧时拇指与读数仍同步收敛到
  新上界（`font-selection` 的「区间收紧时存量字号就地收敛」不受影响）。
