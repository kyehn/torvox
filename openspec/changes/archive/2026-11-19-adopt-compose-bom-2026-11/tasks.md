# 任务

## 1. 依赖与编译平台

- [x] 1.1 确认升级后的失败根因：`checkDebugAarMetadata` 因 ui-android 1.13.0 要求
      compileSdk ≥ 37.1 而拒绝（复现命令：`./gradlew :app:checkDebugAarMetadata`）
- [x] 1.2 `compileSdk` 保持 37，补 `compileSdkMinor = 1` 并在脚本内写明触发原因
      （验证：`./gradlew :app:assembleDebug` BUILD SUCCESSFUL）
- [x] 1.3 依赖版本升级到 compose-bom 2026.11.01 与 lifecycle 2.12.0-alpha06
      （验证：`./gradlew :app:compileDebugKotlin` BUILD SUCCESSFUL）

## 2. 滑块 API 迁移

- [x] 2.1 `SettingsSliderRow` 从已弃用的 `Slider(value=…)` 重载迁到状态式
      `Slider(state=…)`（验证：`-Werror` 下 `:app:compileDebugKotlin` 通过）
- [x] 2.2 区间/步数变化时重建状态，外部值写回状态，拖动值先落状态再回调
      （验证：区间收紧场景下拇指与读数仍同源，见 `font-selection`
      「区间收紧时存量字号就地收敛」）

## 3. 验证与规约

- [x] 3.1 跑门禁：detekt、spotlessCheck、dokkaGenerate、lintDebug、lintVitalRelease、
      assembleDebugAndroidTest、testDebugUnitTest、benchmark/baselineprofile 编译
      （验证：全部 BUILD SUCCESSFUL，单元测试 1080 项全过）
- [x] 3.2 编写提案、设计与 `build-and-validation` 的规约增量
      （验证：`openspec validate adopt-compose-bom-2026-11 --strict` 通过）
- [x] 3.3 归档本 change 并同步主规约（验证：`openspec validate --all --strict` 通过）

## Workflow follow-up

- 归档后确认 `openspec/specs/build-and-validation/spec.md` 的 Android 构建参数要求
  已含「同级别次版本号」的允许条件与场景。
