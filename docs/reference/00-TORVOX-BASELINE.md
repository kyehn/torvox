# torvox 功能基线（用于参考项目三向对比）

> 每个参考项目研究须与本表逐项对比：有 → 详细对比记录；无 → 记录差异。
> 引用格式：`[项目] 文件:行号 函数名`。本表由 2026-08-06 全面盘点生成。

## A. 终端引擎（Rust native/src/terminal/）
| 功能 | 位置 | 说明 |
|---|---|---|
| PTY | pty.rs | PtyPair：fork/execve（linker64+LD_PRELOAD 方案）、close_stray_fds（RLIMIT_NOFILE 上限）、winsize 像素字段 |
| Ghostty VT 引擎 | ghostty_terminal/ | libghostty-vt C FFI（patch 版），单锁，CellData 80B bytemuck 快路径 + GridSnapshot 查询路径 |
| OSC 处理 | osc_handler.rs | OSC 52 剪贴板/133 提示/9 通知/7 cwd，MAX_PAYLOAD_BYTES 上限 |
| 输出处理 | output_processor.rs | 输出批处理（flush 时机、BEL 检测、提示标记） |
| 会话 | session.rs | spawn/resize/write/send_signal/process_output/poll_clipboard/poll_notification/is_exited/exit_code_now/mark_exit_reported/focus_event；等待线程；exit 事件去重 |
| shell 环境 | shell_env.rs | ShellEnv 构造 |
| 键码编码 | cursor_cmds.rs + TerminalInputEncoder.kt | 键→转义序列（kitty 键盘协议） |
| VT 一致性测试 | vt_conformance.rs 等 | 大量一致性测试 |

## B. 渲染（Rust native/src/render/）
| 功能 | 位置 | 说明 |
|---|---|---|
| wgpu 后端 | wgpu_backend.rs | Android 强制 GL（模拟器 SwiftShader）；真机应 Vulkan |
| 上下文 | context.rs | Renderer：surface/atlas/bg_image/pipeline/bind group/uniform；attach_surface；upload_atlas（RGBA 扩展）；raster_scale 双侧同步 |
| 实例构建 | cell_builder.rs | build_instances_from_cell_data：reverse video fg↔bg swap、selection 反色、光标（Block/Bar/Underline）、grapheme_extra 堆叠、missing-glyph 空 quad |
| pass | pass.rs | render_frame/render_cell_data；acquire 超时 worker；debug group 配对 |
| 字体 | font/ | cosmic-text 排版 + swash 光栅化 + guillotiere 打包；FontPipeline 缓存；CJK 回退 |
| 背景图 | context.rs | Rgba8Unorm 纹理 + 半透明混合 |
| 选择渲染 | cell_builder.rs | GPU 侧 fg↔bg swap（经典反色） |
| 截图测试 | screenshot_tests.rs / tests.rs | CPU 端渲染回读断言 |

## C. Android JNI（native/src/android/ffi.rs，48 个导出）
initSession destroySession switchSession getSessionCount resize focusEvent feedPty writeKey pollEvent initLogger setLogFilePath attachWindow render detachWindow setMcpSocketPath setMcpEnabled clipboardResult dialogResult listSessions getTitle scrollbackLength scrollbackLine getTerminalText searchAllInScrollback isCellEmpty listFontFamilies getDefaultFontName getFontInfo clearSearchHighlights setSearchHighlights setSelection setTheme setBackgroundImage clearBackgroundImage setBackgroundParams setCursorBlink resetCursorBlink setRenderPaused setCursorStyle setFontFamily setFontSizeInPlace setRasterScale loadFontFile setSystemLocale setExtraFontPaths getCellWidth getCellHeight getGridRowsColsPacked setScrollOffset

## D. Kotlin UI（android/app/src/main/java/terminal/emulator/）
| 功能 | 位置 |
|---|---|
| 会话管理/渲染线程 | runtime/TerminalRuntime.kt（start/createSession/switchSession/closeSession/RenderSupervisor/事件轮询/退出处理） |
| 桥接 | bridge/Bridge.kt（PollResult/parseEvent）+ NativeBridge.kt + NativeQueryPort.kt + TerminalQueryPort.kt + SelectionExpander.kt |
| 选择 | ui/TerminalSurface.kt（长按→词/URL 扩展→GPU 反色→双手柄 PopupWindow→系统 ActionMode 顶部栏）+ TerminalViewModel.extractSelectedText |
| 输入 | ui/TerminalInputEncoder.kt + input/（KeyModifiers/ModifierState/KeyboardMode）+ InputBatchBuffer.kt + PasteChunker.kt |
| 键盘条 | ui/ModifierBar.kt（TAB/CTRL/ALT/ESC 等） |
| 搜索 | ui/TextSearchBar.kt + SearchResult.kt + UrlDetector.kt + 高亮 |
| 会话抽屉 | ui/SessionDrawer.kt |
| 设置 | ui/SettingsScreen.kt + SettingsComponents.kt + settings/（SettingsRepository 等）+ ToolbarPreferences.kt |
| 主题 | ui/theme/TerminalTheme.kt（Dracula Plus 等） |
| 监控 | monitor/（AnrWatchDog/RenderWatchDog/MemoryMonitor/ThermalMonitor/BootGuard） |
| 服务 | service/TerminalForegroundService.kt + ui/TerminalNotificationHelper.kt |
| 安装 | installer/（BootstrapDownloader/Installer/Orchestrator/SecondStageRunner——staging 原子安装 + linker64 执行链） |
| DocumentsProvider | TerminalDocumentsProvider.kt（SAF CRUD） |
| 剪贴板 | runtime/ClipboardAccess.kt + ClipboardPaster.kt（OSC 52） |
| 日志 | runtime/LogUtil.kt + LogcatFileWriter.kt + LogcatDumpWriter.kt |
| 测试 | androidTest/ + src/test/（JVM 单元 52+） |

## E. MCP（native/src/mcp.rs，tower-mcp）
9 工具：terminal_info / clipboard_get / clipboard_set / notify / toast / open_url / pick_file / dialog / send_signal；Unix socket + Stdio 双传输；mcp feature gate（默认编译，运行时开关）。

## F. 已确认无的功能（参考项目常有而 torvox 无）
- SSH 客户端/服务器
- PRoot/chroot 发行版
- 内置包管理（apt/pkg）
- X11/Wayland 服务器、VNC/RDP/SPICE
- 语音/协作/CRDT
- 插件系统/Intent API
- 会话持久化/恢复（已删除）
- USB 串口/键盘模式选择（已删除）
