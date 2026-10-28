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
nix develop --command cargo ndk -t x86_64 build --release
cp native/target/x86_64-linux-android/release/libnative.so android/app/src/main/jniLibs/x86_64/
(cd android && ./gradlew :app:connectedDebugAndroidTest \
  -Pandroid.testInstrumentationRunnerArguments.class=terminal.emulator.ui.PreviewScreenshotTest)
adb pull /data/local/tmp/termux-preview-screenshots ./screenshots
(cd android && ./gradlew :app:generateBaselineProfile)
```

`generateBaselineProfile` 的采集产物落在 `android/app/src/release/generated/baselineProfiles/`，
release APK 的 dexopt 产物取自 `android/app/build/outputs/apk/release/app-release.apk` 的
`assets/dexopt/`。
