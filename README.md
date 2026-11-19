# assets

构建与预览产物分支：仓库内不保存这些文件。

## screenshots/

`PreviewScreenshotTest` 在模拟器上生成的界面预览图：fastfetch 输出、会话抽屉、设置页。

## baselineprofile/

- `baseline-prof.txt`、`startup-prof.txt`：`./gradlew :app:generateBaselineProfile` 采集的规则。
  采集时 `includeInStartupProfile = true`，故两者内容相同。
- `baseline.prof`、`baseline.profm`：release APK 内 `assets/dexopt/` 的 dexopt 产物。

## 重新生成

```sh
# 原生库（x86_64，模拟器用；arm64 由 CI 或真机走 arm64-v8a）
nix develop . --command nu scripts/build-android-libs.nu x86_64
# 预览截图（真实 prefix + 真实 fastfetch 安装）
nix develop . --command nu scripts/setup-emulator.nu
(cd android && nix develop .. --command ./gradlew :app:connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.ui.PreviewScreenshotTest)
adb pull /data/local/tmp/termux-preview-screenshots ./screenshots
# baseline profile（需模拟器在线；采集走 BaselineProfileGenerator 的真实路径）
(cd android && nix develop .. --command ./gradlew :app:generateBaselineProfile)
# dexopt 产物：先出 release APK，再从 APK 的 assets/dexopt 抽出
nix develop . --command nu scripts/build-apk.nu
unzip -p android/app/build/outputs/apk/release/app-release.apk assets/dexopt/baseline.prof > baselineprofile/baseline.prof
unzip -p android/app/build/outputs/apk/release/app-release.apk assets/dexopt/baseline.profm > baselineprofile/baseline.profm
```

`generateBaselineProfile` 的采集产物落在 `android/app/src/release/generated/baselineProfiles/`，
release APK 的 dexopt 产物取自 `android/app/build/outputs/apk/release/app-release.apk` 的
`assets/dexopt/`。
