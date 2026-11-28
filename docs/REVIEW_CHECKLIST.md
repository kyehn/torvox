# 逐文件审查清单

本清单由 `git ls-files` 枚举现行受控代码文件生成，不以 diff 或某次提交为界。“风险模式命中”是二次扫描的提示计数，不等于问题数；测试文件中的 `unwrap`/`expect` 多为断言，生产代码中的 `unsafe`/`allow` 均已逐处核对前置条件。命中项均已被对应工具链与测试覆盖，人工重点复核的文件另行标注。

| 模块 | 文件 | SHA-256（前 12 位） | 行数 | 风险命中 | 核验结论 |
| --- | --- | --- | ---: | ---: | --- |
| Config | `.cargo/config.toml` | `52d74c818886` | 15 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Gradle build | `android/app/build.gradle.kts` | `bf69a62b39e0` | 262 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/lint.xml` | `9939d6873e87` | 5 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| R8/ProGuard | `android/app/proguard-rules.pro` | `d52a43888aba` | 22 | 7 | assembleRelease/analyzeReleaseR8Config 通过；人工重点复核；风险模式命中 7 处 |
| Android instrumentation | `android/app/src/androidTest/AndroidManifest.xml` | `df89dd6fe4f5` | 8 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/com/termux/test/CucumberOptionsClass.kt` | `a9dcbc5f942c` | 25 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/BehaviorInstrumentedTest.kt` | `9ba638d56efc` | 239 | 3 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 3 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/BehaviorVerificationTest.kt` | `59aed54e401a` | 163 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/DesktopNotificationInstrumentedTest.kt` | `c92c885dd99f` | 72 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/DocumentsProviderInstrumentedTest.kt` | `67a04e5620c1` | 1016 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/FontFileOverrideInstrumentedTest.kt` | `244e134da89c` | 86 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/FontSwitchInstrumentedTest.kt` | `b77088630fac` | 120 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/MainActivitySingleInstanceTest.kt` | `c7aa8b818206` | 57 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ScrollBehaviorQuantifiedTest.kt` | `cfc9badf83cd` | 384 | 10 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 10 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/SelectionDragQuantifiedTest.kt` | `7dd0ef7bf5ed` | 744 | 18 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 18 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/SelectionEspressoTest.kt` | `b20d514176fb` | 463 | 6 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 6 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ShellResponseLatencyTest.kt` | `098eb8433a26` | 128 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/StorageAccessInstrumentedTest.kt` | `11b32b6b487f` | 96 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/TerminalLogcatRule.kt` | `1cfdc3f45b2e` | 159 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/TestUtils.kt` | `f43ea6b8365c` | 817 | 12 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 12 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ThemeApplyOrderInstrumentedTest.kt` | `08bad81a1c42` | 211 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ThemeFollowSystemInstrumentedTest.kt` | `b89f25e5addd` | 289 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ThemeInstrumentedTest.kt` | `3c13edca2647` | 150 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/UxTestUtils.kt` | `4acdee9e6946` | 568 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/bridge/NativePreloadInstrumentedTest.kt` | `b48221b78e1e` | 40 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/ComposeRuleHolder.kt` | `0044055f0493` | 26 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/Hooks.kt` | `cd70255d2a72` | 38 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/SimpleHiltObjectFactory.kt` | `d9ddda0616ab` | 55 | 3 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 3 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/CommonSteps.kt` | `a3bd9bfa5744` | 36 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/FontSteps.kt` | `07687bfb1f50` | 63 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/LifecycleSteps.kt` | `66330ecc6877` | 59 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/ModifierBarSteps.kt` | `06cc87e447a5` | 99 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/NavigationSteps.kt` | `3e74d2352df2` | 140 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/SearchSteps.kt` | `2cd6384b2b98` | 91 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/SelectionSteps.kt` | `a59fa5230b3d` | 244 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/SessionSteps.kt` | `d012cd5cccdb` | 120 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/SettingsSteps.kt` | `2b42c0d6aa68` | 84 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/TerminalLaunchSteps.kt` | `7e14c62c0c88` | 48 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/ThemeSteps.kt` | `1b34a12005b9` | 91 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/diag/CursorPixelAcceptanceTest.kt` | `fb81713e7756` | 122 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/diag/SelectionTapDismissTest.kt` | `d1f205a7abd9` | 124 | 3 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 3 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/diag/SgrColorPixelAcceptanceTest.kt` | `74d6adf17f87` | 125 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/diag/SgrItalicPixelAcceptanceTest.kt` | `e67c81b62c33` | 90 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/BootstrapCompatibilityTest.kt` | `6042d4530404` | 341 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/BootstrapFixture.kt` | `36223d7c04ab` | 56 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/BootstrapSymlinkInstrumentedTest.kt` | `e6403159d605` | 125 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/TerminalPrefix.kt` | `4e5506c6f3bb` | 67 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/TermuxBootstrapRealTerminalTest.kt` | `2d074fa3c101` | 214 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/leakcanary/LeakCanaryIntegrationTest.kt` | `e520e985b3a8` | 20 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/performance/RenderPauseSemanticsTest.kt` | `6a2e3c42d94b` | 56 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/performance/RenderPauseTest.kt` | `6fc93ceb6286` | 42 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/AltScreenImeReflowInstrumentedTest.kt` | `bee15251842d` | 363 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/BackgroundResumeFlickerInstrumentedTest.kt` | `8fb9810e013d` | 517 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/BackspaceLatencyTest.kt` | `5590a3f2fcfb` | 244 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/BootstrapInstrumentedTest.kt` | `ec7d7eb0dfe6` | 102 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/CjkBackspaceSemanticsTest.kt` | `619e5e2d16c0` | 60 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/CjkComposingLatencyTest.kt` | `681e76ad3c7e` | 194 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/CjkPresentSemanticsTest.kt` | `544bd2039cdc` | 34 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/DrawerWidthInstrumentedTest.kt` | `cb7e9fa115cd` | 76 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/FontSizeReflowInstrumentedTest.kt` | `3887c926a76b` | 332 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/HardKeyboardMultipleArrivalTest.kt` | `69577849892d` | 99 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/IdleOutputWakeTest.kt` | `e273377f7391` | 197 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ImeBatchEditDeliveryTest.kt` | `7d237b5fdb7c` | 85 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ImePopupPixelInstrumentedTest.kt` | `7eda2944afa7` | 672 | 13 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 13 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ImeSurfaceStabilityInstrumentedTest.kt` | `21ae812b8ebd` | 120 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/KittyGraphicsOverlayTest.kt` | `08e01f853862` | 193 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/KittyGraphicsPlacementTest.kt` | `5ad13df77b1d` | 475 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/KittyRealClientIcatTest.kt` | `b6287cca7347` | 280 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ModifierBarBottomInsetInstrumentedTest.kt` | `a7b96058dd31` | 113 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ModifierBarGapInstrumentedTest.kt` | `6d4e45d32c0f` | 131 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ModifierBarPressFeedbackInstrumentedTest.kt` | `ad5df1e14279` | 341 | 3 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 3 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ModifierBarTest.kt` | `6c43683fc825` | 344 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/MouseReportingInstrumentedTest.kt` | `82c2df855f46` | 349 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/Osc52ClipboardInstrumentedTest.kt` | `55f2f8786d2b` | 113 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/PasteButtonInstrumentedTest.kt` | `acce847808c1` | 190 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/PreviewScreenshotTest.kt` | `3df0430793fc` | 312 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/PtyWriteQueueBindingInstrumentedTest.kt` | `8ad7e8761d34` | 65 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SessionCreationInstrumentedTest.kt` | `57ca11d8ec22` | 128 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SessionDrawerInstrumentedTest.kt` | `497667961a05` | 206 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SettingsScreenTest.kt` | `4616de66250c` | 131 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ShellPtyInstrumentedTest.kt` | `4c2a822be505` | 221 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/StickyCtrlInterruptInstrumentedTest.kt` | `3c6284781615` | 67 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SurfaceLossRecoveryInstrumentedTest.kt` | `97ab4fbf0586` | 647 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SurfaceRedrawAsyncInstrumentedTest.kt` | `02a231d8e5d1` | 37 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SystemBarIconInstrumentedTest.kt` | `7a8b802f82c9` | 115 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TeardownLeakInstrumentedTest.kt` | `1f55fe65356b` | 92 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TerminalHoleCoverInstrumentedTest.kt` | `1cec757d2cf8` | 371 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TerminalHoleTransitionFillInstrumentedTest.kt` | `2ff13f26ee07` | 362 | 6 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 6 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TerminalScreenLayoutTest.kt` | `8e37b6643fbc` | 46 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TextSearchEndToEndTest.kt` | `0b37a53ba21c` | 490 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/UiAutomatorTest.kt` | `ba13766c3bb4` | 216 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/VisualInlineVerificationTest.kt` | `09cc81bfe975` | 455 | 5 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 5 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/VtCorrectnessInstrumentedTest.kt` | `a3853e85b7c8` | 373 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ZoomPreviewGestureInstrumentedTest.kt` | `4b6cd2b25547` | 254 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android manifest/resources | `android/app/src/debug/AndroidManifest.xml` | `74370c783774` | 29 | 1 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 1 处 |
| Android manifest/resources | `android/app/src/main/AndroidManifest.xml` | `b85dfb17e7de` | 126 | 2 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/BootstrapManager.kt` | `6e7fdb656938` | 215 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/DocumentMutations.kt` | `ae978b8780b9` | 617 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/DocumentQueries.kt` | `d401d9fe3d2f` | 210 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/DocumentWatcher.kt` | `9efe6a2ab6a3` | 231 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/FontManager.kt` | `ecb0cf61ce1b` | 226 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/FontUtils.kt` | `a6c4836c0b24` | 47 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/MainActivity.kt` | `1a9e595083d6` | 669 | 3 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 3 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/SelectionManager.kt` | `f1f955c93b3c` | 494 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/SelectionState.kt` | `9d2eaf0ee6e4` | 155 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/SessionState.kt` | `790e93451641` | 88 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/TerminalApp.kt` | `b4642a335596` | 131 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/TerminalDocumentsProvider.kt` | `174ac3e86872` | 672 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/TerminalViewModel.kt` | `5da31c205733` | 1230 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/ThemeManager.kt` | `1eac8ee2afe6` | 49 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/TransientMessageChannel.kt` | `48364a7c6949` | 42 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/Bridge.kt` | `4fcf2e84f3a9` | 943 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/FontInfo.kt` | `078c74ded7bd` | 164 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt` | `731cc63e353f` | 637 | 4 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 4 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/NativeQueryPort.kt` | `c03cde114780` | 122 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/PollEvent.kt` | `342dbc936992` | 76 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/RenderContract.kt` | `77255bd8a480` | 87 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/ComposingBaseline.kt` | `a464620beeca` | 149 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/ImeCursorContract.kt` | `9189e2a75fd3` | 129 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/KeyModifiers.kt` | `929bd0fcb5a3` | 99 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/ModifierState.kt` | `f992dcb87b4a` | 33 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/TerminalEditorInfo.kt` | `b7f709fd92ab` | 30 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapDownloader.kt` | `152686ea229b` | 173 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapInstallService.kt` | `af6d1c32132e` | 89 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapInstaller.kt` | `029bf27fb371` | 512 | 2 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapOrchestrator.kt` | `9d6561c7541a` | 114 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapPreset.kt` | `8e1f94ddf485` | 31 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapProgress.kt` | `2a02925d21c2` | 41 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/InstalledEntryProbe.kt` | `5987993f4a96` | 45 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/NixOnDroidBootstrap.kt` | `14ce089d74c3` | 26 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/TermuxBootstrap.kt` | `0d627421fffd` | 31 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/monitor/RenderWatchDog.kt` | `63a6a9cb46c7` | 112 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/monitor/SessionLiveness.kt` | `a95dc67e26cf` | 165 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/BellPlayer.kt` | `5f68c45c6d12` | 110 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/ClipboardAccess.kt` | `56d4cc37b4e8` | 76 | 2 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/CountedLogThrottle.kt` | `ea6f9cd559dc` | 50 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/CoverCapture.kt` | `c2268dcf4575` | 221 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/DesktopNotificationDrain.kt` | `0e393631f73c` | 66 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/DesktopNotificationPlayer.kt` | `6b71a0094e76` | 173 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/DesktopNotificationPump.kt` | `a39a171bbf94` | 63 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/FrameClock.kt` | `c36352467248` | 27 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/FrameTimingStats.kt` | `7bf730cade7c` | 126 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/FrameTimingTrend.kt` | `b42283a91327` | 55 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/GridGeometry.kt` | `43454bfca24f` | 135 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/HoleStateJournal.kt` | `112c91d7e2c3` | 118 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/InputBatchBuffer.kt` | `5308098a0c02` | 202 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/LogLevel.kt` | `f7938f1ac748` | 42 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/LogUtil.kt` | `7ce027374a0f` | 53 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/PasteChunks.kt` | `224ec09c83ac` | 57 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/RenderErrorLedger.kt` | `a11129638ad4` | 74 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/RenderFailurePolicy.kt` | `20996a707fba` | 163 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/RenderSupervisor.kt` | `d6c3300e57d4` | 1362 | 3 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 3 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/ScrollFreshness.kt` | `d539d38e9a11` | 40 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionBridgeCloser.kt` | `b4d0752c4d38` | 36 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionConfigBuilder.kt` | `e14a44c2ec87` | 136 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionEntry.kt` | `89da14b9f3cd` | 287 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionRegistry.kt` | `d5bb39e06502` | 121 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionSwitchPolicy.kt` | `ec6f4028e5d7` | 34 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SurfacePresentTracker.kt` | `4606ba10acd4` | 127 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt` | `3a2db361f139` | 3529 | 2 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntimeEntryPoint.kt` | `345d170162ce` | 18 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/VsyncChain.kt` | `1d23099b105b` | 241 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/service/TerminalForegroundService.kt` | `431866010df5` | 360 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/ColorsProperties.kt` | `dc55ebe5bc93` | 167 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/SettingsDataStoreProvider.kt` | `9ba71b828251` | 50 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/SettingsRepository.kt` | `8bcb4f7565f8` | 572 | 2 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/SystemFonts.kt` | `1b0918b48b10` | 24 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/TerminalKeyCatalog.kt` | `17a66fcf63b2` | 168 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/TermuxMacro.kt` | `de76c65b331e` | 163 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/TermuxProperties.kt` | `e55ae5e2baa9` | 846 | 3 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 3 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/TermuxPropertiesRepository.kt` | `27648d5d8905` | 174 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/BackgroundRenderPause.kt` | `870a2e720af5` | 63 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ComposingDiff.kt` | `1aad447f5d93` | 92 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/HandleDragController.kt` | `f4e62d22f278` | 291 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ImeConnection.kt` | `ad7ab17118a5` | 577 | 3 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 3 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ImeShiftController.kt` | `c30db9ae4bd1` | 458 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt` | `a3af6a9996e2` | 1186 | 2 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 2 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/PopupWindowLiveness.kt` | `c6b7ff18997b` | 69 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/RenderPauseLedger.kt` | `5641ed98fb45` | 56 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ResizeManager.kt` | `c8db62a6ba00` | 312 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SearchDebouncer.kt` | `f58fbcf43989` | 57 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SearchQuery.kt` | `6d9d9feebc43` | 12 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SelectionHandles.kt` | `9081c57e3e03` | 484 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SelectionMenu.kt` | `8e6f568aeddd` | 433 | 1 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 1 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SessionDrawer.kt` | `12db0a38e1db` | 406 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsAppearanceSection.kt` | `3df313ee494e` | 265 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsBootstrapSection.kt` | `59143792b23f` | 362 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsClearDataSection.kt` | `e7c8b21eb6b2` | 94 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsComponents.kt` | `d6b2949fb81b` | 349 | 1 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 1 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsLogLevelSection.kt` | `88285c1bd031` | 90 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsScreen.kt` | `402711600e65` | 251 | 1 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 1 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsShellSection.kt` | `515c237fd385` | 129 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsThemeSection.kt` | `793f3173797c` | 312 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SurfaceAttachGate.kt` | `8dd1fb990a5a` | 54 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SurfaceInput.kt` | `33e5d8796c7a` | 912 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SurfaceOwnership.kt` | `c8e766b5448e` | 39 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SystemBars.kt` | `444457424d84` | 84 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TerminalInputEncoder.kt` | `b9015ed0fee3` | 153 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt` | `6eb9a7147f52` | 1082 | 2 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 2 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt` | `423941ac5069` | 1348 | 1 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 1 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TerminalSurfaceMath.kt` | `7c6c2aee8966` | 493 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TextSearchBar.kt` | `b762fe05b8e7` | 223 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/theme/TerminalTheme.kt` | `18da0b2da06c` | 146 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/theme/ThemeRepository.kt` | `f8395a3a9722` | 145 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/DirectoryListing.kt` | `1fe4c5edd363` | 14 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/RunCatching.kt` | `54b6e15cb889` | 19 | 4 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 4 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/Streams.kt` | `16bf03f55519` | 19 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/TerminalDispatchers.kt` | `187a7d441ddf` | 15 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/TextTruncation.kt` | `7a70a0f7437a` | 46 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/UrlOrigin.kt` | `f7486087602d` | 17 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/WorkingDirectory.kt` | `3911414a4260` | 27 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/ic_launcher_background.xml` | `27bc6144a301` | 12 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/ic_new_session.xml` | `7c273165ffe7` | 17 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/ic_notification.xml` | `f3d2eea81c68` | 40 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/launcher_foreground.xml` | `0604080381d6` | 12 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/launcher_monochrome.xml` | `cc08a3bc7f5f` | 88 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/logo_art.xml` | `1e6e143ed582` | 125 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/splash_frame.xml` | `234ae2ff30ea` | 15 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/splash_icon.xml` | `ef6d4dad58bc` | 21 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/mipmap/ic_launcher.xml` | `2e697c3dcaaa` | 6 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/mipmap/ic_launcher_round.xml` | `2e697c3dcaaa` | 6 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/values/colors.xml` | `29d62427b42c` | 11 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/values/strings.xml` | `3b6c9395d575` | 139 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/values/themes.xml` | `a3e3ba0395d3` | 35 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/xml/data_extraction_rules.xml` | `32bc115d804e` | 25 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/xml/shortcuts.xml` | `2babcd8cc917` | 26 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/nix/res/values/strings.xml` | `fdc9074ae816` | 8 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/nix/res/xml/shortcuts.xml` | `9d05b563c491` | 19 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/AndroidManifest.xml` | `4bd5fd18174e` | 21 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/AwaitBridgeReadyTest.kt` | `ea9f9d5410bb` | 41 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ConfigurationChangeContractTest.kt` | `fd743ef55216` | 77 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/DeadKeyLogSeverityTest.kt` | `6e3646ee6971` | 81 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/DocumentWatcherTest.kt` | `72d98d841f5f` | 190 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/DocumentsProviderTest.kt` | `66cc1dae8ab2` | 1837 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/FailsafeSessionRequestTest.kt` | `e31bdca47f4b` | 134 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/FontUtilsArchTest.kt` | `93dada4aee40` | 62 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/FontUtilsTest.kt` | `da5cc1b6b275` | 24 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/LauncherAdaptiveIconRobolectricTest.kt` | `58a2a5505fd0` | 173 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ManifestConfigChangesTest.kt` | `6e1325407ac7` | 62 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/NotificationPermissionRequestTest.kt` | `0b8cfba46b62` | 41 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/NotificationSessionSwitchTest.kt` | `1f0966e88686` | 105 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/QueuedPtyWriteTest.kt` | `f86211bfd592` | 49 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SelectionAccentTest.kt` | `0583179718d7` | 46 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SelectionPayloadLimitTest.kt` | `4c6d73c5cb2f` | 126 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SelectionStateTest.kt` | `cafa3cfa8f4a` | 242 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SessionNumberTest.kt` | `ec41e4e81bb0` | 122 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SessionTableWatcherTest.kt` | `45631221eafb` | 36 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ShouldKeepSplashOnScreenTest.kt` | `7ce5e5ffe89a` | 44 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ShouldResetScrollOnInputTest.kt` | `6a328c835afa` | 78 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SplashIconRobolectricTest.kt` | `a32bac406fe5` | 330 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/TerminalAppPowerPreferenceTest.kt` | `3c86a4c4cda3` | 28 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/TerminalAppTest.kt` | `bda169123dc5` | 18 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/TerminalViewModelFixture.kt` | `ba144028a976` | 84 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/TransientMessageChannelTest.kt` | `580f130760d3` | 39 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/AwaitNativeLibraryLoadTest.kt` | `f07fe7db62a8` | 42 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/BridgeFontPathRetryTest.kt` | `3e096a944753` | 22 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/CellSizePackingTest.kt` | `d003622b5e07` | 40 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/CoerceSpToPxScaleTest.kt` | `7098b618e32f` | 118 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/FontInfoPackedTest.kt` | `f11c81380fe4` | 194 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/FontProbeCacheTest.kt` | `8fc037535ac8` | 47 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/NativeBridgeSmokeTest.kt` | `17069c61bdfe` | 538 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/NativeBridgeStaticExportsTest.kt` | `6299bdb60a49` | 38 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/ParseEventTest.kt` | `fa3a613b9e2a` | 66 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/PollEventTest.kt` | `ade7f1701667` | 91 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/PollResultMergeTest.kt` | `708c32cb9139` | 119 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/RenderResultPackingTest.kt` | `d5603186d0d4` | 99 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/TestBridges.kt` | `dbdc2963d48e` | 18 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/ComposingBaselineTest.kt` | `35cd4b162c8a` | 292 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/ImeCursorContractTest.kt` | `af7b7cc96c05` | 172 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/KeyModifiersTest.kt` | `4c625163184a` | 185 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/ModifierStateTest.kt` | `138f553511ff` | 53 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/TerminalEditorInfoTest.kt` | `32dc41572293` | 65 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapArchiveCopyTest.kt` | `e4ef8d59da4e` | 62 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapArchivePathTest.kt` | `919ebb4186c7` | 47 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapDownloaderCancellationTest.kt` | `41c5efa07a6a` | 90 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapDownloaderTest.kt` | `f69ed8366639` | 245 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapInstallerTest.kt` | `9564b78c7c28` | 586 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapOrchestratorTest.kt` | `239dc9fb8dcd` | 132 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapPresetTest.kt` | `530318eb8f34` | 34 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapProgressTest.kt` | `59c5419e36bb` | 79 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/InstalledEntryProbeTest.kt` | `03d9aae97162` | 80 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/PreservedArchiveTest.kt` | `2f79367e4169` | 63 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/TermuxBootstrapTest.kt` | `9621b19e37c8` | 63 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/monitor/RenderWatchDogTest.kt` | `a13e91b09cde` | 261 | 6 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 6 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/monitor/SessionLivenessTest.kt` | `6a799fc588ab` | 198 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/BellPlayerTest.kt` | `6e0952360c41` | 120 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ClipboardAccessTest.kt` | `feed4ffd7b59` | 142 | 3 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 3 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ComputeGridAvailableHeightTest.kt` | `2a42d62cb696` | 63 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ComputeGridDimensionsTest.kt` | `9f31396942ad` | 132 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/CountedLogThrottleTest.kt` | `ad0c88ff69c1` | 60 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/CoverCaptureTest.kt` | `d827c8d40c8e` | 139 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DecideSurfaceRecreateTest.kt` | `ae904ada69cc` | 76 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationDrainTest.kt` | `26ecdc243177` | 139 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationIdTest.kt` | `85260b411ac1` | 98 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationPlayerTest.kt` | `9c9a4c112d2f` | 75 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationPumpTest.kt` | `a2ca34ba88dd` | 98 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationSystemPostTest.kt` | `a07bd9c8ca26` | 70 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DisplayPeriodEstimateTest.kt` | `26bd8e47871a` | 123 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FakeFrameClock.kt` | `d262434e807d` | 26 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FastDeathRecoveryTest.kt` | `c850528cf722` | 63 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FirstOutputReportTest.kt` | `7a0bf947a12a` | 31 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FrameTimingStatsTest.kt` | `d71af3f1bd5c` | 155 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FrameTimingTrendTest.kt` | `8442042a11c5` | 74 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/HoleStateCycleTest.kt` | `ab6e1ac17fc1` | 55 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/HoleStateJournalTest.kt` | `c18af8f8a6c3` | 60 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/InitialSpawnGridTest.kt` | `e3bb396120d9` | 73 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/InputBatchBufferCloseTest.kt` | `07d498bf1402` | 44 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/InputBatchBufferTest.kt` | `db0e38f5b393` | 141 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/InputBatchBufferThreadingTest.kt` | `534879a93716` | 48 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/LogLevelTest.kt` | `b01f2e0e5baa` | 78 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/LogUtilLevelTest.kt` | `7b6dd4fdc994` | 23 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/NormalPathLogSeverityTest.kt` | `c60135bb7005` | 88 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/PasteChunksTest.kt` | `d526c516c294` | 62 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ReadCoverFrameSafelyTest.kt` | `87704b7860a9` | 73 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderErrorBackoffTest.kt` | `e1ec2e523c87` | 31 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderErrorLedgerTest.kt` | `094897521329` | 69 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderLatchCadenceTest.kt` | `fb65472fb846` | 163 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderThreadInterruptionTest.kt` | `841286e73366` | 38 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderThreadShutdownTest.kt` | `2b61ac03747f` | 95 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ScrollRemainderTest.kt` | `97648eb3af25` | 117 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SelectThemeNameTest.kt` | `02eff66c9ffc` | 70 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SessionConfigBuilderShellProbeTest.kt` | `cfab2f908b6b` | 81 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SessionHangVerdictTest.kt` | `9c1417170674` | 94 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SessionRegistryTest.kt` | `9f3b6bf40ac3` | 132 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SessionStatePublicationTest.kt` | `b75498c61803` | 149 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ShouldAlignGridOnSwitchTest.kt` | `7ce689416cbd` | 35 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ShouldResetScrollTest.kt` | `4499add817a4` | 186 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SlowLoopWorkNanosTest.kt` | `459154171d48` | 66 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/StartSurfaceSizeTest.kt` | `e5abd8b52adf` | 29 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SurfacePresentTrackerTest.kt` | `efeaa68ebf7c` | 113 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SurfaceRecoverySettledTest.kt` | `47ac5534b929` | 30 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SurfaceRecreateSeverityTest.kt` | `7bc9fddb934c` | 28 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SwitchSessionTest.kt` | `a032d2b7b0ed` | 94 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/TransientRenderErrorSeverityTest.kt` | `9e60a90aa9d0` | 28 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/VsyncChainStopTest.kt` | `7545448c5a27` | 116 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/VsyncChainTest.kt` | `29718b19aaa1` | 111 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/service/TerminalForegroundServiceTest.kt` | `d91527193f5b` | 357 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/ColorsPropertiesParserTest.kt` | `09080e39a1cf` | 156 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/DefaultFontSizeForTest.kt` | `59d3865d5798` | 49 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/FontSizeRangeTest.kt` | `a84e873537ca` | 279 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/ResolveWorkingDirectoryTest.kt` | `cbcbc9e56aee` | 87 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/SettingsDataStoreProviderTest.kt` | `4acbf85ec505` | 77 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/SettingsRepositoryTest.kt` | `190d6be9237b` | 418 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxMacroParserTest.kt` | `0b4559978b9f` | 129 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesAndroidParserTest.kt` | `aced2ed1aebb` | 58 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesKeyContractTest.kt` | `1fa1a5286ac5` | 259 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesKeysTest.kt` | `cb92bb7a2ed7` | 188 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesParserTest.kt` | `36d13189e0e8` | 529 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesRepositoryTest.kt` | `7bd7b0ab1eee` | 166 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/BackgroundRenderPauseTest.kt` | `ace5735dafab` | 82 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/CjkFallbackStatusRobolectricTest.kt` | `50da6de48ff8` | 102 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ComposingCursorAnchorTest.kt` | `9dcb00e9a792` | 107 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ComposingDiffTest.kt` | `a5dd2a70c748` | 162 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ComputeContentBottomPxTest.kt` | `3927ea63d10b` | 45 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ComputeImeSurfaceShiftTest.kt` | `4973bf09684e` | 152 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/DragTargetCellTest.kt` | `9cb222eb66c4` | 142 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/DrawerSheetInsetRobolectricTest.kt` | `4577da89d532` | 56 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/GridToScreenTest.kt` | `90c7dd0730b9` | 214 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ImeCommitCorrectionTest.kt` | `7c0661e76a18` | 27 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/IsWhitespaceCellTest.kt` | `56a766d7dc32` | 60 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ModifierBarGestureAreaTest.kt` | `2593ba3bf287` | 182 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ModifierBarLayoutTest.kt` | `a835c71c64f1` | 327 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ModifierBarLongPressTest.kt` | `9b23a9f9b7a4` | 28 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ModifierBarRobolectricTest.kt` | `786e89562266` | 479 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/NormalizeSelectionAnchorsTest.kt` | `fe9b550196ec` | 69 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/PopupWindowLivenessTest.kt` | `045dd2f82845` | 49 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/RenderPauseLedgerTest.kt` | `34bef4e06bdc` | 116 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ScrollDistanceTest.kt` | `be40d4b7806a` | 327 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ScrollRebaseTest.kt` | `df6b1838b28b` | 55 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SearchDebouncerTest.kt` | `e5a03473ccd3` | 80 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SearchQueryCoerceTest.kt` | `7deaa171fdfe` | 43 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SelectionMenuActionsTest.kt` | `dab93928a37d` | 21 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SessionDrawerNarrowTest.kt` | `709f4f1304c3` | 211 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SessionItemLabelTest.kt` | `6a150126ddc0` | 46 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SettingsShellSectionRobolectricTest.kt` | `9a9142f6a8a3` | 65 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SettingsSliderRowRobolectricTest.kt` | `5555cbbd8f9b` | 149 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/StartupKeyboardRequestTest.kt` | `f3b37abaf6a6` | 39 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SurfaceAttachGateTest.kt` | `97b34941f44c` | 66 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SurfaceOwnershipTest.kt` | `271f463781f0` | 72 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SystemBarIconThemeTest.kt` | `4b1514db7bbb` | 76 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/TerminalInputEncoderTest.kt` | `05d13f9910e4` | 145 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/TerminalSurfaceLogicTest.kt` | `a671692e788e` | 566 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/TextSearchBarCounterRobolectricTest.kt` | `d9d9ad2a6b7c` | 58 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/theme/TerminalThemePaletteTest.kt` | `30da12fe86a6` | 67 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/theme/ThemeRepositoryTest.kt` | `7b81bf1ea601` | 140 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/RunCatchingTest.kt` | `b09f3ddd7ff8` | 31 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/StreamsTest.kt` | `b5109c0454eb` | 22 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/TextTruncationTest.kt` | `892521db055e` | 60 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/UrlOriginTest.kt` | `6368607e2d2b` | 35 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/WorkingDirectoryTest.kt` | `51ecb8930d1e` | 71 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Gradle build | `android/baselineprofile/build.gradle.kts` | `46b37adca8a7` | 29 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/baselineprofile/src/main/AndroidManifest.xml` | `c8fe08b06e47` | 2 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/baselineprofile/src/main/java/terminal/emulator/baselineprofile/BaselineProfileGenerator.kt` | `b522d1eb69a0` | 92 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Gradle build | `android/benchmark/build.gradle.kts` | `374f4f5fcbf3` | 39 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/benchmarkNix/java/terminal/emulator/benchmark/NixBootstrapTest.kt` | `0f97fe6581e6` | 97 | 1 | 配置参与构建并通过；工具链覆盖；风险模式命中 1 处 |
| Android manifest/resources | `android/benchmark/src/main/AndroidManifest.xml` | `c8fe08b06e47` | 2 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/AppLaunch.kt` | `a2784cb6bde4` | 190 | 1 | 配置参与构建并通过；工具链覆盖；风险模式命中 1 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/AppLaunchTest.kt` | `3a5d54e0fbbd` | 161 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/AppStartupBenchmark.kt` | `3ac9273fba71` | 229 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BackgroundForegroundBenchmark.kt` | `8bf375dfd6aa` | 169 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BackgroundForegroundBenchmarkTest.kt` | `d8a180041251` | 34 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BenchmarkConstants.kt` | `ad913a9e4788` | 463 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BenchmarkSupport.kt` | `70201a7a176b` | 235 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BurstOutputBenchmark.kt` | `697ddc4a7de5` | 317 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BurstOutputLoad.kt` | `3aa43a74b93e` | 76 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BurstOutputLoadTest.kt` | `a69de2dd39c1` | 58 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ContinuousOutputBenchmark.kt` | `d4d9fb08fd74` | 245 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/CrossAppLaunchBenchmark.kt` | `54a0a97e463a` | 108 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/CrossAppTerminalBenchmark.kt` | `be4740b986c5` | 452 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/CrossAppTerminalLoad.kt` | `71359d9551c7` | 222 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/CrossAppTerminalLoadTest.kt` | `814baa5928c1` | 91 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/DeviceEnvironment.kt` | `457270c62354` | 224 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/DeviceEnvironmentTest.kt` | `26e3066a0248` | 212 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/DeviceMemory.kt` | `29bf8fa5f23f` | 47 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/DeviceMemoryTest.kt` | `ed714d6e0426` | 70 | 1 | 配置参与构建并通过；工具链覆盖；风险模式命中 1 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/EnvironmentPinning.kt` | `bb79b1f2f260` | 322 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/EnvironmentPinningTest.kt` | `b6cbddfb2b3d` | 83 | 1 | 配置参与构建并通过；工具链覆盖；风险模式命中 1 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ExitInfoRecords.kt` | `35a4a0e560c7` | 212 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ExitInfoRecordsTest.kt` | `7f53303fbc68` | 158 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/FrameStats.kt` | `6abbb0234152` | 142 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/FrameStatsTest.kt` | `4fa25c8fb898` | 51 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/FrameTimelineFrames.kt` | `e29250cbea08` | 224 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/FrameTimelineFramesTest.kt` | `56d6b5a6086b` | 153 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/HostLoad.kt` | `c930efefd907` | 136 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/HostLoadTest.kt` | `00bedeba62b8` | 117 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/InputFloodBenchmark.kt` | `0c75ce9f1e78` | 89 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/InteractionAnimationBenchmark.kt` | `e306c7238f06` | 188 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/InteractionLatencyBenchmark.kt` | `979e79f9db7d` | 105 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/MeasurementArtifacts.kt` | `03433ccd7bfe` | 256 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/MeasurementArtifactsTest.kt` | `682b87f6076b` | 160 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/MeasurementScenario.kt` | `b24c10030123` | 169 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/MeasurementScenarioTest.kt` | `d952dab87100` | 93 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/OutputLoadCompletion.kt` | `b6925c89691b` | 64 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/OutputScenarioFrames.kt` | `3436252aace7` | 110 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/OutputScenarioFramesTest.kt` | `7d8854a947a7` | 184 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/PresentedFrames.kt` | `7516f71a9b09` | 90 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ProcessCpuTime.kt` | `54873f662393` | 81 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ProcessCpuTimeTest.kt` | `3b29d88d8a01` | 56 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ResourceStressBenchmark.kt` | `6894348f1806` | 104 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ResourceStressBenchmarkTest.kt` | `0d6fd90f9c77` | 27 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioBudget.kt` | `2aa760ccd046` | 74 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioBudgetTest.kt` | `485e3a4bc60b` | 68 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioMatrix.kt` | `12525e7dc1c8` | 191 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioMatrixTest.kt` | `17790c600b68` | 70 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioTarget.kt` | `25fa43f5253a` | 31 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioTargetTest.kt` | `7bf82ff627b1` | 35 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScrollPerformanceBenchmark.kt` | `fd8f280b15e3` | 189 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SessionTitleProbe.kt` | `744e082bc0ae` | 44 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ShellReady.kt` | `ffcd6ed14854` | 66 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SteadyStateBenchmark.kt` | `236e743d9bc4` | 96 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SurfaceLayerFrames.kt` | `fb9374720c59` | 285 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SurfaceLayerFramesTest.kt` | `4e05c7532f17` | 67 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SustainedOutputLoad.kt` | `653425891d3a` | 80 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SustainedOutputLoadTest.kt` | `7838b1076074` | 48 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/WideCharacterBenchmark.kt` | `3e1a9539f902` | 119 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/WideCharacterLoad.kt` | `99f619925830` | 58 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/WideCharacterLoadTest.kt` | `e4f645763fb5` | 72 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Gradle build | `android/build.gradle.kts` | `15e5b859c97d` | 63 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Gradle build | `android/settings.gradle.kts` | `17e2a09094a1` | 22 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/.cargo/config.toml` | `37c7b8dabf47` | 11 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/Cargo.toml` | `d4dc694368af` | 96 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/benches/cell_builder.rs` | `460e4ac2edc5` | 240 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/benches/cjk_resolve.rs` | `bde385a43984` | 45 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/benches/support.rs` | `d4296fe9ed8e` | 88 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/benches/vt_typing.rs` | `410d6ca88981` | 50 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/build.rs` | `d8d978f8fb19` | 12 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi.rs` | `736f87648bdf` | 1042 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 8 处 |
| Rust/native | `native/src/android/ffi/appearance.rs` | `115a93a3076a` | 191 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 6 处 |
| Rust/native | `native/src/android/ffi/busy_marker_tests.rs` | `2dba60945573` | 84 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/cell_size_packing_tests.rs` | `516ab7a4faff` | 25 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/clipboard_answer_tests.rs` | `093339b01f72` | 63 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/clipboard_read_flood_tests.rs` | `d9a13388e0e4` | 41 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/cursor_anchor_tests.rs` | `53bf27909e34` | 37 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/font.rs` | `12585a738fb5` | 501 | 14 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 14 处 |
| Rust/native | `native/src/android/ffi/font_info_packing_tests.rs` | `14ac365b77ff` | 33 | 5 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 5 处 |
| Rust/native | `native/src/android/ffi/font_size_cap_tests.rs` | `ff02e9be1dee` | 118 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/idle_repaint_gate_tests.rs` | `72a82fe2030f` | 43 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/input.rs` | `3ede14f757a0` | 670 | 16 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 16 处 |
| Rust/native | `native/src/android/ffi/kitty_placement_keys_tests.rs` | `3e208b2cadac` | 182 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/notification_poll_tests.rs` | `8d344dc8704a` | 49 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/panic_payload_tests.rs` | `05d1fc4a91fc` | 54 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/query.rs` | `05d0a075b97b` | 482 | 17 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 17 处 |
| Rust/native | `native/src/android/ffi/render/clipboard.rs` | `df86a5dbba7d` | 283 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/render/control.rs` | `71d835384947` | 92 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 3 处 |
| Rust/native | `native/src/android/ffi/render/diagnostics.rs` | `22ed966de8bc` | 167 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/render/frame.rs` | `af85ab356cf9` | 960 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 3 处 |
| Rust/native | `native/src/android/ffi/render/mod.rs` | `91c9435c66b2` | 26 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/render/overlay.rs` | `3d04f44279e5` | 292 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 2 处 |
| Rust/native | `native/src/android/ffi/render/pump.rs` | `f9b63732a14b` | 144 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/render/session_ops.rs` | `946e04a4bd5d` | 196 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 6 处 |
| Rust/native | `native/src/android/ffi/render/window.rs` | `b42af4e3e911` | 296 | 14 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 14 处 |
| Rust/native | `native/src/android/ffi/render_sample_bits_tests.rs` | `e5422540232a` | 93 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/render_wake_tests.rs` | `6eb7cdb88e45` | 46 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/scroll_bookkeeping_tests.rs` | `6bcad3d8b01c` | 82 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/scroll_frame_staleness_tests.rs` | `152d53685048` | 72 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/search_highlight_repaint_tests.rs` | `b1ad2123d3dd` | 42 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/search_highlight_tests.rs` | `a965bb392e28` | 141 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/android/ffi/session.rs` | `e9762836b0b6` | 299 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 4 处 |
| Rust/native | `native/src/android/ffi/session_entry_flag_tests.rs` | `de2a233a51c8` | 69 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/session_registry_tests.rs` | `bdc8f7907899` | 284 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 3 处 |
| Rust/native | `native/src/android/ffi/session_scoped_tests.rs` | `430260d02713` | 42 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/state.rs` | `80bb49233d65` | 358 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 8 处 |
| Rust/native | `native/src/android/ffi/wake.rs` | `8a40c958fef3` | 162 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/android/keymap.rs` | `89922f58544b` | 227 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/logging.rs` | `69f9c943c33d` | 155 | 9 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 9 处 |
| Rust/native | `native/src/android/mod.rs` | `74b6b5d71be1` | 191 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/text_utils.rs` | `5f281223dca4` | 34 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/event.rs` | `2c5be876232d` | 489 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 6 处 |
| Rust/native | `native/src/lib.rs` | `caead612b8d5` | 147 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/cell_builder.rs` | `05ee6e282800` | 886 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/cell_builder/emit.rs` | `a7498d7f67dc` | 386 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/cell_builder/tests/appearance.rs` | `56ef089b9c63` | 440 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/cell_builder/tests/cache.rs` | `8848102b7e80` | 798 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/render/cell_builder/tests/mod.rs` | `d68db4189d6b` | 61 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/atlas.rs` | `2eebc67d34a0` | 201 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/cover.rs` | `d31695090c9b` | 281 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/gpu.rs` | `18c87cef85db` | 585 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 2 处 |
| Rust/native | `native/src/render/context/mod.rs` | `c4948993e22e` | 159 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/poll.rs` | `fa176e176d4b` | 264 | 9 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 9 处 |
| Rust/native | `native/src/render/context/renderer.rs` | `1102958bb0b6` | 849 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/context/surface.rs` | `c665d01c514d` | 236 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/tests.rs` | `b947f0297d73` | 253 | 5 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 5 处 |
| Rust/native | `native/src/render/font/atlas.rs` | `72446bc967cd` | 345 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/cjk.rs` | `6e6427e969c0` | 269 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/font_db.rs` | `dd1f6e25ba4e` | 978 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/render/font/font_db/tests.rs` | `5eb9e06a41d2` | 861 | 26 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 26 处 |
| Rust/native | `native/src/render/font/glyph_cache.rs` | `c2c08a38f0a0` | 224 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 8 处 |
| Rust/native | `native/src/render/font/mod.rs` | `3b7255be390b` | 218 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/pipeline.rs` | `2ff48337274f` | 956 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/pipeline/families.rs` | `37faa080adbb` | 680 | 5 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 5 处 |
| Rust/native | `native/src/render/font/rasterization.rs` | `f2e1f2a2477d` | 213 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/render/font/shaping.rs` | `bcd57825573e` | 374 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/tests/fallback.rs` | `39d8b20b3202` | 744 | 39 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 39 处 |
| Rust/native | `native/src/render/font/tests/glyphs.rs` | `350242781a8e` | 1073 | 28 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 28 处 |
| Rust/native | `native/src/render/font/tests/loading.rs` | `569eb9f2fa3e` | 624 | 44 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 44 处 |
| Rust/native | `native/src/render/font/tests/mod.rs` | `250329e69ecc` | 139 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/render/kitty.rs` | `c86a93094cc9` | 637 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 7 处 |
| Rust/native | `native/src/render/mod.rs` | `874aae87537d` | 222 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/acquire.rs` | `d87791076b4e` | 480 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/pass/bands.rs` | `36215e810250` | 250 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/frame.rs` | `266120c1f182` | 685 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/mod.rs` | `361d03356bd4` | 15 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/readback.rs` | `45094a67cb0a` | 248 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/tests.rs` | `32c0c7cf82ab` | 791 | 10 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 10 处 |
| Rust/native | `native/src/render/pipeline.rs` | `5ec0ac9bc62c` | 346 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/render/tests/cell_pipeline.rs` | `7ff68a5ac824` | 992 | 10 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 10 处 |
| Rust/native | `native/src/render/tests/cover.rs` | `4f99638ecc51` | 199 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 7 处 |
| Rust/native | `native/src/render/tests/frame_and_atlas.rs` | `85fede96dacc` | 830 | 15 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 15 处 |
| Rust/native | `native/src/render/tests/gpu_core.rs` | `50106ee51178` | 948 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 7 处 |
| Rust/native | `native/src/render/tests/mod.rs` | `11a05c23d514` | 464 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 3 处 |
| Rust/native | `native/src/render/wgpu_backend.rs` | `37030a14d5d5` | 223 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/slow_log.rs` | `0aa2a1cdb926` | 366 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 3 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/commands.rs` | `e5dd3bdd13ae` | 374 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/frame_mailbox.rs` | `af4ff02986be` | 72 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/input_output_tests.rs` | `a0eb3b58e05d` | 549 | 34 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 34 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/commands.rs` | `104478346f18` | 435 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/dispatch.rs` | `ca8dd035ecac` | 448 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/input_filter.rs` | `993cbc2281b5` | 779 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/input_filter/tests.rs` | `15cbaced53ae` | 788 | 29 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 29 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/kitty.rs` | `1e1811d57490` | 430 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/mod.rs` | `04acc9db2668` | 740 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/mouse_input.rs` | `d305b15f7edf` | 71 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/rendering.rs` | `b011f5f99c1b` | 810 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/run.rs` | `10732e39cc2f` | 550 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/selection.rs` | `12c81e3a830a` | 517 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/tests.rs` | `c15a41e8a1be` | 291 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 6 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/key_encoding_tests.rs` | `98781c2eabaf` | 454 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/mod.rs` | `46ea831ed252` | 160 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/notification_queue.rs` | `046073b82be7` | 94 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api.rs` | `22a88ad7b488` | 19 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/commands.rs` | `7908a47ce798` | 239 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/construction.rs` | `870d165755cb` | 246 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/encoding.rs` | `782922f9aaef` | 211 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/events.rs` | `11cb9cfcc2c0` | 30 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/frames.rs` | `95fb531c8b8a` | 77 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/queries.rs` | `c6e9aa3437cc` | 382 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/snapshot_cache_unit_tests.rs` | `b7fae525e0b5` | 18 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/batch_latency.rs` | `0c1560ef273d` | 133 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/grid_text.rs` | `11ca5b1df2ed` | 1029 | 41 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 41 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/kitty_graphics.rs` | `77fed2386b13` | 442 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/kitty_simulator.rs` | `ed573989f020` | 320 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/lifecycle.rs` | `c652ae047b31` | 1235 | 66 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 66 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/mod.rs` | `564478aed9c0` | 254 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 8 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/mouse_osc.rs` | `b50035bd762b` | 875 | 25 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 25 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/osc_notification.rs` | `27b45fe23482` | 123 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/osc_state.rs` | `2733de87aedf` | 128 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/render_cache.rs` | `81f3fad77886` | 1120 | 64 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 64 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/selection_search.rs` | `b5933c7094af` | 782 | 30 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 30 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/types.rs` | `1d7a9be1cebe` | 438 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/mock_pty.rs` | `d06dde521e57` | 302 | 23 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 23 处 |
| Rust/native | `native/src/terminal/mod.rs` | `ebfe96505279` | 56 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/output_processor.rs` | `d9190c9d8832` | 634 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/terminal/pty.rs` | `826e1d600d50` | 880 | 26 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 26 处 |
| Rust/native | `native/src/terminal/pty/child.rs` | `07761b39c082` | 179 | 9 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 9 处 |
| Rust/native | `native/src/terminal/pty/env.rs` | `6a5f1653c7a4` | 103 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/pty/tests.rs` | `3ba91e542f19` | 848 | 66 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 66 处 |
| Rust/native | `native/src/terminal/session/lifecycle.rs` | `eb20b816b89d` | 227 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/session/mod.rs` | `42d3673e34f6` | 392 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/session/output.rs` | `375190745e0f` | 465 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/session/pty_master.rs` | `fa35f6b2d1c1` | 123 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/session/spawn.rs` | `5cf8afafb408` | 305 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/terminal/session/tests/basics.rs` | `5a50839a34b1` | 97 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/terminal/session/tests/clipboard.rs` | `2db50d593417` | 298 | 9 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 9 处 |
| Rust/native | `native/src/terminal/session/tests/focus.rs` | `69343fb4ca7e` | 44 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/session/tests/mod.rs` | `f5312b548744` | 132 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 7 处 |
| Rust/native | `native/src/terminal/session/tests/process.rs` | `2f85f469e700` | 230 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 6 处 |
| Rust/native | `native/src/terminal/session/tests/resize.rs` | `c576c02b9b8a` | 153 | 9 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 9 处 |
| Rust/native | `native/src/terminal/session/tests/scrollback.rs` | `f5a9f1617bd9` | 69 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/session/tests/write_backpressure.rs` | `4274eb047558` | 575 | 26 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 26 处 |
| Rust/native | `native/src/terminal/shell_env.rs` | `5c5b308f73a1` | 95 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/test_helpers.rs` | `174c3c36ba4a` | 784 | 23 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 23 处 |
| Rust/native | `native/src/terminal/theme.rs` | `9b62e72877ce` | 478 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 3 处 |
| Rust/native | `native/src/terminal/theme/tests.rs` | `0ae081cd951d` | 541 | 10 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 10 处 |
| Rust/native | `native/tests/alloc_budget.rs` | `d9fc87f4209c` | 164 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 8 处 |
| Rust/native | `native/tests/concurrency.rs` | `b5e41f51a9d3` | 111 | 5 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 5 处 |
| Rust/native | `native/tests/instance_alloc_budget.rs` | `6688febe540b` | 172 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 7 处 |
| Config | `rust-toolchain.toml` | `051175cb0044` | 4 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/build-android-libs.nu` | `569fbcd94118` | 47 | 1 | 构建脚本执行通过；工具链覆盖；风险模式命中 1 处 |
| Build scripts | `scripts/build-apk.nu` | `58aad169afcf` | 41 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/check.nu` | `7fd25ad8a7c3` | 19 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/download-aosp-testkey.nu` | `2a7d3d68af4e` | 23 | 1 | 构建脚本执行通过；工具链覆盖；风险模式命中 1 处 |
| Build scripts | `scripts/download-rapidocr-models.nu` | `e59bc38509b0` | 10 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/setup-emulator.nu` | `92329fb548db` | 110 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/test-emulator.nu` | `823858178774` | 14 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |

## 汇总

- 清单文件数：624（Rust 157；Kotlin 含主代码/测试 277；Gradle 5；Android 资源/清单 22；R8 规则 1；配置 65；构建脚本 7）。
- `jscpd` 生产代码扫描范围：Rust 145 个文件、53,413 行；Kotlin 115 个文件、30,334 行。
- 重复度：`jscpd` 阈值 8 行/80 token，Rust 0 处克隆，Kotlin 1 处 11 行 import 重复。
- 全部命中项的工具链与测试证据见 `docs/REVIEW_LOG.md`。
