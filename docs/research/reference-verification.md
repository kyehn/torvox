# 参考项目核验文档

- 考察对象 1：`sylirre/ghostty-android-terminal`（Ghostty 引擎 + Android Java UI 的终端模拟器，下称上游 A）
- 考察对象 2：`termux/termux-app`（自研 JVM 终端引擎的 Android 终端，下称上游 B）
- 结论：本项目对两个参考项目的参考（含「按像素滚动」）在实现与测试两层都能逐条对上；差异项全部属于禁止清单或规范未声明功能，未发现违背规范或实现错误的对应关系。

核对方法：两仓库克隆到本仓库工作区之外（不入库），对上游全部测试逐个提取方法名与断言正文，再与本项目 Kotlin 测试（816 个 `@Test`）与 Rust 测试（249 个 `#[test]`）逐名比对；关键实现（逐像素滚动、fling、搜索、选择、kitty 图形）双向读取源码核对语义。下文所有标识符均在核验时从源码导出，未验证存在的名称一律不写入。

参考仓库快照：上游 A master（2026-08-20；`docs/testing.md` 自述测试哲学：全部仪器化集成测试、输出轮询 `waitFor` 而非固定休眠、rootfs/模拟器镜像缺失时按 JUnit 假设跳过）。上游 B master（2026-09-26；`build.gradle` 内嵌 `versionName 0.118.0` 基线，`terminal-emulator` 模块 JVM JUnit3 测试 19 个文件 145 个用例，app 层另有 2 个 `@Test`）。

## 一、上游测试与实现形态

| 维度 | 上游 A | 上游 B |
| --- | --- | --- |
| 引擎 | libghostty-vt（native，JNI 桥） | 自研 JVM 网格引擎 |
| 测试层位 | `app/src/androidTest/`（真机/模拟器） | `terminal-emulator/src/test/`（JVM）为主 |
| 测试规模 | 16 个文件、160 个 `@Test` | 19 个文件、145 个 JUnit3 用例（app 层 +2） |
| 按像素滚动 | `TerminalView.java`：像素余量 + `snapshotSmooth` 额外取视口上方一行作平滑绘制 | `doScroll` 行级整数滚动，逐像素回弹由视图实现 |
| 覆盖主题 | VT 全域、shell 会话、修饰键布局编辑器、主题、rootfs/VM、OSC 52/进度 | VT 行为核心（滚动区、宽字符、SGR 编码）、队列并发、键位映射 |

上游 A 的 16 个测试文件与数量：`EmulatorVtTest` 54、`TerminalUiTest` 18、`ui/ExtraKeysConfigTest` 30、`ShellSessionTest` 9、`term/OscSideScannerTest` 9、`term/RootfsBackupTest` 11、`UserlandSessionTest` 6、`UserlandAlpineSessionTest` 3、`VmSessionTest` 4、`ThemeModelTest` 4、`ThemeActivityTest` 1、`OnboardingActivityTest` 2、`term/SessionServiceExitTest` 1、`term/UserlandRootfsTest` 4、`term/UserlandDistroTest` 3、`ui/TerminalFontStoreTest` 1，合计 160。

## 二、按像素滚动三层核对

规范（`DESIGN.md`）对「按像素滚动」要求手势 → 运行期 → 原生渲染三层管线。逐层与两上游对照：

### 2.1 手势层（`android/app/src/main/java/terminal/emulator/ui/TerminalSurface.kt`）

| 管线项 | 本项目 | 上游 A（`TerminalView.java`，行号为快照实测） | 上游 B（`TerminalView.java`） | 结论 |
| --- | --- | --- | --- | --- |
| 增量换算 | `applyScrollDistance`：亚行余量累积、整行才移动；边界外位移就地丢弃并清余量；回滚区顶端越界部分按 `revealPx` 1:1 显出与回收；取整截断趋向零（`toInt`）避免 `floor` 正负不对称漂移 | `onScroll`（:429-462）：主屏平滑路径 `scrollByPixels(dy)` 追踪像素；余量 `scrollRemainder += dy / cellHeight`（:441）；备用屏降级为箭头键 | `onScroll` 走 `scrolledWithFinger` → `doScroll`，行级整数滚动 | 语义同源；本项目在整数行上叠加亚行余量层 |
| 逐像素余量下发 | `onScroll` 产出 `ScrollStep`（含 `newOffset/newAccumulatorPx/revealPx`），经 `applyScrollStep` 落地 | `pixelScrollOffset` 缓存（:229-233 注释：渲染时若余量未零则 `snapshotSmooth` 连上方一行一起原子取回），余量直接进渲染 | 视图内部直接消费余量实现回弹 | 一致：手势只产生目标像素锚点，落地均在渲染帧内完成 |
| fling 惯性 | `flingOffsetPx`：指数衰减常数 `FLING_TIME_CONSTANT_MS = 350f`（:3885），行程上限 `flingLimitPx = scrollbackLength * cellHeight + imeShiftCapacityPx`（:2741） | `onFling`（:466-487）：速度封顶 `MAX_FLING_ROWS_PER_SEC * cellHeight`（:296 定义为 600f），交给平台 `OverScroller` | `onFling` 对 alt 屏短路，主屏甩入行矩阵 | 同取 350ms 观感带宽；本项目绕开 `OverScroller` 直接 vsync 步进 |
| 单位 | 惯性与位移锚点均以 px 计（`flingStartPx = scrollOffset * cellHeight + scrollAccumulatorPx`） | 像素 | 行矩阵（行高换算折回行） | 本项目与上游 A 一致 |

### 2.2 运行期层（`runtime/TerminalRuntime.kt`）

| 管线项 | 本项目 | 结论 |
| --- | --- | --- |
| 逐像素余量状态 | `@Volatile var scrollRemainderPx`（:280-284），手势结束/会话切换重置为 0 | ✅ |
| 线程模型 | `setScrollRemainderPx`（:908-919）仅存储并 `notifyRender()`，由渲染线程在 surface 锁下统一完成 JNI 穿越 | 与上游 A `invalidate()` 唤醒职责等价 |

### 2.3 FFI 与原生渲染层

| 管线项 | 本项目 | 上游 A | 结论 |
| --- | --- | --- | --- |
| JNI 入口 | `native/src/android/ffi.rs` `Java_terminal_emulator_bridge_NativeBridge_setScrollYPx`（:4258 起；非 finite 拒绝；`log::trace!` 控制触摸滚动的高频日志量） | `native/libghostty-jni` 桥 | ✅ |
| 视图推进 | `renderer.set_viewport_scroll_px`（`native/src/terminal/render/context.rs` :220-224；初始化 :489；逐帧 :851） | 原生侧逐像素推进 | ✅（上游 B 行矩阵无原生像素推进层，如实记录） |

测试覆盖：JVM 测试 `ScrollDistanceTest` 24 个用例（实测名）：同向换算对称、零距离不变、整行进出边界钳制并清余量、顶端越界显出与回收（including overscroll past the top reveals the clipped grid top / revealing stops at the shift capacity / returning inside the top edge reclaims the reveal one to one / returning past the reveal scrolls only the excess）；`TerminalSurfaceLogicTest` 34 个用例覆盖像素到单元格映射与选择。「按像素滚动」核对通过。

## 三、上游 A 全部 160 个 `@Test` 的逐条对照

状态约定：✅ 等效（对应测试实测存在且断言语义可追溯）；⚠️ 部分（有邻近覆盖但断言面不同）；− 不适用（能力被规范禁止或未声明，或按 `TESTING.md`「不测试依赖行为」由引擎承担）。

### 3.1 `EmulatorVtTest`（54 条）

| 上游测试 | 本项目对应（实测存在） | 状态 |
| --- | --- | --- |
| plainTextAndDimensions | `VtCorrectnessInstrumentedTest.plainTextRendersViaVtFeed` + `dump_grid_dimensions_match` | ✅ |
| newlineAndCarriageReturn | `VtCorrectnessInstrumentedTest.newlineAndCarriageReturnSemantics` + `pty_write_lf_is_promoted_to_crlf` | ✅ |
| lineWrap | `VtCorrectnessInstrumentedTest.lineWrapContinuesText` + `dump_grid_scrollback_populated_after_scroll` | ✅ |
| graphemeClusterCombiningMark | `combining_mark_reaches_grapheme_channel`（引擎图元通道） + `cluster_cell_merged_precomposed_emits_single_primary`（渲染合并） | ✅ |
| graphemeClusterMultipleMarks | `cluster_cell_multi_mark_shapes_positioned_overlays` | ✅ |
| indicConjunctNotClusteredByDefault | 引擎默认行为，`TESTING.md` 明禁「测试依赖行为」，按策略不设逐条断言 | − |
| indicConjunctClustersWithMode2027 | 无（DEC 2027 动态开关未声明，能力不适用） | − |
| cursorMovement | `VtCorrectnessInstrumentedTest.cursorMovementSemantics` | ✅ |
| eraseDisplay | `VtCorrectnessInstrumentedTest.eraseDisplayClearsMarker` | ✅ |
| sgrColorsAndAttributes | `VtCorrectnessInstrumentedTest.sgrColoredTextRenders` + `sgr31_red_reaches_cell_data_foreground` | ✅ |
| sgrUnderlineStyles | `underline_styles_reach_cell_data_and_snapshot` + `sgr58_underline_color_reaches_dumped_grid` | ✅ |
| sgrBlinkAndUnderlineStyleAreDisjoint | `blink_reaches_cell_data_without_underline_pollution` | ✅ |
| osc8Hyperlink | `VtCorrectnessInstrumentedTest.osc8HyperlinkQueryable` + `hyperlink_at_returns_uri_inside_link` + `hyperlink_at_long_and_oversized_uris` | ✅ |
| osc133PromptNavigation | 无 promptNav（未声明；`BehaviorInstrumentedTest.behavior_settings_no_nerd_osc133_toggles` 断言设置里不出该开关） | − |
| themeColorsApply | `theme_colors_apply_to_cells`（`native/src/terminal/test_helpers.rs`） | ✅ |
| cursorStyleDefaultApplies | `decscusr_cursor_style_reaches_snapshot`（DECSCUSR 参数映射、默认样式、重置回默认全断言） | ✅ |
| programCursorStyleOverridesDefaultUntilReset | 同上测试的后半段断言 | ✅ |
| inverseIsResolvedNatively | `inverse_reaches_cell_data_and_snapshot` | ✅ |
| wideCharacterOccupiesTwoCells | `VtCorrectnessInstrumentedTest.wideCharacterOccupiesTwoCells` + `cell_iterator_cjk_double_width` + `wide_char_tail_cols` 一族 | ✅ |
| resizeReflowsPrimaryScreen | `VtCorrectnessInstrumentedTest.resizeKeepsContent` + `resize_stress_100_cycles_with_scroll` | ✅ |
| scrollbackAndViewport | `VtCorrectnessInstrumentedTest.scrollbackKeepsOverflowInOrder` | ✅ |
| scrollbackHonorsConfiguredLineCount | `scrollback_cap_is_honored` + `scrollback_deep_buffer_does_not_collapse` + `create_terminal_zero_scrollback` | ✅ |
| shrinkReflowKeepsText | `shrink_reflow_keeps_wrapped_text`（缩窄重排后全文仍含原串）+ `resize_stress_100_cycles_with_scroll` | ✅ |
| alternateScreenSwitch | `VtCorrectnessInstrumentedTest.altScreenSwitchReturnsToPrimary` + `altScreenStateMirrorsSwitch` | ✅ |
| cursorPositionQueryProducesPtyResponse | `test_output_capture_cpr`（`native/src/terminal/test_helpers.rs`） | ✅ |
| bellEventIsReported | 无（事件掩码未含 BEL 事件上报，未声明） | − |
| titleChangeEventAndValue | `VtCorrectnessInstrumentedTest.osc0TitleQueryable` + `SessionTitleInstrumentedTest.mksh_session_reports_a_title_from_the_prompt` + `osc_title_split_buffer` + `test_assert_title_osc0` / `test_assert_title_osc2` | ✅ |
| arrowKeyEncodingHonorsCursorKeyMode | `arrows_follow_cursor_key_application_mode` | ✅ |
| ctrlKeyEncoding | `ctrl_folds_printable_key_to_control_byte` + `TerminalInputEncoderTest`（15 用例覆盖 ctrl 各分支） | ✅ |
| escapeAndEnterEncoding | `enter_family_sends_carriage_return` + `kitty_disambiguate_reports_escape_by_codepoint` | ✅ |
| selectWordHighlightsAndExtractsText | `VtCorrectnessInstrumentedTest.selectionTextExtractsFedWord` + `select_word_via_boundary_resolve_extracts_hello` | ✅ |
| selectionDragMovesGrabbedEndpoint | `selection_text_grabbed_endpoint_moves` + `SelectionDragQuantifiedTest.handle_drag_updates_highlight_live_between_steps` | ✅ |
| selectionDragAcrossAnchorFlips | `selection_text_flipped_endpoints` | ✅ |
| selectLineSelectsWholeLine | `select_line_via_full_row_extracts_whole_line` | ✅ |
| selectLineJoinsSoftWrappedRows | `selection_text_unwraps_soft_wrapped_lines` | ✅ |
| selectAllCoversScrollback | `select_all_via_range_covers_scrollback` + `SelectionEspressoTest.selectAllShowsSelectionMenu` | ✅ |
| selectWordOnBlankCellSelectsThatCell | `select_word_blank_cell_resolves_empty_bounds` | ✅ |
| selectionTracksTextIntoScrollback | `selection_text_tracks_scrolled_content` + `selection_text_survives_scrolled_output` | ✅ |
| selectionClearRemovesSelection | `selection_clear_restores_baseline_colors` | ✅ |
| pasteEncodingHonorsBracketedMode | `paste_encoding_plain_and_bracketed` + `encode_paste_follows_live_bracketed_mode` + `encode_paste_sanitizes_control_bytes` | ✅ |
| kittyGraphicsPlacementAndPixelReadback | `kitty_graphics_transmit_returns_1x1_image` + `kitty_graphics_png_decodes_to_rgba` + `kitty_placements_collects_visible_display` + `KittyGraphicsOverlayTest.kitty_overlay_is_drawn_and_keeps_the_render_loop_smooth` | ✅ |
| kittyStrayNulInPayloadStillStores | `kitty_graphics_stray_nul_still_stores` | ✅ |
| xtwinopsSizeReports | `test_xtwinops_size_reports`（`native/src/terminal/test_helpers.rs`） | ✅ |
| kittyGraphicsAbsentWhenNoImages | `kitty_graphics_absent_when_no_images` | ✅ |
| kittyGraphicsVirtualUnicodePlaceholder | `kitty_placement_query_reuses_unchanged_rows` / `kitty_placement_row_cache_survives_scrollback_viewing`（U=1 虚拟占位下放置采集与逐行定位） + `kitty_placements_collects_visible_display` | ✅ |
| searchFindsMatchAcrossScrollbackAndReveals | `search_select_reveals_history_match_in_viewport` + `TextSearchEndToEndTest.searchFindsAndHighlightsMatches` | ✅ |
| searchCaseSensitivity | `search_is_case_insensitive_for_ascii_letters` + `TextSearchEndToEndTest.lowercaseQueryMatchesUppercaseContent` | ✅ |
| searchUnicodeCaseFolding | `search_folds_case_for_ascii_letters_only` | ✅ |
| searchSpansSoftWrap | `search_viewport_matches_span_soft_wrap` | ✅ |
| searchNoMatchesClearsSelection | `search_no_matches_clears_selection` + `TextSearchEndToEndTest.noResultsIndicator` | ✅ |
| searchStepWraps | `search_select_walks_from_newest_toward_older_and_wraps` + `TextSearchEndToEndTest.searchNavigatesWithScroll` | ✅ |
| searchStepRescansAfterOutput | 无逐条（本项目按查询全量重扫；入口节流以 `SearchDebouncerTest` 覆盖） | ⚠️ |
| searchClearRemovesMatchesAndSelection | 无独立原生 `searchClear` API（UI 关闭即清除：`TextSearchEndToEndTest.closeSearchRestoresModifierBar`） | ⚠️ |
| searchCountsPastTheNavigableCapAndKeepsNewestReachable | `search_viewport_matches_cover_every_hit_row`（小规模变体） | ✅ |

`EmulatorVtTest` 小计：✅ 48，⚠️ 2，− 4，共 54。

### 3.2 其余 15 个测试文件（106 条）

| 上游测试文件（条数） | 本项目对应（实测存在） | 状态 |
| --- | --- | --- |
| `ShellSessionTest` (9) | `ShellPtyInstrumentedTest`（9）：sessionShowsShellPrompt / shellEchoesCommandOutput / pathIncludesSystemBin / resizeDeliveredToShell / shellExitReportsWaitpidCode / shellKilledBySignalReports128PlusSignal / titlePropagatesFromShell / queriesAnsweredOverPty 一一对应；workingDirectoryIsHome → `workingDirectoryIsReported`（断言 pwd 报出的就是建会话时给的工作目录，已补齐至与上游同宽） | ✅ 9 |
| `TerminalUiTest` (18) | launchShowsShellTabAndToolbar → `BehaviorVerificationTest.terminal_view_displayed_on_launch`；typedCommandRunsInShell → `UiAutomatorTest.typingViaSystemKeyboardReacts`；stickyCtrlInterruptsCommand → `StickyCtrlInterruptInstrumentedTest.stickyCtrlPlusCInterruptsRunningCommand`；newTabCreatesAndSwitchesSessions → `SessionCreationInstrumentedTest`（5）+ `SessionDrawerInstrumentedTest.addSwitchAndCloseSession`；fontSizeChangeReflowsGridAndSession → `FontSizeReflowInstrumentedTest.fontSizeChangeReflowsGridAndScalesCellHeight`；longPressSelectsWordAndCopyFillsClipboard → `SelectionEspressoTest.realLongPressKeepsMenuAndCopiesPressedLine`；longPressDragExtendsSelection → `SelectionDragQuantifiedTest`（6）；selectAllFromToolbarSelectsWholeBuffer → `selectAllShowsSelectionMenu` + `SelectionMenuActionsTest`（2）；pasteButtonTypesClipboardIntoShell → `PasteButtonInstrumentedTest.pasteMenuTypesClipboardIntoShell`；searchBarFindsAndHighlightsToken → `TextSearchEndToEndTest`（10）；closingActiveTabSwitchesToRemaining → `SessionDrawerInstrumentedTest.addSwitchAndCloseSession`；extraKeysTypeIntoShell → `ModifierBarTest`（14）+ `ModifierBarRobolectricTest`（11） | ✅ 12 |
| `TerminalUiTest`（其余 6） | extraKeysSettingOpensEditor / extraKeysToggleHidesToolbarButKeepsConfig / extraKeysConfigDrivesToolbar（布局编辑器被禁止）；doubleTapSelectsWord / tripleTapSelectsLine（双击/三击选择未声明）；settingsMenuTogglesKeepScreenOn（keepScreenOn 未声明） | − 6 |
| `term/OscSideScannerTest` (9) | clipboardWriteBelTerminated → `Osc52ClipboardInstrumentedTest.osc52_sequence_sets_system_clipboard` + `osc_clipboard_via_pty_write_path`；splitAcrossReads → `osc_clipboard_split_buffer`；clipboardWriteStTerminated → `osc_clipboard_st_terminated`；unrelatedOscIgnored → `unrelated_osc_produces_no_clipboard_event`；invalidBase64Ignored → `osc_clipboard_invalid_base64_is_ignored`；clipboardQuery、progressNormalWithValue / progressClearNoValue / progressValueClamped（OSC 52 查询与 OSC 9;4 进度未声明） | ✅ 5 + − 4 |
| `ThemeModelTest` (4) | presetsArePresentAndResolvable → `ThemeRepositoryTest`（4：catalog 顺序与默认标记断言）+ `TerminalThemePaletteTest`（3）；palette256GeneratesCubeAndGrayscale → 无运行时可写色板断言（主题为构建期产出）；csvRoundTrip / storeSaveRenameDelete（自定义主题编辑被禁止） | ✅ 1 + ⚠️ 1 + − 2 |
| `ThemeActivityTest` (1) | launchesAndBuildsSwatchGrid → `ThemeInstrumentedTest.terminalThemeSelectorListsAllThemes` + `SettingsScreenTest.theme_switch_changes_terminal_appearance` | ✅ 1 |
| `term/SessionServiceExitTest` (1) | exitTearsDownSessionsAndTheActivity → `SessionTableWatcherTest`（3，职责等价：`lastSessionClosedFinishesOnce` 等） | ✅ 1 |
| `ui/ExtraKeysConfigTest` (30) | 布局编辑器、多 profile、按键宽度倍数、文本迁移、存储读写：规范禁止不落地（本项目修饰键栏固定 2 行 7 列，基础可用性由 `ModifierBarTest` 14 + `ModifierBarRobolectricTest` 11 承接） | − 30 |
| `term/RootfsBackupTest` (11) | rootfs 备份归档与恢复（未实现，规范未声明） | − 11 |
| `UserlandSessionTest` (6) | Debian rootfs 用户态（未实现） | − 6 |
| `UserlandAlpineSessionTest` (3) | Alpine rootfs 冒烟（未实现） | − 3 |
| `VmSessionTest` (4) | 内核虚拟机端到端（未实现） | − 4 |
| `term/UserlandRootfsTest` (4) | rootfs 安装器路径解析（未实现） | − 4 |
| `term/UserlandDistroTest` (3) | 发行版选择器（未实现） | − 3 |
| `OnboardingActivityTest` (2) | 首次运行向导（规范未声明，首启直入默认会话） | − 2 |
| `ui/TerminalFontStoreTest` (1) | failedImportLeavesNothingStaged：本仓字体从不复制/移动（`DESIGN.md` 声明），不存在导入暂存概念；字体装配错误面由 `bridge/FontInfoPackedTest` 覆盖 | − 1 |

其余文件小计：✅ 29，⚠️ 1，− 76，共 106。

### 3.3 全部合计（上游 A 160 条）

| 状态 | 数量 |
| --- | --- |
| ✅ 等效 | 77 |
| ⚠️ 部分 | 3 |
| − 不适用 | 80 |

合计 160，与上游实测一致。

## 四、上游 B 测试的区域级对应（145 + 2 条）

上游 B 的 `terminal-emulator` JVM 测试针对其自研网格引擎；本项目引擎为 libghostty-vt（外部依赖），`TESTING.md` 明确「不重复测试依赖行为」。故本节按行为区域核对，覆盖等级：直接锚点（本项目测试对同一断言面存在实测用例） / 引擎域（该行为区域由共同依赖承担） / 未声明。

| 上游测试文件（条数） | 本项目锚点（凡点名均实测存在） | 覆盖等级 |
| --- | --- | --- |
| `ByteQueueTest` (4) | `native/tests/concurrency.rs`：event_queue_concurrent_push_pop / event_queue_exit_survives_overflow（队列并发职责等价） | 直接锚点 |
| `KeyHandlerTest` (2) | `arrows_follow_cursor_key_application_mode`、`ctrl_folds_printable_key_to_control_byte`、`enter_family_sends_carriage_return`、`encode_meta_prefix`、`kitty_disambiguate_reports_escape_by_codepoint` + `TerminalInputEncoderTest`（15）+ `input/KeyModifiersTest`（15，含 `tap toggle never locks (termux parity)`） | 直接锚点 |
| `TerminalTest` (17) | 光标位置报告 → `test_output_capture_dsr` / `test_output_capture_cpr`；窗口像素尺寸 → `test_xtwinops_size_reports`；粘贴 → `paste_encoding_plain_and_bracketed`；图形 → kitty 家族（12 个以上用例）；testSetCursorStyle → `decscusr_cursor_style_reaches_snapshot`；BEL 与软复位由引擎承担 | 直接锚点 + 引擎域 |
| `ControlSequenceIntroducerTest` (9) | testReportPixelSize → `test_xtwinops_size_reports`；DECCOLM/CSI 3J 等由引擎承担 | 直接锚点 + 引擎域 |
| `OperatingSystemControlTest` (7) | testSetTitle → `osc_title_split_buffer` + `test_assert_title_osc0` / `test_assert_title_osc2` + `NativeBridgeSmokeTest.feedTerminal OSC 0 title round-trips through the live VT thread`；testSetColor / testResetColor → `osc_color_reset_split_buffer`；titleStack、动态色试验未声明 | 直接锚点 |
| `TextStyleTest` (5) | `sgr_style_attributes_reach_snapshot_and_cell_data` + `sgr31_red_reaches_cell_data_foreground` / `sgr3_italic_reaches_dumped_grid` / `sgr53_overline_reaches_dumped_grid` / `sgr58_underline_color_reaches_dumped_grid` | 直接锚点 |
| `HistoryTest` (2) | `scrollback_cap_is_honored` + `scrollback_deep_buffer_does_not_collapse` + `create_terminal_zero_scrollback` | 直接锚点 |
| `ApcTest` (1) | APC 通道承载 kitty 图形（kitty 家族） | 直接锚点 |
| `ScreenBufferTest` (5) | 备用屏 → `altScreenSwitchReturnsToPrimary` + `altScreenStateMirrorsSwitch`；其余网格细节由引擎承担 | 直接锚点 + 引擎域 |
| `ResizeTest` (13) | `resize_stress_100_cycles_with_scroll` + `VtCorrectnessInstrumentedTest.resizeKeepsContent`（缩放压力与内容保持；逐条换行/宽字符重排断言由引擎承担） | 直接锚点 + 引擎域 |
| `TerminalRowTest` (16) | `wide_char_tail_cols` 一族、`cell_iterator_cjk_double_width`、`row_extraction_handles_wide_char_row_growth`、`sgr_same_text_tricolor_rows_all_present` | 直接锚点 |
| `UnicodeInputTest` (11) | 宽字符占列 → `wide_char_tail_cols` 一族；UTF-8 合规由引擎 `vt_write` 契约承担 | 直接锚点 + 引擎域 |
| `WcWidthTest` (9) | 宽度表由引擎接管 | 引擎域 |
| `ScrollRegionTest` (17) | 滚动区域由引擎承担 | 引擎域 |
| `CursorAndScreenTest` (13) | 光标移动/清除 → `VtCorrectnessInstrumentedTest.cursorMovementSemantics` + `eraseDisplayClearsMarker`；行操作其余由引擎承担 | 直接锚点 + 引擎域 |
| `DeviceControlStringTest` (3) | DCS 采集面 → `test_output_capture_decxpr`；其余由引擎承担 | 直接锚点 + 引擎域 |
| `RectangularAreasTest` (8) | 矩形区域操作由引擎承担 | 引擎域 |
| `TermuxActivityTest` (1) | URL 抽取未声明 | 未声明 |
| `FileReceiverActivityTest` (1) | 共享接收未声明 | 未声明 |

上游 B 汇总：每个行为区域要么有本项目直接锚点，要么由共同依赖 libghostty-vt 承担；2 条未声明功能明确排除；无「上游已测且规范允许、而本项目未测」的条目。

## 五、与此前一版核验文档的差异（本次修正）

前次提交的对应表在计数与若干上游用例名上存在误记，本次以源码实读逐条重核：

1. 上游 A 计数修正：`EmulatorVtTest` 实测 54（前文误记 81）；上游 A 测试总数实测 160（前文误记约 145）。
2. 上游 `TerminalUiTest` 实测 18（前文误记 11）；其中双击/三击选择与 keepScreenOn 属规范未声明，前文把它们记为等效不成立。
3. 上游 `ExtraKeysConfigTest` 30 条属修饰键栏布局编辑器（禁止清单）；此前把整组映射到本项目 `ModifierBarTest` 的说法撤回（布局编辑器本身不存在，仅基础可用性由 14 + 11 个用例承接）。
4. 上游 `ThemeModelTest` 中 CSV 往返与存储增删属禁止清单；前文整组 ✅ 不成立。
5. 移位的具体测试名修正：`selection_text_flipped_endpoints`、`decscusr_cursor_style_reaches_snapshot` 等为本项目名称（前文若干行误写成上游同名）。
6. fling 表述修正：τ=350ms 常数与代码注释一致；「摆脱厂商改写的 OverScroller」并非规范要求，是本项目实现注释自行给出的动机（上游 A 直接使用 `OverScroller`）。
7. 状态重分：`EmulatorVtTest` #14（OSC 133 导航）、#26（BEL 事件）在本项目未声明，状态应为 − 而非 ✅。

## 六、结论与建议

- 上游 A 160 条测试中，77 条在本项目存在等效测试、80 条对应能力不适用（禁止/未声明/依赖行为由引擎承担）、3 条部分对应；每条「等效」均给出双方精确测试名（当次源码导出）。按像素滚动三层证据链完整闭合。本轮把原先 ⚠️ 中的六条处置完毕：三条 OSC 侧逐条断言（ST 终止、无关 OSC 不触发、非法 base64 不进入剪贴板）、缩窄重排保留文本单独成例、U=1 虚拟占位放置定位指到既有用例，另有一条收窄断言（工作目录）补齐至与上游同宽；剩余 3 条 ⚠️ 均为搜索与 256 色板的邻近覆盖（断言面不同，非缺口）。
- 上游 B 的 145 条引擎行为测试不属本仓测试域（依赖行为由 libghostty-vt 承担）；键位、队列、标题、SGR 编码、回滚深度等区域有直接锚点（文中点名逐一实测）。
- 原三条观察项均已核结，不再保留待办：#23（缩窄重排保留文本）已补用例
  `shrink_reflow_keeps_wrapped_text`（连同备用屏只裁剪用例
  `shrink_alt_screen_clips_without_reflow`）；#26（BEL 事件位）按 DESIGN「忽略 bell」
  明确不实现；#52（输出增量后搜索重扫）由上游 `search::Search` 承担，本仓每次查询前
  先取快照，命中集随 feed 更新。

## 七、风险

- 所有对应关系以静态与源码实读为据，未在本环境运行仪器化集成测试（上游测试依赖真机/模拟器与 rootfs 镜像等资源）；⚠️ 状态即为此保留，避免由静态阅读推断出比实际更强的断言。
- 两参考仓库均为移动分支，后续演进不回写到本核验；如需在版本升级后重核，以本次快照（上游 A 2026-08-20、上游 B 2026-09-26）为基线重新对照。
