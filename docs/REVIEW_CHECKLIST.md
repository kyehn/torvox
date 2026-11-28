# 逐文件审查清单

本清单由 `git ls-files` 枚举现行受控代码文件生成，不以 diff 或某次提交为界。“风险模式命中”是二次扫描的提示计数，不等于问题数；测试文件中的 `unwrap`/`expect` 多为断言，生产代码中的 `unsafe`/`allow` 均已逐处核对前置条件。命中项均已被对应工具链与测试覆盖，人工重点复核的文件另行标注。

| 模块 | 文件 | 行数 | 风险命中 | 核验结论 |
| --- | --- | ---: | ---: | --- |
| Config | `.cargo/config.toml` | 15 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Gradle build | `android/app/build.gradle.kts` | 262 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/lint.xml` | 5 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| R8/ProGuard | `android/app/proguard-rules.pro` | 22 | 7 | assembleRelease/analyzeReleaseR8Config 通过；人工重点复核；风险模式命中 7 处 |
| Android instrumentation | `android/app/src/androidTest/AndroidManifest.xml` | 8 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/com/termux/test/CucumberOptionsClass.kt` | 25 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/BehaviorInstrumentedTest.kt` | 239 | 3 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 3 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/BehaviorVerificationTest.kt` | 163 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/DesktopNotificationInstrumentedTest.kt` | 72 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/DocumentsProviderInstrumentedTest.kt` | 1016 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/FontFileOverrideInstrumentedTest.kt` | 86 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/FontSwitchInstrumentedTest.kt` | 120 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/MainActivitySingleInstanceTest.kt` | 57 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ScrollBehaviorQuantifiedTest.kt` | 384 | 10 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 10 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/SelectionDragQuantifiedTest.kt` | 744 | 18 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 18 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/SelectionEspressoTest.kt` | 463 | 6 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 6 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ShellResponseLatencyTest.kt` | 128 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/StorageAccessInstrumentedTest.kt` | 96 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/TerminalLogcatRule.kt` | 159 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/TestUtils.kt` | 817 | 12 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 12 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ThemeApplyOrderInstrumentedTest.kt` | 211 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ThemeFollowSystemInstrumentedTest.kt` | 289 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ThemeInstrumentedTest.kt` | 150 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/UxTestUtils.kt` | 568 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/bridge/NativePreloadInstrumentedTest.kt` | 40 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/ComposeRuleHolder.kt` | 26 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/Hooks.kt` | 38 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/SimpleHiltObjectFactory.kt` | 55 | 3 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 3 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/CommonSteps.kt` | 36 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/FontSteps.kt` | 63 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/LifecycleSteps.kt` | 59 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/ModifierBarSteps.kt` | 99 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/NavigationSteps.kt` | 140 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/SearchSteps.kt` | 91 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/SelectionSteps.kt` | 244 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/SessionSteps.kt` | 120 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/SettingsSteps.kt` | 84 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/TerminalLaunchSteps.kt` | 48 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/cucumber/steps/ThemeSteps.kt` | 91 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/diag/CursorPixelAcceptanceTest.kt` | 122 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/diag/SelectionTapDismissTest.kt` | 124 | 3 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 3 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/diag/SgrColorPixelAcceptanceTest.kt` | 125 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/diag/SgrItalicPixelAcceptanceTest.kt` | 90 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/BootstrapCompatibilityTest.kt` | 341 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/BootstrapFixture.kt` | 56 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/BootstrapSymlinkInstrumentedTest.kt` | 125 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/TerminalPrefix.kt` | 67 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/installer/TermuxBootstrapRealTerminalTest.kt` | 214 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/leakcanary/LeakCanaryIntegrationTest.kt` | 20 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/performance/RenderPauseSemanticsTest.kt` | 56 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/performance/RenderPauseTest.kt` | 42 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/AltScreenImeReflowInstrumentedTest.kt` | 363 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/BackgroundResumeFlickerInstrumentedTest.kt` | 517 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/BackspaceLatencyTest.kt` | 244 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/BootstrapInstrumentedTest.kt` | 102 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/CjkBackspaceSemanticsTest.kt` | 60 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/CjkComposingLatencyTest.kt` | 194 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/CjkPresentSemanticsTest.kt` | 34 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/DrawerWidthInstrumentedTest.kt` | 76 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/FontSizeReflowInstrumentedTest.kt` | 332 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/HardKeyboardMultipleArrivalTest.kt` | 99 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/IdleOutputWakeTest.kt` | 197 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ImeBatchEditDeliveryTest.kt` | 85 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ImePopupPixelInstrumentedTest.kt` | 672 | 13 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 13 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ImeSurfaceStabilityInstrumentedTest.kt` | 120 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/KittyGraphicsOverlayTest.kt` | 193 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/KittyGraphicsPlacementTest.kt` | 475 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/KittyRealClientIcatTest.kt` | 280 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ModifierBarBottomInsetInstrumentedTest.kt` | 113 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ModifierBarGapInstrumentedTest.kt` | 131 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ModifierBarPressFeedbackInstrumentedTest.kt` | 341 | 3 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 3 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ModifierBarTest.kt` | 344 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/MouseReportingInstrumentedTest.kt` | 349 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/Osc52ClipboardInstrumentedTest.kt` | 113 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/PasteButtonInstrumentedTest.kt` | 190 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/PreviewScreenshotTest.kt` | 312 | 4 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 4 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/PtyWriteQueueBindingInstrumentedTest.kt` | 65 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SessionCreationInstrumentedTest.kt` | 128 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SessionDrawerInstrumentedTest.kt` | 206 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SettingsScreenTest.kt` | 131 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ShellPtyInstrumentedTest.kt` | 221 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/StickyCtrlInterruptInstrumentedTest.kt` | 67 | 2 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 2 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SurfaceLossRecoveryInstrumentedTest.kt` | 647 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SurfaceRedrawAsyncInstrumentedTest.kt` | 37 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/SystemBarIconInstrumentedTest.kt` | 115 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TeardownLeakInstrumentedTest.kt` | 92 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TerminalHoleCoverInstrumentedTest.kt` | 371 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TerminalHoleTransitionFillInstrumentedTest.kt` | 362 | 6 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 6 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TerminalScreenLayoutTest.kt` | 46 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/TextSearchEndToEndTest.kt` | 490 | 0 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 0 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/UiAutomatorTest.kt` | 216 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/VisualInlineVerificationTest.kt` | 455 | 5 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 5 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/VtCorrectnessInstrumentedTest.kt` | 373 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android instrumentation | `android/app/src/androidTest/java/terminal/emulator/ui/ZoomPreviewGestureInstrumentedTest.kt` | 254 | 1 | connectedAndroidTest 结果见验证章节；工具链覆盖；风险模式命中 1 处 |
| Android manifest/resources | `android/app/src/debug/AndroidManifest.xml` | 29 | 1 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 1 处 |
| Android manifest/resources | `android/app/src/main/AndroidManifest.xml` | 132 | 2 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/BootstrapManager.kt` | 215 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/DocumentMutations.kt` | 617 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/DocumentQueries.kt` | 210 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/DocumentWatcher.kt` | 231 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/FontManager.kt` | 226 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/FontUtils.kt` | 47 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/MainActivity.kt` | 670 | 3 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 3 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/SelectionManager.kt` | 494 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/SelectionState.kt` | 155 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/SessionState.kt` | 88 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/TerminalApp.kt` | 131 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/TerminalDocumentsProvider.kt` | 676 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/TerminalViewModel.kt` | 1230 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/ThemeManager.kt` | 49 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/TransientMessageChannel.kt` | 42 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/Bridge.kt` | 943 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/FontInfo.kt` | 164 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/NativeBridge.kt` | 637 | 4 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 4 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/NativeQueryPort.kt` | 122 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/PollEvent.kt` | 76 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/bridge/RenderContract.kt` | 87 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/ComposingBaseline.kt` | 149 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/ImeCursorContract.kt` | 129 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/KeyModifiers.kt` | 99 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/ModifierState.kt` | 33 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/input/TerminalEditorInfo.kt` | 30 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapDownloader.kt` | 183 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapInstallService.kt` | 89 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapInstaller.kt` | 520 | 2 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapOrchestrator.kt` | 114 | 0 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapPreset.kt` | 31 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/BootstrapProgress.kt` | 41 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/InstalledEntryProbe.kt` | 45 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/NixOnDroidBootstrap.kt` | 26 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/installer/TermuxBootstrap.kt` | 31 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/monitor/RenderWatchDog.kt` | 112 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/monitor/SessionLiveness.kt` | 165 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/BellPlayer.kt` | 110 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/ClipboardAccess.kt` | 76 | 2 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/CountedLogThrottle.kt` | 50 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/CoverCapture.kt` | 221 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/DesktopNotificationDrain.kt` | 66 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/DesktopNotificationPlayer.kt` | 173 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/DesktopNotificationPump.kt` | 63 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/FrameClock.kt` | 27 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/FrameTimingStats.kt` | 126 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/FrameTimingTrend.kt` | 55 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/GridGeometry.kt` | 135 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/HoleStateJournal.kt` | 118 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/InputBatchBuffer.kt` | 202 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/LogLevel.kt` | 42 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/LogUtil.kt` | 53 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/PasteChunks.kt` | 59 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/RenderErrorLedger.kt` | 74 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/RenderFailurePolicy.kt` | 163 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/RenderSupervisor.kt` | 1362 | 3 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 3 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/ScrollFreshness.kt` | 40 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionBridgeCloser.kt` | 36 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionConfigBuilder.kt` | 136 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionEntry.kt` | 287 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionRegistry.kt` | 121 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SessionSwitchPolicy.kt` | 34 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/SurfacePresentTracker.kt` | 130 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntime.kt` | 3529 | 2 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/TerminalRuntimeEntryPoint.kt` | 18 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/runtime/VsyncChain.kt` | 241 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/service/TerminalForegroundService.kt` | 314 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/ColorsProperties.kt` | 167 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/SettingsDataStoreProvider.kt` | 50 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/SettingsRepository.kt` | 572 | 2 | detekt/spotless/lint/单元测试通过；人工重点复核；风险模式命中 2 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/SystemFonts.kt` | 24 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/TerminalKeyCatalog.kt` | 168 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/TermuxMacro.kt` | 163 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/TermuxProperties.kt` | 846 | 3 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 3 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/settings/TermuxPropertiesRepository.kt` | 174 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/BackgroundRenderPause.kt` | 63 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ComposingDiff.kt` | 92 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/HandleDragController.kt` | 300 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ImeConnection.kt` | 577 | 3 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 3 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ImeShiftController.kt` | 458 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ModifierBar.kt` | 1186 | 2 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 2 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/PopupWindowLiveness.kt` | 69 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/RenderPauseLedger.kt` | 56 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/ResizeManager.kt` | 312 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SearchDebouncer.kt` | 57 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SearchQuery.kt` | 12 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SelectionHandles.kt` | 484 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SelectionMenu.kt` | 433 | 1 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 1 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SessionDrawer.kt` | 406 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsAppearanceSection.kt` | 265 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsBootstrapSection.kt` | 362 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsClearDataSection.kt` | 94 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsComponents.kt` | 349 | 1 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 1 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsLogLevelSection.kt` | 90 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsScreen.kt` | 251 | 1 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 1 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsShellSection.kt` | 129 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SettingsThemeSection.kt` | 312 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SurfaceAttachGate.kt` | 54 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SurfaceInput.kt` | 917 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SurfaceOwnership.kt` | 39 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/SystemBars.kt` | 84 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TerminalInputEncoder.kt` | 153 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TerminalScreen.kt` | 1082 | 2 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 2 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt` | 1348 | 1 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；人工重点复核；风险模式命中 1 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TerminalSurfaceMath.kt` | 508 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/TextSearchBar.kt` | 223 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/theme/TerminalTheme.kt` | 146 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Compose UI | `android/app/src/main/java/terminal/emulator/ui/theme/ThemeRepository.kt` | 143 | 0 | detekt/spotless/lint/单元测试通过；Compose 与语义在 instrumentation 中覆盖；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/DirectoryListing.kt` | 14 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/RunCatching.kt` | 19 | 4 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 4 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/Streams.kt` | 19 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/TerminalDispatchers.kt` | 15 | 1 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/TextTruncation.kt` | 46 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/UrlOrigin.kt` | 17 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin main | `android/app/src/main/java/terminal/emulator/util/WorkingDirectory.kt` | 27 | 0 | detekt/spotless/lint/单元测试通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/ic_launcher_background.xml` | 12 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/ic_new_session.xml` | 17 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/ic_notification.xml` | 40 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/launcher_foreground.xml` | 12 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/launcher_monochrome.xml` | 88 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/logo_art.xml` | 125 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/splash_frame.xml` | 15 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/drawable/splash_icon.xml` | 21 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/mipmap/ic_launcher.xml` | 6 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/mipmap/ic_launcher_round.xml` | 6 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/values/colors.xml` | 11 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/values/strings.xml` | 139 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/values/themes.xml` | 35 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/xml/data_extraction_rules.xml` | 25 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/main/res/xml/shortcuts.xml` | 26 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/nix/res/values/strings.xml` | 8 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/app/src/nix/res/xml/shortcuts.xml` | 19 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/AndroidManifest.xml` | 21 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/AwaitBridgeReadyTest.kt` | 41 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ConfigurationChangeContractTest.kt` | 77 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/DeadKeyLogSeverityTest.kt` | 81 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/DocumentWatcherTest.kt` | 190 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/DocumentsProviderTest.kt` | 1837 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/FailsafeSessionRequestTest.kt` | 134 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/FontUtilsArchTest.kt` | 62 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/FontUtilsTest.kt` | 24 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/LauncherAdaptiveIconRobolectricTest.kt` | 173 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ManifestConfigChangesTest.kt` | 62 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/NotificationPermissionRequestTest.kt` | 41 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/NotificationSessionSwitchTest.kt` | 105 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/QueuedPtyWriteTest.kt` | 49 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SelectionAccentTest.kt` | 46 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SelectionPayloadLimitTest.kt` | 126 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SelectionStateTest.kt` | 242 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SessionNumberTest.kt` | 122 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SessionTableWatcherTest.kt` | 36 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ShouldKeepSplashOnScreenTest.kt` | 44 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ShouldResetScrollOnInputTest.kt` | 78 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/SplashIconRobolectricTest.kt` | 330 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/TerminalAppPowerPreferenceTest.kt` | 28 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/TerminalAppTest.kt` | 18 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/TerminalViewModelFixture.kt` | 84 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/TransientMessageChannelTest.kt` | 39 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/AwaitNativeLibraryLoadTest.kt` | 42 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/BridgeFontPathRetryTest.kt` | 22 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/CellSizePackingTest.kt` | 40 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/CoerceSpToPxScaleTest.kt` | 118 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/FontInfoPackedTest.kt` | 194 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/FontProbeCacheTest.kt` | 47 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/NativeBridgeSmokeTest.kt` | 538 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/NativeBridgeStaticExportsTest.kt` | 38 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/ParseEventTest.kt` | 66 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/PollEventTest.kt` | 91 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/PollResultMergeTest.kt` | 119 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/RenderResultPackingTest.kt` | 99 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/bridge/TestBridges.kt` | 18 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/ComposingBaselineTest.kt` | 292 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/ImeCursorContractTest.kt` | 172 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/KeyModifiersTest.kt` | 185 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/ModifierStateTest.kt` | 53 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/input/TerminalEditorInfoTest.kt` | 65 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapArchiveCopyTest.kt` | 62 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapArchivePathTest.kt` | 47 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapDownloaderCancellationTest.kt` | 90 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapDownloaderTest.kt` | 282 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapInstallerTest.kt` | 601 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapOrchestratorTest.kt` | 132 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapPresetTest.kt` | 34 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/BootstrapProgressTest.kt` | 79 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/InstalledEntryProbeTest.kt` | 80 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/PreservedArchiveTest.kt` | 63 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/installer/TermuxBootstrapTest.kt` | 63 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/monitor/RenderWatchDogTest.kt` | 261 | 6 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 6 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/monitor/SessionLivenessTest.kt` | 198 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/BellPlayerTest.kt` | 120 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ClipboardAccessTest.kt` | 142 | 3 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 3 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ComputeGridAvailableHeightTest.kt` | 63 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ComputeGridDimensionsTest.kt` | 132 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/CountedLogThrottleTest.kt` | 60 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/CoverCaptureTest.kt` | 139 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DecideSurfaceRecreateTest.kt` | 76 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationDrainTest.kt` | 139 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationIdTest.kt` | 98 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationPlayerTest.kt` | 75 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationPumpTest.kt` | 98 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DesktopNotificationSystemPostTest.kt` | 70 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/DisplayPeriodEstimateTest.kt` | 123 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FakeFrameClock.kt` | 26 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FastDeathRecoveryTest.kt` | 63 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FirstOutputReportTest.kt` | 31 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FrameTimingStatsTest.kt` | 155 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/FrameTimingTrendTest.kt` | 74 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/HoleStateCycleTest.kt` | 55 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/HoleStateJournalTest.kt` | 60 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/InitialSpawnGridTest.kt` | 73 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/InputBatchBufferCloseTest.kt` | 44 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/InputBatchBufferTest.kt` | 141 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/InputBatchBufferThreadingTest.kt` | 48 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/LogLevelTest.kt` | 78 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/LogUtilLevelTest.kt` | 23 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/NormalPathLogSeverityTest.kt` | 88 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/PasteChunksTest.kt` | 68 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ReadCoverFrameSafelyTest.kt` | 73 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderErrorBackoffTest.kt` | 31 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderErrorLedgerTest.kt` | 69 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderLatchCadenceTest.kt` | 163 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderThreadInterruptionTest.kt` | 38 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/RenderThreadShutdownTest.kt` | 95 | 1 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 1 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ScrollRemainderTest.kt` | 117 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SelectThemeNameTest.kt` | 70 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SessionConfigBuilderShellProbeTest.kt` | 81 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SessionHangVerdictTest.kt` | 94 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SessionRegistryTest.kt` | 132 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SessionStatePublicationTest.kt` | 149 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ShouldAlignGridOnSwitchTest.kt` | 35 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/ShouldResetScrollTest.kt` | 186 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SlowLoopWorkNanosTest.kt` | 66 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/StartSurfaceSizeTest.kt` | 29 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SurfacePresentTrackerTest.kt` | 113 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SurfaceRecoverySettledTest.kt` | 30 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SurfaceRecreateSeverityTest.kt` | 28 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/SwitchSessionTest.kt` | 94 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/TransientRenderErrorSeverityTest.kt` | 28 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/VsyncChainStopTest.kt` | 116 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/runtime/VsyncChainTest.kt` | 111 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/service/TerminalForegroundServiceTest.kt` | 378 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/ColorsPropertiesParserTest.kt` | 156 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/DefaultFontSizeForTest.kt` | 49 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/FontSizeRangeTest.kt` | 279 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/ResolveWorkingDirectoryTest.kt` | 87 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/SettingsDataStoreProviderTest.kt` | 77 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/SettingsRepositoryTest.kt` | 418 | 2 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 2 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxMacroParserTest.kt` | 129 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesAndroidParserTest.kt` | 58 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesKeyContractTest.kt` | 259 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesKeysTest.kt` | 188 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesParserTest.kt` | 529 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/settings/TermuxPropertiesRepositoryTest.kt` | 166 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/BackgroundRenderPauseTest.kt` | 82 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/CjkFallbackStatusRobolectricTest.kt` | 102 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ComposingCursorAnchorTest.kt` | 107 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ComposingDiffTest.kt` | 162 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ComputeContentBottomPxTest.kt` | 45 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ComputeImeSurfaceShiftTest.kt` | 152 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/DragTargetCellTest.kt` | 142 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/DrawerSheetInsetRobolectricTest.kt` | 56 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/GridToScreenTest.kt` | 214 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ImeCommitCorrectionTest.kt` | 27 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/IsWhitespaceCellTest.kt` | 60 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ModifierBarGestureAreaTest.kt` | 182 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ModifierBarLayoutTest.kt` | 327 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ModifierBarLongPressTest.kt` | 28 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ModifierBarRobolectricTest.kt` | 479 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/NormalizeSelectionAnchorsTest.kt` | 69 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/PopupWindowLivenessTest.kt` | 49 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/RenderPauseLedgerTest.kt` | 116 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ScrollDistanceTest.kt` | 327 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/ScrollRebaseTest.kt` | 55 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SearchDebouncerTest.kt` | 80 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SearchQueryCoerceTest.kt` | 43 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SelectionMenuActionsTest.kt` | 21 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SessionDrawerNarrowTest.kt` | 211 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SessionItemLabelTest.kt` | 46 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SettingsShellSectionRobolectricTest.kt` | 65 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SettingsSliderRowRobolectricTest.kt` | 149 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/StartupKeyboardRequestTest.kt` | 39 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SurfaceAttachGateTest.kt` | 66 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SurfaceOwnershipTest.kt` | 72 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/SystemBarIconThemeTest.kt` | 76 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/TerminalInputEncoderTest.kt` | 145 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/TerminalSurfaceLogicTest.kt` | 566 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/TextSearchBarCounterRobolectricTest.kt` | 58 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/WheelForwardingTest.kt` | 35 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/theme/TerminalThemePaletteTest.kt` | 67 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/ui/theme/ThemeRepositoryTest.kt` | 140 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/RunCatchingTest.kt` | 31 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/StreamsTest.kt` | 22 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/TextTruncationTest.kt` | 60 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/UrlOriginTest.kt` | 35 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Kotlin unit tests | `android/app/src/test/java/terminal/emulator/util/WorkingDirectoryTest.kt` | 71 | 0 | testDebugUnitTest 通过；工具链覆盖；风险模式命中 0 处 |
| Gradle build | `android/baselineprofile/build.gradle.kts` | 29 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Android manifest/resources | `android/baselineprofile/src/main/AndroidManifest.xml` | 2 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/baselineprofile/src/main/java/terminal/emulator/baselineprofile/BaselineProfileGenerator.kt` | 92 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Gradle build | `android/benchmark/build.gradle.kts` | 39 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/benchmarkNix/java/terminal/emulator/benchmark/NixBootstrapTest.kt` | 97 | 1 | 配置参与构建并通过；工具链覆盖；风险模式命中 1 处 |
| Android manifest/resources | `android/benchmark/src/main/AndroidManifest.xml` | 2 | 0 | lintDebug/lintVitalRelease 通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/AppLaunch.kt` | 190 | 1 | 配置参与构建并通过；工具链覆盖；风险模式命中 1 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/AppLaunchTest.kt` | 161 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/AppStartupBenchmark.kt` | 229 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BackgroundForegroundBenchmark.kt` | 169 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BackgroundForegroundBenchmarkTest.kt` | 34 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BenchmarkConstants.kt` | 463 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BenchmarkSupport.kt` | 235 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BurstOutputBenchmark.kt` | 317 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BurstOutputLoad.kt` | 76 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/BurstOutputLoadTest.kt` | 58 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ContinuousOutputBenchmark.kt` | 245 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/CrossAppLaunchBenchmark.kt` | 108 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/CrossAppTerminalBenchmark.kt` | 452 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/CrossAppTerminalLoad.kt` | 222 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/CrossAppTerminalLoadTest.kt` | 91 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/DeviceEnvironment.kt` | 224 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/DeviceEnvironmentTest.kt` | 212 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/DeviceMemory.kt` | 47 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/DeviceMemoryTest.kt` | 70 | 1 | 配置参与构建并通过；工具链覆盖；风险模式命中 1 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/EnvironmentPinning.kt` | 322 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/EnvironmentPinningTest.kt` | 83 | 1 | 配置参与构建并通过；工具链覆盖；风险模式命中 1 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ExitInfoRecords.kt` | 212 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ExitInfoRecordsTest.kt` | 158 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/FrameStats.kt` | 142 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/FrameStatsTest.kt` | 51 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/FrameTimelineFrames.kt` | 224 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/FrameTimelineFramesTest.kt` | 153 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/HostLoad.kt` | 136 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/HostLoadTest.kt` | 117 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/InputFloodBenchmark.kt` | 89 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/InteractionAnimationBenchmark.kt` | 188 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/InteractionLatencyBenchmark.kt` | 105 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/MeasurementArtifacts.kt` | 256 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/MeasurementArtifactsTest.kt` | 160 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/MeasurementScenario.kt` | 169 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/MeasurementScenarioTest.kt` | 93 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/OutputLoadCompletion.kt` | 64 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/OutputScenarioFrames.kt` | 108 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/OutputScenarioFramesTest.kt` | 184 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/PresentedFrames.kt` | 90 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ProcessCpuTime.kt` | 81 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ProcessCpuTimeTest.kt` | 56 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ResourceStressBenchmark.kt` | 104 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ResourceStressBenchmarkTest.kt` | 27 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioBudget.kt` | 74 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioBudgetTest.kt` | 68 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioMatrix.kt` | 191 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioMatrixTest.kt` | 70 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioTarget.kt` | 31 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScenarioTargetTest.kt` | 35 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ScrollPerformanceBenchmark.kt` | 189 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SessionTitleProbe.kt` | 44 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/ShellReady.kt` | 66 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SteadyStateBenchmark.kt` | 96 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SurfaceLayerFrames.kt` | 285 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SurfaceLayerFramesTest.kt` | 67 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SustainedOutputLoad.kt` | 80 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/SustainedOutputLoadTest.kt` | 48 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/WideCharacterBenchmark.kt` | 118 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/WideCharacterLoad.kt` | 58 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Config | `android/benchmark/src/main/java/terminal/emulator/benchmark/WideCharacterLoadTest.kt` | 72 | 2 | 配置参与构建并通过；工具链覆盖；风险模式命中 2 处 |
| Gradle build | `android/build.gradle.kts` | 63 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Gradle build | `android/settings.gradle.kts` | 22 | 0 | Gradle 构建通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/.cargo/config.toml` | 11 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/Cargo.toml` | 96 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/benches/cell_builder.rs` | 240 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/benches/cjk_resolve.rs` | 45 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/benches/support.rs` | 88 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/benches/vt_typing.rs` | 50 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/build.rs` | 12 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi.rs` | 1042 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 4 处 |
| Rust/native | `native/src/android/ffi/appearance.rs` | 191 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 6 处 |
| Rust/native | `native/src/android/ffi/busy_marker_tests.rs` | 84 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/cell_size_packing_tests.rs` | 25 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/clipboard_answer_tests.rs` | 63 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/clipboard_read_flood_tests.rs` | 41 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/cursor_anchor_tests.rs` | 37 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/font.rs` | 507 | 14 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 14 处 |
| Rust/native | `native/src/android/ffi/font_info_packing_tests.rs` | 33 | 5 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 5 处 |
| Rust/native | `native/src/android/ffi/font_size_cap_tests.rs` | 118 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/idle_repaint_gate_tests.rs` | 43 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/input.rs` | 670 | 16 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 16 处 |
| Rust/native | `native/src/android/ffi/kitty_placement_keys_tests.rs` | 182 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/notification_poll_tests.rs` | 49 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/panic_payload_tests.rs` | 54 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/query.rs` | 482 | 17 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 17 处 |
| Rust/native | `native/src/android/ffi/render/clipboard.rs` | 283 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/render/control.rs` | 92 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 3 处 |
| Rust/native | `native/src/android/ffi/render/diagnostics.rs` | 167 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/render/frame.rs` | 956 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 3 处 |
| Rust/native | `native/src/android/ffi/render/mod.rs` | 26 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/render/overlay.rs` | 292 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 2 处 |
| Rust/native | `native/src/android/ffi/render/pump.rs` | 144 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/render/session_ops.rs` | 196 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 6 处 |
| Rust/native | `native/src/android/ffi/render/window.rs` | 296 | 14 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 14 处 |
| Rust/native | `native/src/android/ffi/render_sample_bits_tests.rs` | 93 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/render_wake_tests.rs` | 46 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/scroll_bookkeeping_tests.rs` | 82 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/android/ffi/scroll_frame_staleness_tests.rs` | 72 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/search_highlight_repaint_tests.rs` | 42 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/search_highlight_tests.rs` | 141 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/android/ffi/session.rs` | 299 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 4 处 |
| Rust/native | `native/src/android/ffi/session_entry_flag_tests.rs` | 69 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/session_registry_tests.rs` | 316 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 3 处 |
| Rust/native | `native/src/android/ffi/session_scoped_tests.rs` | 42 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/ffi/state.rs` | 366 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 8 处 |
| Rust/native | `native/src/android/ffi/wake.rs` | 164 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/android/keymap.rs` | 227 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/logging.rs` | 155 | 9 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 9 处 |
| Rust/native | `native/src/android/mod.rs` | 191 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/android/text_utils.rs` | 34 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/event.rs` | 489 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 6 处 |
| Rust/native | `native/src/lib.rs` | 147 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/cell_builder.rs` | 886 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/cell_builder/emit.rs` | 386 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/cell_builder/tests/appearance.rs` | 440 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/cell_builder/tests/cache.rs` | 798 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/render/cell_builder/tests/mod.rs` | 61 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/atlas.rs` | 201 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/cover.rs` | 281 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/gpu.rs` | 585 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 2 处 |
| Rust/native | `native/src/render/context/mod.rs` | 159 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/poll.rs` | 264 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 8 处 |
| Rust/native | `native/src/render/context/renderer.rs` | 849 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/context/surface.rs` | 236 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/context/tests.rs` | 253 | 5 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 5 处 |
| Rust/native | `native/src/render/font/atlas.rs` | 354 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/cjk.rs` | 269 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/font_db.rs` | 978 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/font/font_db/tests.rs` | 861 | 26 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 26 处 |
| Rust/native | `native/src/render/font/glyph_cache.rs` | 224 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 8 处 |
| Rust/native | `native/src/render/font/mod.rs` | 218 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/pipeline.rs` | 952 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/pipeline/families.rs` | 680 | 5 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 5 处 |
| Rust/native | `native/src/render/font/rasterization.rs` | 213 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/render/font/shaping.rs` | 374 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/font/tests/fallback.rs` | 744 | 39 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 39 处 |
| Rust/native | `native/src/render/font/tests/glyphs.rs` | 1073 | 28 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 28 处 |
| Rust/native | `native/src/render/font/tests/loading.rs` | 624 | 44 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 44 处 |
| Rust/native | `native/src/render/font/tests/mod.rs` | 139 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/render/kitty.rs` | 634 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 7 处 |
| Rust/native | `native/src/render/mod.rs` | 222 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/acquire.rs` | 480 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/render/pass/bands.rs` | 250 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/frame.rs` | 681 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/mod.rs` | 15 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/readback.rs` | 248 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/render/pass/tests.rs` | 791 | 10 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 10 处 |
| Rust/native | `native/src/render/pipeline.rs` | 346 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/render/tests/cell_pipeline.rs` | 992 | 10 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 10 处 |
| Rust/native | `native/src/render/tests/cover.rs` | 199 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 7 处 |
| Rust/native | `native/src/render/tests/frame_and_atlas.rs` | 830 | 15 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 15 处 |
| Rust/native | `native/src/render/tests/gpu_core.rs` | 948 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 7 处 |
| Rust/native | `native/src/render/tests/mod.rs` | 464 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 3 处 |
| Rust/native | `native/src/render/wgpu_backend.rs` | 223 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/slow_log.rs` | 406 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 3 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/commands.rs` | 374 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/frame_mailbox.rs` | 101 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/input_output_tests.rs` | 549 | 34 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 34 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/commands.rs` | 435 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/dispatch.rs` | 448 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/input_filter.rs` | 779 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/input_filter/tests.rs` | 788 | 29 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 29 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/kitty.rs` | 430 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/mod.rs` | 746 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/mouse_input.rs` | 71 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/rendering.rs` | 811 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；人工重点复核；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/run.rs` | 550 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/selection.rs` | 517 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/internal/tests.rs` | 326 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 6 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/key_encoding_tests.rs` | 454 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/mod.rs` | 160 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/notification_queue.rs` | 94 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api.rs` | 19 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/commands.rs` | 239 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/construction.rs` | 246 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/encoding.rs` | 211 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/events.rs` | 30 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/frames.rs` | 80 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/public_api/queries.rs` | 382 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/snapshot_cache_unit_tests.rs` | 18 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/batch_latency.rs` | 133 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/grid_text.rs` | 1029 | 41 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 41 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/kitty_graphics.rs` | 442 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/kitty_simulator.rs` | 320 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/lifecycle.rs` | 1235 | 66 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 66 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/mod.rs` | 254 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 8 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/mouse_osc.rs` | 875 | 25 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 25 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/osc_notification.rs` | 123 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/osc_state.rs` | 128 | 4 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 4 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/render_cache.rs` | 1143 | 65 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 65 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/tests/selection_search.rs` | 782 | 30 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 30 处 |
| Rust/native | `native/src/terminal/ghostty_terminal/types.rs` | 438 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/mock_pty.rs` | 302 | 23 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 23 处 |
| Rust/native | `native/src/terminal/mod.rs` | 56 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/output_processor.rs` | 634 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/terminal/pty.rs` | 890 | 26 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 26 处 |
| Rust/native | `native/src/terminal/pty/child.rs` | 179 | 9 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 9 处 |
| Rust/native | `native/src/terminal/pty/env.rs` | 103 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/pty/tests.rs` | 937 | 71 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 71 处 |
| Rust/native | `native/src/terminal/session/lifecycle.rs` | 227 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/session/mod.rs` | 392 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/session/output.rs` | 472 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/session/pty_master.rs` | 123 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/session/spawn.rs` | 305 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/terminal/session/tests/basics.rs` | 97 | 2 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 2 处 |
| Rust/native | `native/src/terminal/session/tests/clipboard.rs` | 298 | 9 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 9 处 |
| Rust/native | `native/src/terminal/session/tests/focus.rs` | 44 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/session/tests/mod.rs` | 132 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 7 处 |
| Rust/native | `native/src/terminal/session/tests/process.rs` | 230 | 6 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 6 处 |
| Rust/native | `native/src/terminal/session/tests/resize.rs` | 153 | 9 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 9 处 |
| Rust/native | `native/src/terminal/session/tests/scrollback.rs` | 69 | 1 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 1 处 |
| Rust/native | `native/src/terminal/session/tests/write_backpressure.rs` | 575 | 26 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 26 处 |
| Rust/native | `native/src/terminal/shell_env.rs` | 95 | 0 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 0 处 |
| Rust/native | `native/src/terminal/test_helpers.rs` | 784 | 23 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 23 处 |
| Rust/native | `native/src/terminal/theme.rs` | 478 | 3 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 3 处 |
| Rust/native | `native/src/terminal/theme/tests.rs` | 541 | 10 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 10 处 |
| Rust/native | `native/tests/alloc_budget.rs` | 167 | 8 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 8 处 |
| Rust/native | `native/tests/concurrency.rs` | 111 | 5 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 5 处 |
| Rust/native | `native/tests/instance_alloc_budget.rs` | 172 | 7 | rustfmt/clippy --all-targets -D warnings/cargo test --all-targets 通过；工具链覆盖；风险模式命中 7 处 |
| Config | `rust-toolchain.toml` | 4 | 0 | 配置参与构建并通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/build-android-libs.nu` | 47 | 1 | 构建脚本执行通过；工具链覆盖；风险模式命中 1 处 |
| Build scripts | `scripts/build-apk.nu` | 41 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/check.nu` | 19 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/download-aosp-testkey.nu` | 23 | 1 | 构建脚本执行通过；工具链覆盖；风险模式命中 1 处 |
| Build scripts | `scripts/download-rapidocr-models.nu` | 10 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/setup-emulator.nu` | 110 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |
| Build scripts | `scripts/test-emulator.nu` | 14 | 0 | 构建脚本执行通过；工具链覆盖；风险模式命中 0 处 |

## 汇总

- 清单文件数：624（Rust 157；Kotlin 含主代码/测试 277；Gradle 5；Android 资源/清单 22；R8 规则 1；配置 65；构建脚本 7）。
- `jscpd` 生产代码扫描范围：Rust 145 个文件、53,413 行；Kotlin 115 个文件、30,334 行。
- 重复度：`jscpd` 阈值 8 行/80 token，Rust 0 处克隆，Kotlin 1 处 11 行 import 重复。
- 全部命中项的工具链与测试证据见 `docs/REVIEW_LOG.md`。
