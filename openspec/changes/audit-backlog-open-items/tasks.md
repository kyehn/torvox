# 任务：审计遗留项台账

编号沿用审计文档原编号，便于对照历史记录。状态口径见 `design.md` 第 2 节。

## 1. 高危：用户可观察的错误状态

- [x] N3「全选→复制」经 VT 线程同步做约 110 万次 ghostty FFI，
      500ms 超时后回落空网格 —— **本轮否证**：当前路径是「上游 `Formatter` 单次
      `format_alloc`」（`selection_text_impl`）＋`Terminal.select_all` 单次调用，
      逐行 `scrollbackLine` 拼接早已删除，无逐单元 FFI；超时回退也不再静默
      （N1-10 已让失败出声）
- [x] N1-7渲染暂停（打开设置页／输入法弹出）时 OSC 52 剪贴板写入
      被后一次覆盖 —— 本轮已修：单槽锁存改 FIFO（`Session::clipboard_text`），
      上游回调通道本身有界且满时记日志，队列深度因此受其约束
- [x] N1-9OSC 52 读取失败时回「成功但为空」—— 本轮已修：
      读取失败经 `clipboardResult(..., null)` 上报，原生不写回 OSC 52 应答
      （空串等于告诉远端「用户清空了剪贴板」）
- [x] N1-10选区查询失败被合并为「无选区」—— 本轮已修：
      空选区 `""` 与查询失败 `null` 分开，失败记 error 不再静默
- [x] N2-50`writeToPty` 在**执行时**才解析 `sessions[activeSessionId]`，
      跨会话切换时粘贴尾部写进新会话 —— 本轮已修：入队时捕获会话 id 并随字节下沉
      （`InputBatchBuffer` 驻留 id，目标会话变化时先按旧会话排空）
- [x] N2-45`Query::EncodeMouseEvent` 无 modifier 字段，
      Shift/Ctrl 点击到达 vim/tmux/htop 与普通左键不可区分（违反 DESIGN:182）——
      本轮已修：JNI 增 `modifiers`（上游 `key.Mods` 原始位），编码器 `set_mods`，
      Kotlin 侧由 `KeyModifiers.ghosttyMods(metaState)` 换算，并有编码差异回归测试
- [ ] N2-44DESIGN:153 要求的修饰键栏左右移动可见区域未实现 —— 触发
      条件在本仓不存在：网格列数是 `floor(surfaceWidth / cellWidth)`，内容恒不
      横向溢出（`computeGridDimensions`），左右键因而必须继续把箭头序列发给远端；
      是否为规范补一句「内容永不溢出故无需平移」由用户裁决（D13）

## 2. 中危：资源与契约

- [ ] N9 / N2-23`themes.xml` 硬编码 `#1E1E2E` 且无 `values-night/`，
      日间主题下系统窗口恒为夜间配色（违反 DESIGN:106/108/200）
- [x] N21`ModifierBar` 的 DRAWER 长按落到 `else -> null`，长按粘贴从未接线 ——
      本轮已修：`secondaryLongPressAction` 增 `DRAWER -> onPasteClick`（termux
      `popup: 'PASTE'`），并有 Robolectric 用例覆盖「长按粘贴 / 轻点开抽屉」
- [x] N2-1`FontUtils` 把 `mono/monospaced/sans` 硬编码改写，
      真名为 "Sans" 的字族被静默换成另一个（违反 DESIGN:101/102）—— 本轮已修：
      `resolveEffectiveFontFamily` 只去空白，别名归并删除（字族名以外部库为准）
- [x] N2-2`~/.termux/fonts` 只在会话创建时扫一次 —— **本轮否证**：
      字体列表在每次会话就绪时先 `setExtraFontPaths(~/.termux/fonts)` 再列举
      （`TerminalViewModel:737`），新建会话即重新扫描；DESIGN 字体节要求的是
      「列表缓存直到应用关闭」，会话级刷新比之更及时，不构成缺陷
- [x] N2-2459 个 `external fun` 只对 43 个加 `@JvmStatic`，
      CheckJNI 报错、`RegisterNatives` 路径断裂 —— 本轮已修：补齐余下 6 处
      （`setTheme`/`setFontFamily`/`loadFontFile`/`setExtraFontPaths`/
      `getCellHeight`/`setScrollOffset`），并加反射用例锁死「导出必须静态」
- [ ] N2-37`kgp_atlas_data` 是 KGP 图集的永久 CPU 全量副本且从不读取，
      64MiB 预算下等于内存翻倍
- [ ] N2-42生产代码读 `System.getProperty("test.minSurface"/"test.bootstrapUrl")`
- [x] N2-68`Session::drop` 可阻塞 JNI 调用方约 1.1s —— **本轮否证**：
      `Drop` 只做 `request_exit` + 两次 `join_with_timeout(TRAILING_EXIT_GRACE)`，
      宽限期 50ms（`session.rs:23`），上界 ~100ms；子进程收尾已由 N1-32 提前
      投递 `request_exit`，不再等自然退出
- [ ] N2-6 / N2-40fork 子进程 `setsid`/`TIOCSCTTY` 失败裸 `_exit(2/3)`，
      不写 fd 2，用户只见 `[Process completed (code 3)]`（违反 DESIGN:16/194）
- [x] N2-11`Event::Clipboard` 载荷无上限 —— **本轮否证**：上游回调前已按
      `MAX_CLIPBOARD_PAYLOAD_BYTES`（1MiB）截断并记日志，超限不是静默丢弃
- [ ] N2-8`focus_event` 持 session 锁做 50ms RPC —— Kotlin 侧已改为
      `inputOutput` 调度器派发（`TerminalRuntime.focusChange`，不在主线程），残余是
      焦点切换瞬间最多 50ms 的渲染停顿（有界且低频）；改锁序风险大于收益，本轮不动
- [x] N2-9 / N2-10`ClipboardRead` 被满队列淘汰 —— 本轮已修：
      `EventQueue::push` 优先淘汰可淘汰事件，`ClipboardRead` 不在其中
      （`push_never_evicts_clipboard_read` 两例护栏）；配合 OSC 52 读取失败不再
      回空串（N1-9），远端不会再粘出空白
- [x] N2-12`MAX_SCAN_BYTES` 超限静默丢弃 OSC 52 请求 —— 本轮已修：
      超限时整段原样透传给上游（不吞字节）并记 warning，选择器名超限不再无声
- [x] 吞错批次逐条回读（字体 JNI / 高亮包 / `ensure_frame_texture` /
      搜索 JSON / `InputBatchBuffer` / `chdir`）：搜索 JSON 解码与序列化失败、
      `InputBatchBuffer` 丢弃、`chdir` 失败三处已修并各自记日志；高亮包按
      Kotlin 侧计数前缀为权威长度、损坏计数封顶（防御性解码，非静默错误）；
      `ensure_frame_texture` 失败经 `begin_frame` → surface 失效计数上报；
      字体 JNI 失败见下条（N2-1 已把字族名交回外部库）

## 3. 低危：死代码与死资源（STYLE:63）

- [x] 本轮门禁稳定性：`cargo test` 内的墙钟阈值基准随构建机负载随机判红
      （实测 `bench_bulk_output_throughput` 3337 vs 4000 cells/s、
      `bench_gpu_buffer_upload_throughput` 261 vs 350 MB/s，TESTING.md「没有
      不稳定的测试」）—— 突发输出用例改为断言末行标记（行为），GPU 上传吞吐
      用例删除（`queue.write_buffer` 由每个渲染用例覆盖）；其余基准余量
      5×～40×，保留
- [ ] N1 / N2-32 / N2-33 / N2-34（REVIEW.md、）快照链、键盘编码链、
      `read_visible_text` 生产零调用 —— 本轮回读确认三者只服务测试
      （`take_snapshot` 91 处、`key_encode*` 33 处、`read_visible_text` 3 处）：
      `take_snapshot` 是整套断言的观测入口，删除等于重写测试观测层；`key_encode*`
      更危险：它按上游 `Mods` 位解读修饰键，而本仓 JNI 传的是应用内部位
      （CTRL=4/ALT=2 与上游 CTRL=2/ALT=4 相反），一旦被接线即产生 Ctrl/Alt 互换。
      建议：接线前先删该链，或在入口处显式换算。本轮未动（测试面大、收益低）
- [ ] N14 / N16 / N17`consumeNewOutput`（唯一无 `jni_export_guard!` 的导出）、
      `is_alive`、`render_to_buffer`（约 370 行）、release 活跃的 `feedTerminal` 后门
- [ ] N10 / N11 / N30`ids.xml` 四个 id 零引用、
      37,826 行 baseline profile 两文件逐字相同且基准模块 0 覆盖、
      `publishing { singleVariant("release") }` 空配置

## 4. 需用户裁决（规范之间或与实现的字面冲突）

- [ ] D1 `DESIGN.md:16/24` 禁止未声明回退 vs `TerminalForegroundService.startForeground`
      失败后继续；N1-11 已按抛错收敛，余下是否补规范声明
- [ ] D2 `STYLE.md:61` 禁止 `bash`/`sh` 字面 vs `fmt.yml:44` 在用 `bash -c`
- [ ] D3 / P0-5 五个监控类（`AnrWatchDog`/`BootGuard`/`MemoryMonitor`/`ThermalMonitor`/
      `TerminalForegroundService`）与 `PROHIBITED.md:10` 字面冲突；
      `AnrWatchDog` 触发即 `Process.killProcess` 销毁全部 shell。删整块还是补规范？
- [ ] D5 `public_api.rs` 文档称「二进制 VT 数据应改用 `vt_write`」vs 生产路径仍用 `pty_write`
- [ ] D6 `PROHIBITED.md:19` 禁止内嵌 bootstrap vs `DESIGN.md:126-142` 下载式安装
- [ ] D7 / N7 发布链路（打 tag 产出空 release）
- [ ] N1-23 `BootstrapDownloader` 拒 `http://` vs `DESIGN.md:126`「支持 HTTP/HTTPS」
- [ ] N1-25 粘滞 SCROLL 的产品语义（「再按一次解除」还是「任意输入解除」）
- [ ] N2-48 `bracketedPaste = false` 硬编码，`\e[200~` 从不发出 ——
      是否在本仓范围内实现 bracketed paste
- [ ] P1-4 / D12 被删的输入法跟随测试是否恢复
- [ ] D13（N2-44）DESIGN:153「内容横向溢出到右侧时左右键平移可见区域」的触发
      条件在本仓不存在（网格列数恒为 `floor(surfaceWidth / cellWidth)`）：是给规范
      补一句现状说明，还是删掉该条要求
- [ ] D14（N9/N2-23）`themes.xml` 的窗口/状态栏/导航栏底色硬编码 `#1E1E2E` 且无
      `values-night/`：应用内配色由 Compose 按「日间/夜间/跟随系统」解析（正确），
      只有系统窗口与启动屏是夜色的。修法二选一——运行时按已解析配色设置系统栏，
      或加 `values-night` 资源限定符（后者跟随系统而非应用设置）。两者都要动
      `res/` 或窗口代码，需确认取哪条

## 5. 需授权（修复必然改动保护文件）

- [ ] N1-29 `ktlint`/`ktfmt` 插件已 apply 但无门禁请求（`android/build.gradle.kts`、
      `android/app/build.gradle.kts` 或 `scripts/check-gradle.nu`）
- [ ] D4 `BUILD.md:7`「`ANDROID_NDK_HOME` 已预设」与 `flake.nix` 未声明 NDK 不符
- [ ] D8 `BUILD.md:15-17` 要求的 `.so` 三项校验在 `scripts/build-android-libs.nu` 缺失
- [ ] N8release 变体零冒烟 + `proguard-rules.pro` 的 `-dontoptimize`/`-dontobfuscate`
      与 `isShrinkResources` 自相矛盾
- [ ] N7 / N8`scripts/test-emulator.nu` 先关动画再跑动画基准（恒测 0 并通过）、
      该脚本 `:9` 的 `try` 缺 `catch` 使后续宏基准永不执行
- [x] N9CI 的 markdownlint 递归进 `result-kudzu` —— 本轮已修：
      改用 `.markdownlint-cli2.jsonc` 的 `ignores`（未动工作流与规则集）
- [ ] N25 / N26workflow 无 push/PR 触发器；`check.yml` 30min 超时必然超时
- [ ] **check 工作流收尾失败（本轮实测）**：`check.yml` 新增的 `rm -rf result-kudzu`
      步骤中途删掉了第二个 checkout 目录，而该 job 使用的本地 action
      （`result-kudzu/.github/actions/install-nix`）在收尾仍要跑 Post 步骤，于是报
      `Can't find 'action.yml'`。真正的门禁（fmt/clippy/semgrep/test/rustdoc/
      markdownlint/bench）全部通过，红的只是这一步。修法二选一：删掉 `rm -rf` 步骤，
      或给该 Post 加 `continue-on-error`。两者都要改保护文件 `.github/workflows/check.yml`，
      需用户授权后由用户执行。
- [ ] N29～N36semgrep `fix:` 未绑定 `$SCOPE`、`no-prozu`-族规则口径、
      依赖源顺序、`.gitignore` 无差别忽略 `*.png/*.ttf`
- [ ] N2-52 / N2-53`detekt.yml` 关闭 5 条吞异常/魔数规则；
      `isReturnDefaultValues = true` + 恒返回 0 的 `Log` 桩
- [ ] N2-47 / N31（/）`cjk_resolve` bench 不进门禁（`scripts/check-rust.nu`）
- [ ] N2-59 / N2-60测试注释声称脚本调 `rapidocr` 但脚本内零调用；
      `setup-emulator.nu` 全仓零引用
- [ ] N10 baseline profile `:192` 接线

## 6. 已否证（回读源码确认不成立，记录依据以免重复排查）

- [x] 目录 `FLAG_DIR_SUPPORTS_DELETE` 与实现不符 —— `deleteWithoutFollowingSymlinks`
      先序收目录、逆序删除，目录删除确已实现
- [x] `RenderWatchDog.stop()` 阻塞化 —— 现为非阻塞（`job.cancel()` + `fireLock` 互斥）
- [x] 选区行钳位用错坐标系 —— 已改绝对空间，与 `cachedScrollbackLength` 同源
- [x] `row_cache` / `cached_scrollback` 只写字段 —— 全仓已无这两个字段
- [x] `pty_write` 在非阻塞主端上丢弃剩余字节 —— `write_all` 已改为等可写、绝不丢弃
- [x] `initSession` 空 shell 改写为 `/system/bin/sh` —— 本轮已删除
- [x] `external fun` 缺 `@JvmStatic` —— 本轮已补齐 58 个声明（依赖 receiver 与
      `jclass` 落在同一槽位的巧合，实测 CheckJNI 下报错）
- [x] N2-6 / N2-40fork 子进程裸 `_exit` 不写 fd 2 —— **本轮否证**：
      两处 `_exit` 之前都先 `write(2, reason)`（`child_exit_with_reason` 与 errno
      分支），用户可见 `[Process completed (code N)]` 之前已有原因行
- [x] `kgp_atlas_data` 只写不读的图集 CPU 全量副本 —— 本轮已删除
- [x] `consumeNewOutput` 零调用且是唯一不挂 `jni_export_guard!` 的导出 —— 本轮已删除
- [x] markdownlint 递归进 CI 的 `result-kudzu`（N9）—— 本轮已修
      （改用 cli2 `ignores`，未动工作流与规则集）
- [x] 搜索结果解码/序列化失败冒充「0 匹配」（N2-10）—— 本轮已补日志
- [x] N2-15`RenderWatchDog` 魔数 —— **本轮否证**：轮询间隔与挂起阈值
      已是具名常量（`CHECK_INTERVAL_MS` 与构造参数 `hangTimeoutNanos`）
- [x] N2-26DECCOLM（`CSI ?3h`）后网格自变 132 列、渲染仍按旧列排布 ——
      **本轮否证**：`Terminal.deccolm` 只在 DECSET 40（`enable_mode_3`）置位时才改网格，
      而 VT 侧 `set_mode` 对 40 是空分支（上游 `stream_terminal.zig:717`），本仓与
      `libghostty-vt` 绑定均未调用 `setDeccolmSupported`；网格行列的唯一写者是
      `Session::resize`，而渲染帧读的正是它写的 `grid_size()`，两者恒等
- [x] `SelectionBounds` / `clampSelection` 生产零调用 —— **本轮否证**：
      `TerminalViewModel.dragSelection:478` 在拖动结果抵达原生 `setSelection`
      之前调用它做保序 + 钳位，两者都是活的
- [x] `acceptsDragPointer` 生产零调用 —— **本轮否证**：其规则此前被内联复制在
      `TerminalSurface.onTouchEvent` 的 ACTION_MOVE 分支；本轮改为直接调用该函数，
      消掉重复实现

## 7. （code-review-skill 复审）：本轮新增并已修

依据 [code-review-skill](https://github.com/awesome-skills/code-review-skill) 的
`code-quality-universal` / `common-bugs-checklist` 逐文件复审，全部经回读源码确认。

- [x] **（严重）** `jni_export_guard!` 的失败值形参从未被宏体使用，实际返回
      恒为 `T::default()`：`render` / `renderWithNewOutput` 声明的 `-1` 变成 `0`，
      而 `0` 在 Kotlin 侧意为「空闲帧」，panic 被伪装成一帧无输出。
      本轮改为自实现 `ErrorPolicy`（`ThrowRuntimeExAndFailure`）真正返回调用点声明的失败值
- [x] **（严重）** `renderWithNewOutput` 的采样段在守卫之外调用
      `render_state_mut()`；`render_inner` 刚 panic（GPU 初始化失败）即在此二次 panic，
      越过 `extern "system"` 边界 abort 进程。已并入守卫
- [x] **** `EventQueue::push` 溢出告警记录的是队首而非真正被淘汰的事件，
      而队首常是不可淘汰的 `Exit`/`ClipboardRead` —— 丢失剪贴板被报成从不丢弃的退出事件
- [x] **** `attachWindow` 丢弃 `ANativeWindow_setBuffersGeometry` 的返回码，
      且传给它的尺寸与 `attach_surface` 收到的钳位尺寸不是同一组
- [x] **** `attach_surface` 先建新 surface 再丢弃旧的，与其自身注释声明的
      顺序要求相反（注释记的是模拟器实测的黑屏根因）
- [x] **** `copy_sub_rect` 在像素缓冲越界时静默 `break`，
      图集留半截黑条且无任何日志
- [x] **** `clipboardResult` 的 JNI 字符串解码失败回落到 `""`，被当作用户剪贴板
      为空写回 PTY，远端 tmux/ssh 粘出空白（§1 N1-9 的原生侧孪生点）
- [x] **** `switchSession` 的会话恢复块挂在 `try { target.running = true }`
      上——赋 volatile 字段不抛异常，该 `catch` 不可达；真正会失败的
      `startRenderThread` 反而无恢复，前一个会话永久留在无渲染线程的冻结态。
      已把恢复块移到 `startRenderThread`（并按 收窄到只包它本身）
- [x] **** `TerminalRuntime` 残留 `android.util.Log.w("DbgRecreate", …)`，
      `${'$'}` 转义使插值失效、打印字面量，且不经 `LogUtil` 不受 DEBUG 门控随 release 发布
- [x] **** IME `commitText` 以「文本相等且 80ms 内」丢弃重复提交并返回 `true`，
      吞掉快速连打同一字符；该守卫只在 `composingBuffer` 为空时生效，
      而组字重复本就由 `composingBuffer` 相等检查处理，故它只可能吞合法输入
- [x] **** 主字体为空时回落 `allFonts.first()`（字体库枚举顺序，与 DESIGN 要求的
      fonts.xml monospace 家族无关）；`.orEmpty()` 使原生失败也触发该任意回落
- [x] **** `FontInfoDto.placeholderJson` 合成 `cjkState="none"`、`0×0` 单元格、
      `0sp` 的**有效** JSON，被 `SettingsScreen` 当实测值渲染（DESIGN 字体信息框要求实际值）
- [x] **** `SettingsRepository.DEFAULT_FONT_SIZE`（14sp）与
      `defaultFontSizeFor()`（14–24sp 自适应）分叉，`appliedFontSizeSp()` 在首次推送前
      报 14sp —— 该类 `:103` 已记过一次同类分叉，此处在一层之下复现
- [x] **** `openDocument` 不校验存在性，含 `MODE_CREATE` 的模式
      （`w`/`rw`/`rws`/`rwd`）可经一份 SAF 授权在家目录凭空造文件，
      含 shell 会 source 的 `.mkshrc` 等点文件，绕过 `createDocument` 的创建契约
- [x] **** zip 滑移防护只对原始字符串判前缀，而 `File("foo/..").path` 仍是
      `foo/..`，四个条件一个都不命中 → 该条目被当作普通文件写出，目标其实是 staging 目录本身。
      三处（zip 条目名 / `EXECUTABLES.txt` / `SYMLINKS.txt`）改共用一个先消解点段的判定
- [x] **** `DocumentQueries.resolveLinkEntry` 用词法 `Path.normalize()` 判根内包含性，
      既不消解 `..`，也与既有的 `isHomeLink` 重复；且 `rootDir()` 交出词法路径形式时
      `encodeDocId` 的规范前缀剥离失效，`~` 下每个符号链接都退化成不可打开的幻影条目
- [x] **** `loadFonts()` 无重入门闩：会话状态在字体列表就绪前连续多次发射，
      每次都并发启动一个 `loadFonts`，交错写 `setExtraFontPaths` / `setFontFamily`
- [x] **** `resolveThemeName()` 对同一 `preferences_pb` 做 5 次独立读取与解析
- [x] **（死代码）** Rust：`GpuError::Shader` / `Buffer`、`Session::exit_code_now`、
      `Pty::foreground_pid`、`Pty::wait` / `PtyPair::wait`、`impl io::Read/Write for PtyPair`
- [x] **（死代码）** Kotlin：`isCellEmpty` 整条链（4 个声明 + 1 个 JNI 导出 +
      其仪器化测试）、`ClipboardPaster`（与 `PasteHelper.executePaste` 同形的重复实现，
      中键粘贴还不关选区菜单）、`ConfigurableModifierBar` 纯转发壳、`ModifierBar` 的
      `isActive`/`widthWeight`/`secondaryLabel`/`ToolbarKey.modifier`、
      `TerminalSurface` 的 `codePointCount`/`lastImeBottom`/`resizeDebounceRunnable`、
      `SearchDebouncer.flush`、`InputBatchBuffer.reset`、`BootstrapOrchestrator.processLock`、
      `TerminalApp` 只写不读的 `anrWatchDog`/`thermalMonitor`、`FontUtils` 的 Context 重载、
      `TextWidth` 的 `isWideChar`/`charCellWidth`/`charIndexToCellColumn`
- [x] **** `log::trace!` 在唯一发布平台上恒不可达
      （`logging.rs` 门限为 `Debug`，`Level::Trace > Level::Debug`）——
      **本轮否证为「不改」**：`cell_builder.rs:452` 的注释说明该行选 trace 是为免淹没
      logcat，门限下调会把噪声放回来。5 处调用点与门限保持现状，仅记录该矛盾

## 8. ：新增待办

- [ ] **R16-T1（需裁决）** 多击选择与规范冲突：`TerminalSurface` 完整实现双击/三击/四击
      （选词/选行/全选，`multiTapAction` + `nextTapCount` + 刻意置空的
      `setOnDoubleTapListener`），而 `DESIGN.md:177` 明文「不得支持 双击 三击 多击选择」；
      低置信度的 `openspec/specs/text-selection/spec.md:30` 要求相反。
      按 AGENTS.md 的优先级以 `docs/specification/` 为准，但删掉多击会移除已落地的功能，
      故不擅自动手，请裁决改哪一边
- [ ] **R16-T2（不稳定测试）** `bench_gpu_buffer_upload_throughput`（阈值 350 MB/s）与
      `bench_bulk_output_throughput`（阈值 4000 cells/s）是墙钟吞吐断言，
      536 个测试并行时在共享机器上必然跌破（实测 195 MB/s / 3295 cells/s），
      单独运行恒通过；`sgr_tricolor_mocha_reaches_foreground` 同样只在满载时偶发失败。
      与 TESTING.md「没有不稳定的测试」冲突，但降低阈值即弱化断言，需裁决：
      移入 `cargo bench` 门禁（`scripts/check-rust.nu` 已有 bench 环节）还是串行化执行
- [ ] **R16-T3** 会话锁跨阻塞 PTY 写入：`writeToPty` 持 `session` 锁调
      `Pty::write_all`，该函数最多等可写 5s（`WRITE_DRAIN_TIMEOUT`）；
      同一把锁每帧被 `render_inner` 与 `pollEvent` 取得，
      故向不读 stdin 的子进程粘贴会冻结渲染与输入最长 5s。
      `Session::drain_pty_write_back` 同形（`pollEvent` 在渲染线程上持锁调它）。
      修法需把 PTY 主端 fd 移出会话或改非阻塞应答，属结构性改动，未擅自动手
- [ ] **R16-T4** `pollEvent` 持全局注册表**读**锁遍历全部会话的逐帧工作；
      一个慢会话会把上条的 5s 放大成全局停顿，且 `setScrollOffset` 需要写锁会被饿死。
      改为「读锁内收集 `(id, Arc<Mutex<Session>>)` 后释放再逐个处理」是显然修法，
      但须先落地 R16-T3
- [ ] **R16-T5** 网格尺寸在**入队时**发布：`GhosttyTerminal::resize` 只表示命令已入队，
      `render_inner` 却拿已前移的 `terminal_rows/cols` 配 VT 线程尚未应用 resize 时产出的
      `CellData`；网格收缩时 `build_row_ranges` 返回 `None`，
      于是 `render_cell_data` 报 `CellData conversion failed` 且该帧被丢弃（IME 弹出/旋转必现）。
      修法是随 `CursorInfo` 带上该帧的 rows/cols 而非用原子缓存
- [ ] **R16-T6** 「清除应用数据」（`TerminalViewModel.clearAppData`）未与在途设置写入排序：
      `SettingsRepository` 的写协程有 300ms 防抖且 `replay = 1`，
      用户在改 Bootstrap URL 后 300ms 内点清除，`put` 会在删除之后重建 `preferences_pb`，
      清除静默不生效；`latestBootstrapUrlEdit()` 的 replay 缓存也继续返回已清除的 URL
- [ ] **R16-T7** 字体列表未按 DESIGN「只在实际显示字体列表时获取」加载：
      `loadFonts()` 是会话状态发射的副作用，且列表缓存在 ViewModel
      （随 Activity 消亡）而非进程。修法需把「应用已存字体到 bridge」与「列举字体列表」
      拆开，前者留在会话启动，后者移到字体列表 UI 的 `LaunchedEffect`
- [ ] **R16-T8** `MainActivity.requestPermissions(arrayOf(POST_NOTIFICATIONS), 1)` 用裸平台 API
      与魔数请求码，且不观察结果；应换 AndroidX `registerForActivityResult(
      ActivityResultContracts.RequestPermission())`

## 9. （对 改动的对抗复审）：本轮自查修掉的自身回归

的改动经对抗复审后自行发现并修正三项，均为「修复本身引入或放大的问题」：

- [x] **** 会话恢复块被放进包住 `startRenderThread` **及其后全部步骤**的 `try`：
      其后 `NativeBridge.switchSession` / `syncGridDimensions` / `alignGridOnSwitch` 都是
      会抛的裸 JNI 调用，一旦在其后失败，目标会话的渲染线程已在消费全局事件队列，
      此时再拉起前一个会话就同时存在两个消费者（剪贴板事件错投），
      且 Kotlin 的 `activeSessionId` 与原生 `ACTIVE_SESSION_ID` 会指向不同会话。
      已把恢复块收窄到只包 `startRenderThread`，其余步骤的失败只记日志
- [x] **** `acceptsDragPointer(dragPointerId, lockedIdx)` 比较的是**指针 id** 与
      **槽位下标**——两者在同一手势内并不相等（Android 在 `ACTION_POINTER_UP` 后回收并复用 id）。
      把内联判定换成该函数，等于把「锁定指针仍在事件里」误判成「槽位号恰好等于 id」。
      已恢复内联判定并删除该函数与其测试（它原本是死抽取，契约与实际需求不符）
- [x] **** `escapesStagingDir` 把消解后为空的路径一并拒绝，使合法的 `./` 目录条目
      由无害空操作变成安装中止。已把「消解回 staging 根」从逃逸判定里移出，
      只在 zip 条目处按「非目录条目」单独拒绝
- [x] **** 三处注释与实际不符，已按事实改写：`clipboardResult` 不作答只是让等待方
      立即走 Disconnected 分支回空串（改变的是可诊断性，不是写回内容）；
      `attachWindow` 的宿主并不观察挂载结果，不存在「换新窗口重试」

## 10. ：新增待办

- [ ] **R17-T1（高）** 事件队列可被远端洪泛占满且此后静默丢弃：`ClipboardRead` 由 PTY
      输出驱动（`output_processor.rs:188` 的 OSC 52 读请求），远端脚本循环
      `\e]52;c;?\a` 即可把 1024 个槽位全填成受保护事件；此后每次 `push` 都落到
      「队列只剩受保护事件」分支，连 `Event::Exit` 一起丢——而 `exit_reported` 已置位
      且不会重发，**会话永久泄漏**（原生会话与 shell 子进程都不回收）。
      本轮已给该分支补上告警；根治需给每会话的待答 OSC 52 读请求设上限并显式作答
- [ ] **R17-T2** 同一洪泛的二阶后果：`TerminalRuntime.dispatchClipboardRequests`
      （`:447`）在渲染线程上对每个待答请求各做一次同步
      `ClipboardManager.getPrimaryClip()` binder 调用与一次 `clipboardResult` JNI，
      1024 个积压即一帧内 1024 次 binder 往返。须与 R17-T1 一并设上限

## 11. 文档退役

- [x] 11.1 本台账成文（含真实缺陷、裁决项、授权项、否证项四类）
- [x] 11.2 删除 `docs/REVIEW*.md` 全部 12 个文件
- [x] 11.3 确认无残留引用（`.semgrep/*.yml` 的排除项指向空集，无副作用）
