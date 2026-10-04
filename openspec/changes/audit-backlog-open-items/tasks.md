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
- [x] N2-37`kgp_atlas_data` 是 KGP 图集的永久 CPU 全量副本 ——
      R29 确认该字段已不存在（§6 同名条已登记删除），本条为过期重复，关闭
- [x] N2-42生产代码读 `System.getProperty("test.minSurface"/"test.bootstrapUrl")` ——
      R29 已收敛：两缝线确有调用方（仪器化与应用同进程，`setProperty` 可达），
      但 release 进程无人设置；现仅调试构建读取（`BuildConfig.DEBUG` 门控），
      release 语义零变；
- [x] N2-68`Session::drop` 可阻塞 JNI 调用方约 1.1s —— **本轮否证**：
      `Drop` 只做 `request_exit` + 两次 `join_with_timeout(TRAILING_EXIT_GRACE)`，
      宽限期 50ms（`session.rs:23`），上界 ~100ms；子进程收尾已由 N1-32 提前
      投递 `request_exit`，不再等自然退出
- [x] N2-6 / N2-40fork 子进程 `setsid`/`TIOCSCTTY` 失败裸 `_exit(2/3)` ——
      R29 确认与 §6 同名已否证条重复（两处 `_exit` 前均已 `write(2, reason)`），
      本条为过期重复，关闭
- [x] N2-11`Event::Clipboard` 载荷无上限 —— **本轮否证**：上游回调前已按
      `MAX_CLIPBOARD_PAYLOAD_BYTES`（1MiB）截断并记日志，超限不是静默丢弃
- [x] N2-8`focus_event` 持 session 锁做 50ms RPC —— Kotlin 侧已改为
      `inputOutput` 调度器派发（`TerminalRuntime.focusChange`，不在主线程），残余是
      焦点切换瞬间最多 50ms 的渲染停顿（有界且低频）；改锁序风险大于收益，本轮不动。
      R29 确认维持：同族的搜索/剪贴板持锁停顿已分别经 R21-T1（锁外查询）与
      R17-T2（同帧单次 binder）消除，`focusChange` 只剩查询本身的 50ms 有界等待
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
- [x] N1 / N2-32 / N2-33 / N2-34（REVIEW.md、）快照链、键盘编码链、
      `read_visible_text` 生产零调用 —— R29 裁定保留：三者连同 `is_alive` 与
      `feedTerminal` 是测试观测层（91/33/3 处测试调用；冒烟测试经 `feedTerminal`
      走 JNI 全链），删观测层等于重写测试体系；`key_encode` 按上游位解读而 JNI
      传应用内部位，接线即错——保持「只测不用」现状
      （`take_snapshot` 91 处、`key_encode*` 33 处、`read_visible_text` 3 处）：
      `take_snapshot` 是整套断言的观测入口，删除等于重写测试观测层；`key_encode*`
      更危险：它按上游 `Mods` 位解读修饰键，而本仓 JNI 传的是应用内部位
      （CTRL=4/ALT=2 与上游 CTRL=2/ALT=4 相反），一旦被接线即产生 Ctrl/Alt 互换。
      建议：接线前先删该链，或在入口处显式换算。本轮未动（测试面大、收益低）
- [x] N14 / N16 / N17`consumeNewOutput`（已删，见 §6）、
      `render_to_buffer`（约 200 行回读脚手架）—— R29 收归 `#[cfg(test)]`
      （背景填充/滚动帧像素测试继续用它作无 surface 回读，不进生产二进制），
      超时路径补 `unmap`（R28-T2 一并消失）；`is_alive` 与 `feedTerminal`
      见上条（测试观测层，保留）。`GpuError::Readback` 变体同步门控，
      门控揪出的已死 `GPU_POLL_TIMEOUT` 常量一并收归测试
- [x] N10 / N11 / N30`ids.xml` 四个 id 零引用 —— R29 已删除，
      编译通过。N11 原前提不成立：两文件是 37 行手写规则与 18,913 行采集产物，
      并不相同，且由 AGP 合并进 `assets/dexopt/baseline.prof`（接线见
      `app/build.gradle.kts:108-109`），保留。N30 的 `singleVariant("release")`
      是 baselineprofile 插件的接线要求（非空配置），保留

## 4. 需用户裁决（规范之间或与实现的字面冲突）

- [ ] D1【R29 建议补规范声明（N1-11 已按抛错收敛服务侧行为，只差一句话声明）】`DESIGN.md:16/24` 禁止未声明回退 vs `TerminalForegroundService.startForeground`
      失败后继续；N1-11 已按抛错收敛，余下是否补规范声明
- [ ] D2【R29 建议接受现状：STYLE 的禁令针对 `.nu` 脚本语言选择，工作流调 gradle
      的宿壳不在此列；较真则改 STYLE 一词，需用户改保护文档】`STYLE.md:61` 禁止 `bash`/`sh` 字面 vs `fmt.yml:44` 在用 `bash -c`
- [ ] D3 / P0-5【R29 建议补规范（五类全部已实现、有测试、有生产语义，删整块是功能倒退）】五个监控类（`AnrWatchDog`/`BootGuard`/`MemoryMonitor`/`ThermalMonitor`/
      `TerminalForegroundService`）与 `PROHIBITED.md:10` 字面冲突；
      `AnrWatchDog` 触发即 `Process.killProcess` 销毁全部 shell。删整块还是补规范？
- [ ] D5【R29 实测：扫描器只拦截 OSC 52，其余（含 Kitty 直接 RGB 高字节）全经 `pty_write`
      的 `>0xF7→空格` 整形，损坏是可能但尚未实证；建议保持现状或立项重构 VT 输入路径，
      不建议小步碰】`public_api.rs` 文档称「二进制 VT 数据应改用 `vt_write`」vs 生产路径仍用 `pty_write`
- [ ] D6【R29 建议记无冲突关闭：下载≠内嵌，APK 未预装发行版，实现与禁令一致】`PROHIBITED.md:19` 禁止内嵌 bootstrap vs `DESIGN.md:126-142` 下载式安装
- [ ] D7 / N7 发布链路（打 tag 产出空 release）
- [ ] N1-23【R29 建议保持仅 HTTPS（明文下载引导 zip 是供应链风险）并把 DESIGN 改为
      仅 HTTPS，需用户改保护文档或授权】`BootstrapDownloader` 拒 `http://` vs `DESIGN.md:126`「支持 HTTP/HTTPS」
- [ ] N1-25【R29 建议保持现状混合语义（按钮切换 + 任意输入解除，DESIGN 无声明，
      双行为互补且已有实现），如需收敛请指定】粘滞 SCROLL 的产品语义（「再按一次解除」还是「任意输入解除」）
- [ ] N2-48【R29 建议不实现（无规范声明，STYLE:59 禁止擅加功能；要做需先立项声明语义）】`bracketedPaste = false` 硬编码，`\e[200~` 从不发出 ——
      是否在本仓范围内实现 bracketed paste
- [x] P1-4 / D12 被删的输入法跟随测试是否恢复 —— R29 裁定不恢复旧文件：
      被删的 `ImeLayoutStabilityTest`（366 行）意图（弹出位移/无闪烁/裁剪口径）现由
      `ImePopupPixelInstrumentedTest`（contentFew/contentMany/中文提交，本轮实测
      2/3 通过、剩余 1 例为 AVD 环境所限）与 `ComputeImeSurfaceShiftTest` 覆盖；
      恢复旧文件等于重复锁定同一行为
- [ ] D13（N2-44）【R29 建议给规范补一句现状说明（网格恒不溢出），触发条件不存在，
      删条与实现条都不合适】DESIGN:153「内容横向溢出到右侧时左右键平移可见区域」的触发
      条件在本仓不存在（网格列数恒为 `floor(surfaceWidth / cellWidth)`）：是给规范
      补一句现状说明，还是删掉该条要求
- [ ] D14（N9/N2-23）【R29 建议运行时按已解析配色设置系统栏（跟随应用内日间/夜间开关），
      `values-night` 只跟随系统，不合应用语义】`themes.xml` 的窗口/状态栏/导航栏底色硬编码 `#1E1E2E` 且无
      `values-night/`：应用内配色由 Compose 按「日间/夜间/跟随系统」解析（正确），
      只有系统窗口与启动屏是夜色的。修法二选一——运行时按已解析配色设置系统栏，
      或加 `values-night` 资源限定符（后者跟随系统而非应用设置）。两者都要动
      `res/` 或窗口代码，需确认取哪条

## 5. 需授权（修复必然改动保护文件）

R29 说明：本节 13 项的修法都已在条内写明，全部要求改保护文件
（`.github/`、`scripts/`、`flake.nix`、`build.gradle.kts`、`detekt.yml`、
 semgrep 规则、`docs/specification/`），按规范必须用户亲改或明确授权，
 故本轮只核对现状准确性（D8 缺失与 fmt.yml:44 原样属实），不动文件。
 待授权后按条修，每条独立小步提交。

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
- [x] `external fun` 缺 `@JvmStatic` —— **本条原结论有误，见 **：
      当时只补了部分声明，六个生产导出仍缺，且台账误记为「已补齐 58 个」。
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

- [ ] **R16-T1（需裁决）**【R29 建议保留功能改规范：删已落地的双击/三击/四击是功能倒退，
      以 `docs/specification/` 为准的另一条路是用户把 DESIGN:177 改为允许】多击选择与规范冲突：`TerminalSurface` 完整实现双击/三击/四击
      （选词/选行/全选，`multiTapAction` + `nextTapCount` + 刻意置空的
      `setOnDoubleTapListener`），而 `DESIGN.md:177` 明文「不得支持 双击 三击 多击选择」；
      低置信度的 `openspec/specs/text-selection/spec.md:30` 要求相反。
      按 AGENTS.md 的优先级以 `docs/specification/` 为准，但删掉多击会移除已落地的功能，
      故不擅自动手，请裁决改哪一边
- [ ] **R16-T2（不稳定测试）**【R29 建议移入 `cargo bench` 门禁（`check-rust.nu` 已有
      bench 环节，串行执行阈值即稳定），降低阈值等于弱化断言，不取】`bench_gpu_buffer_upload_throughput`（阈值 350 MB/s）与
      `bench_bulk_output_throughput`（阈值 4000 cells/s）是墙钟吞吐断言，
      536 个测试并行时在共享机器上必然跌破（实测 195 MB/s / 3295 cells/s），
      单独运行恒通过；`sgr_tricolor_mocha_reaches_foreground` 同样只在满载时偶发失败。
      与 TESTING.md「没有不稳定的测试」冲突，但降低阈值即弱化断言，需裁决：
      移入 `cargo bench` 门禁（`scripts/check-rust.nu` 已有 bench 环节）还是串行化执行
- [ ] **R16-T3**【R29 建议维持 park：见本条内评估】会话锁跨阻塞 PTY 写入：`writeToPty` 持 `session` 锁调
      `Pty::write_all`，该函数最多等可写 5s（`WRITE_DRAIN_TIMEOUT`）——
      R29 评估后维持不动：5s 等待是防截断的已验证决策（§6：丢弃会让粘贴被静默
      截断半条命令），调短/丢弃都是回归；根治需把 PTY 主端 fd 移出会话
      （Pty 所有权重构），与收益不成比例。同族的具体停顿已消除
      （R21-T1 搜索锁外化、R17-T2 剪贴板批处理）；
      同一把锁每帧被 `render_inner` 与 `pollEvent` 取得，
      故向不读 stdin 的子进程粘贴会冻结渲染与输入最长 5s。
      `Session::drain_pty_write_back` 同形（`pollEvent` 在渲染线程上持锁调它）。
      修法需把 PTY 主端 fd 移出会话或改非阻塞应答，属结构性改动，未擅自动手
- [x] **R16-T4** `pollEvent` 持全局注册表**读**锁遍历全部会话的逐帧工作 ——
      R29 已修：读锁内只克隆会话 Arc（活跃 + 后台），VT 解析/事件收割全部移出
      锁外逐个处理；销毁竞态已审计（Arc 保活、take 锁存恰好一次、Kotlin 侧未知
      会话幂等回收），`setScrollOffset` 写锁不再被饿死；
      一个慢会话会把上条的 5s 放大成全局停顿，且 `setScrollOffset` 需要写锁会被饿死。
      改为「读锁内收集 `(id, Arc<Mutex<Session>>)` 后释放再逐个处理」是显然修法，
      但须先落地 R16-T3
- [x] **R16-T5** 网格尺寸在**入队时**发布 —— R29 已修：`CursorInfo` 新增
      `rows`/`cols`（VT 产出本帧时的真实网格，与 CellData 同源），帧装配改读它，
      不再读提前发布的原子缓存；附同源回归测试两例（默认网格 + resize 后）。
      修法即台账预告的方案；`GhosttyTerminal::resize` 只表示命令已入队，
      `render_inner` 却拿已前移的 `terminal_rows/cols` 配 VT 线程尚未应用 resize 时产出的
      `CellData`；网格收缩时 `build_row_ranges` 返回 `None`，
      于是 `render_cell_data` 报 `CellData conversion failed` 且该帧被丢弃（IME 弹出/旋转必现）。
      修法是随 `CursorInfo` 带上该帧的 rows/cols 而非用原子缓存
- [x] **R16-T6** 「清除应用数据」（`TerminalViewModel.clearAppData`）未与在途设置写入排序 ——
      R29 已修：仓库新增互斥 + 停用闩（`dropPendingBootstrapUrlEdits` /
      `rearmBootstrapUrlEdits`），清除前停防抖写入并清 replay，完成后
      `finally` 重开；在途 `put` 要么先完成（产物随即被删）要么看到停用跳过；
      附 700ms 真实睡眠回归测试；
      `SettingsRepository` 的写协程有 300ms 防抖且 `replay = 1`，
      用户在改 Bootstrap URL 后 300ms 内点清除，`put` 会在删除之后重建 `preferences_pb`，
      清除静默不生效；`latestBootstrapUrlEdit()` 的 replay 缓存也继续返回已清除的 URL
- [x] **R16-T7** 字体列表未按 DESIGN「只在实际显示字体列表时获取」加载 ——
      R29 已拆：会话发射只调 `applyStoredFontSettings`（bridge 按会话持有，
      每个新会话都需应用一次，比原来只在列表为空时应用更正确），枚举收归
      `refreshFontList` 由外观项 `LaunchedEffect` 触发（LazyColumn 懒加载
      即精确的「显示时获取」）；498 单测全过；
      `loadFonts()` 是会话状态发射的副作用，且列表缓存在 ViewModel
      （随 Activity 消亡）而非进程。修法需把「应用已存字体到 bridge」与「列举字体列表」
      拆开，前者留在会话启动，后者移到字体列表 UI 的 `LaunchedEffect`
- [x] **R16-T8** `MainActivity.requestPermissions(arrayOf(POST_NOTIFICATIONS), 1)` 用裸平台 API ——
      R29 已换 AndroidX `registerForActivityResult(RequestPermission())`
      （随已声明的 activity-compose 到达，未新增依赖），拒绝仍不影响终端功能；
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

- [x] **R17-T1（高）** 事件队列可被远端洪泛占满且此后静默丢弃 —— R29 已修：
      单会话未作答 OSC 52 读设上限 8（`MAX_UNANSWERED_CLIPBOARD_READS_PER_SESSION`），
      超限不占槽、不推事件、不起线程，就地显式作答空串（xterm 兼容），告警 5s 限频；
      槽位、线程、保护事件三者都不再增长，Exit 永远有位置。附按会话隔离计数测试；`ClipboardRead` 由 PTY
      输出驱动（`output_processor.rs:188` 的 OSC 52 读请求），远端脚本循环
      `\e]52;c;?\a` 即可把 1024 个槽位全填成受保护事件；此后每次 `push` 都落到
      「队列只剩受保护事件」分支，连 `Event::Exit` 一起丢——而 `exit_reported` 已置位
      且不会重发，**会话永久泄漏**（原生会话与 shell 子进程都不回收）。
      本轮已给该分支补上告警；根治需给每会话的待答 OSC 52 读请求设上限并显式作答
- [x] **R17-T2** 同一洪泛的二阶后果 —— R29 已修：`dispatchClipboardRequests`
      每帧只读一次剪贴板（binder 1024→1），同一结果逐个 JNI 回复；
      单线程同步派发无交错，逐请求映射与原来逐字相同；`TerminalRuntime.dispatchClipboardRequests`
      （`:447`）在渲染线程上对每个待答请求各做一次同步
      `ClipboardManager.getPrimaryClip()` binder 调用与一次 `clipboardResult` JNI，
      1024 个积压即一帧内 1024 次 binder 往返。须与 R17-T1 一并设上限

## 11. ：新增并已修

- [x] **** 宽字符的 `cell.style()` 失败路径按 1 列推进 `current_col`，绕过了
      `raw.wide()`：该宽字符之后整行的 `col` 左移一格且行长不再等于 cols。
      已抽出 `cell_columns` 并让失败占位也走真实宽度（`raw_cell()` 失败时宽度无从得知，
      仍只能按 1 列，已在注释中写明这是该路径的固有上限）
- [x] **** `cjk.rs` 的轮廓探测用 `raster_scale.max(1.0)`，而 `atlas.rs` 用
      `raster_scale`：Kotlin 允许 0.5f..4f，故 `raster_scale < 1` 时探测尺寸与图集尺寸不同，
      「是否内嵌 bitmap strike」的结论在两个尺寸间翻转，CJK 回退随之误判。已对齐
- [x] **** `read_line_text_impl` 在 `grid_ref`/`cell` 失败时少产一个字符，
      而 `search_in_scrollback_all_impl` 把段内字符下标直接当列号 →
      该行之后所有搜索高亮左移一格，且偏移经软换行段累加继续放大。已改为每列必出一个字符
- [x] **** `Surface.postDelayedSurfaceRecreate` 的重试链不挂字段，
      `onDetachedFromWindow` 取消不了；它每次尝试都 `setRenderPaused(false)` +
      `resumeRendering()`，跨越 ON_PAUSE 即撤销刚请求的暂停，
      在已被系统回收的 BufferQueue 上继续出帧（永久黑屏）。已挂字段并新增
      `cancelSurfaceRecreate()`，由 `TerminalScreen` 的 ON_PAUSE 调用
- [x] **** `resetScrollOffset()` 只重置偏移，不失效回滚长度缓存，
      切会话后 100ms 节流窗口内 `currentViewportTopGrid()` 用上一个会话的回滚长度算行号，
      长按/拖手柄锚到无关的回滚区。已一并失效节流
- [x] **** `onSingleTapUp` 先跑 `handleMultiTap` 再判 `isAfterLongPress`：
      「轻击 → 长按拖动抬手」落在 400ms 窗口内会被计为第 2 击，
      用抬手坐标选词覆盖长按选区，且 `isAfterLongPress` 滞留使下一次真正轻击被吞。
      已把长按抬手提到计数之前
- [x] **** `performClick()` 在抽屉遮罩/边缘手势的抬手上也被调用。
      已移到抽屉早退之后（注释如实写明滚动与长按抬手仍会播报）
- [x] **** `acceptsDragPointer` 把**指针 id** 与**槽位下标**比较
      （Android 在 `ACTION_POINTER_UP` 后回收并复用 id）。已恢复内联判定并删除该死抽取
- [x] **** `detectDpkgVersion` 在 `waitFor` 之后同步 `readText()`：
      dpkg 的任何后代进程若继承并持有 stdout 管道，`readText()` 永不返回，
      `finally` 的 `destroy()` 与其后的锁文件清理都执行不到。已改为守护线程并行排空
- [x] **（严重）** **台账 §6 的「`external fun` 已补齐 58 个 `@JvmStatic`」与事实不符**：
      `setTheme` / `setFontFamily` / `loadFontFile` / `setExtraFontPaths` /
      `getCellHeight` / `setScrollOffset` 六个生产导出仍缺注解。原生侧全部是静态签名
      （形参第二位 `_class: JClass`），缺注解时 JVM 绑定实例方法，
      第 3 个 JNI 槽位传 `this` 而原生按 `jclass` 接收——两者都是 64 位且 `_class` 从不解引用，
      故只有 debuggable 构建的 CheckJNI 会报参数类型错误。已补齐，
      并新增 `NativeBridgeStaticExportsTest` 以反射断言全部导出都带注解
- [x] **** `entry.lastRenderDone` 只在 `count >= 0` 分支刷新：
      渲染器持续返回 -1（Surface 迟迟不就绪）时完成时刻冻结在最后一帧成功处，
      10s 后 `RenderWatchDog` 把仍在循环的线程判为挂死，
      反复重启直至 `closeDeadSession` 关掉用户的 shell。失败帧与异常帧现都刷新
- [x] **** `currentScrollbackLength()` 在查询**之前**写节流时间戳：
      bridge 缺席或 `scrollbackLength()` 抛错时白白烧掉一个 100ms 窗口。已改为成功后推进
- [x] **** 全回滚区正则搜索是阻塞 JNI，却跑在 `Dispatchers.Main.immediate` 上：
      5 万行回滚上以秒计，输入法与按键一起卡住，`AnrWatchDog` 还会 `killProcess` 掉所有 shell。
      已移入 `TerminalDispatchers.inputOutput`
- [x] **** 引导 URL 输入框被 `LaunchedEffect(bootstrapUrl)` 用回流值覆盖：
      每次击键经 300ms 防抖落盘，回流带着**上一次**的值抵达，抹掉刚敲进去的字符。
      已改为 `remember(bootstrapUrl)`（与 `ShellInput` 同款）
- [x] **** `clearAppData` 丢弃 `deleteRecursively()` 的返回值却在 UI 上无条件
      显示「已清除」：删除失败时 `settings.preferences_pb` 原封不动。已检查返回值，
      有残留则记错误日志且不回调成功
- [x] **** `clearAppData` 删除 `boot_state` 却不重建：`BootGuard.writeCounter`
      无 `mkdirs()`，崩溃循环计数自此静默失效。已与其余监视目录一并重建
- [x] **** 切换搜索大小写敏感度时只 `cancel()` 了在跑的协程，没取消待执行的防抖搜索：
      后者带着**切换前**的敏感度在 150ms 后启动并覆盖 `searchState`，
      界面显示「区分大小写」却配着不区分的结果集
- [x] **** `RenderWatchDog.start()` 的「已在跑则返回」检查与启动不是原子的，
      两个并发调用者可各起一个 2s 轮询循环。已并入同一临界区
- [x] **** `FrameTimingStats.record` 无边界检查：调用方未先排空窗口就越界写，
      `ArrayIndexOutOfBoundsException` 抛在**渲染线程**上，整个终端随之消失。
      已改为窗口满时丢帧并记日志
- [x] **** `ThermalMonitor.register()` 不幂等：重复调用多挂一个监听器与一个线程，
      `onCritical` 触发两次
- [x] **** `escapesStagingDir` 把消解后为空的路径一并拒绝，使合法的 `./` 目录条目
      由无害空操作变成安装中止。已把「消解回 staging 根」移到 zip 条目处按条目种类判定
- [x] **** `snapshot_needs_rebuild` 的注释称签名里有「滚动偏移」，实际没有；
      视口偏移靠 `Command::ScrollViewport` 自行置脏。注释已按事实改写
- [x] **** `attachWindow` / `clipboardResult` 两处注释与实际行为不符（声称存在
      宿主重试 / 声称改变写回内容），已按事实改写

## 12. ：新增待办

- [x] **R21-T1（高）** 全回滚区搜索在原生侧是**持会话锁**的同步查询 —— R29 已修：
      查询通道（`query_tx`，可克隆）只在锁内取出，VT 查询本身在锁外跑；
      `query` 样板下沉为 `query_on` 通道外置版，`search_all_in_scrollback` 委托它，
      零语义漂移（附双路径一致性测试）；渲染每帧取锁不再被大搜索冻结；
      （`ffi.rs:2523` 的无界 `parking_lot` 锁 + `QUERY_TIMEOUT_MS = 500` 的等待，
      且调用方超时后 VT 线程仍在继续搜索）。该锁每帧被 `render_inner` 与 `pollEvent` 取得，
      故一次大回滚搜索会让**渲染**停顿数百毫秒乃至数秒（只把它从主线程挪到了 IO 线程，
      没有消除锁竞争本身）。根治须把搜索移出会话锁之外（结果交回后由渲染线程消费）
- [x] **R21-T2** `TerminalForegroundService.start()`（`MainActivity.kt:188`）在任何会话 ——
      R29 已修：无 extra 的裸启动沿用上次已知计数（冷启动为 0 即「启动中」，
      新增 `notification_starting` 文案），不再回落成 1；0 会话不持唤醒锁；
      附裸启动/显式 0 的通知文案 + 无锁断言两例；
      出生之前就启动，intent 不带 `session_count`，而 `service:90` 用
      `coerceAtLeast(1)` → 冷启动时常驻通知恒称「1 个活动会话」，
      `acquireWakeLockIfNeeded()` 也为不存在的会话持有 `PARTIAL_WAKE_LOCK`。
      `coerceAtLeast(1)` 还让合法的 `session_count = 0` 与「未设置」不可区分
- [x] **R21-T3** `ThermalMonitor` 没有 `unregister()` —— R29 已补 `unregister`
      （与兄弟监视器的 start/stop 对齐）并加生命周期契约测试
      （注销空操作安全、注册→注销→重注册全程不抛、决策逻辑不受影响）；
      只保证了 `register()` 幂等，注销路径仍缺
- [x] **R21-T4** `InputBatchBuffer.write()` 的 `send(chunk)` 在 `synchronized(lock)` 之外 ——
      R29 已修：入队（`sender.execute`，无界队列永不阻塞）收进锁内
      （`sendLocked`），排空与入队原子；中转 `toSend` 列表一并消除；
      既有 9 例保序测试全过；
      两个并发写入者可以按 X、Y 排空却按 Y、X 入队；类 KDoc 声称的顺序保证只对单写入者成立
      （当前全部写入点都在主线程，故为潜在而非现存缺陷）
- [x] **R21-T5** `ReadScan::Selection` 在 OSC 未闭合时会把后续约 59 字节非 BEL/非 ESC 的 ——
      R29 否证：流式消歧的固有行为（`\e]52;cc` 后未见 `;` 之前无法判定读写，
      推测性发射在字节流上不可能），顺序不变、延迟以 `MAX_SCAN_BYTES` 为界
      （N2-12 已让超限透传并出声），不改；
      输出吞进 `buf` 再吐出（顺序不变，仅增加延迟）；与已登记的 N2-12 同族但后果不同
- [x] **R21-T6** `OutputSnapshot.clipboard_read` 在单个 `process()` 块内 last-wins ——
      R29 已修：块内改为 `clipboard_reads` 有序数组，会话槽改为 FIFO 队列
      （与写侧对称），JNI 逐帧取走全部；flood 由 R17-T1 上限显式作答，
      故深度有界。附同块双读全保留测试 + BDD 步骤适配；
      分块读取时多个 OSC 52 读请求只留一个。与 §10 R17-T1/T2 同族（读请求被丢弃而不作答）

## 13. ：已修

- [x] **（自身回归）** `clearAppData` 把「重建监视目录」放在「有残留则返回」之后：
      删除是逐目录独立的，`prefs` 删成功而 `boot_state` 残留时就此返回，
      进程级 DataStore 单例因父目录消失而此后所有设置写入全失败。已改为无条件重建
- [x] **** `clearAppData` 的注释与代码相反（`.orEmpty()` 把 `listFiles()` 的
      I/O 失败当成「目录为空」）。已改为按未清除记账，并让日志措辞覆盖两种情形
- [x] **** 待挂载 Surface 的三元组（`pendingSurface` + 宽 + 高）是三个
      `@Volatile` 字段：读者可能拿到新 Surface 配旧尺寸，而 `attachSurface` 的尺寸
      必须与该 Surface 同源，否则交换链配置与 BufferQueue 几何各说各话。
      已收进单个 `@Volatile private class PendingSurface`
- [x] **** `SessionEntry.renderWatchDog` 是整条记录里唯一没有 `@Volatile` 的字段，
      而它在 `stopDeadRenderThreadResources`（锁外）被写、在 `startRenderThread`（锁内）被读，
      没有 happens-before 边：读到陈旧 null 就会漏停一个 2s 轮询循环。已加 `@Volatile`
- [x] **** `recomputeGridFromFontMetrics` 分两次读 pending Surface：
      可能把 1080x2400 与 1080x2340 拼成一个从未存在过的矩形，由此算出的网格会以
      SIGWINCH 推给 PTY 却与任何 Surface 都不匹配。已一次取出三元组
- [x] **** 换视图预算（次数 / 上次时刻 / 已告警）是三个 `@Volatile` 字段，
      由渲染线程与 `surfaceTransitionExecutor` 各自成组读写。已收进
      `SessionEntry.SurfaceRecreateBudget` 并整条替换
- [x] **** `loadFontFile` 丢弃 `set_font_family` 的结果：设置页显示新字体名，
      终端仍用管线默认字体渲染，且无任何日志（`setFontFamily` 导出对同一结果上报 false）。
      已改为记错误并回 null（Kotlin 侧回落用户已存族名，不崩溃）
- [x] **（自身回归）** 上一条的提前返回跳过了其后的
      `cell_cache = None` 与 `dirty.store(true)`，留下「新管线已装、旧实例缓存仍在、
      且没请求新帧」的三重不一致。已把两步提到检查之前
- [x] **** `listFontFamilies` 的数组元素初值为 null，而任一 `new_string` /
      `set_element` 失败都会留下 null；Kotlin 侧声明是 `Array<String>`，
      null 元素会在字体选择器里变成 NPE。已改为失败即整体作废
- [x] **** `SettingsRepositoryTest` 有一个空函数体测试：不断言任何行为，
      把 `SettingsRepository.kt` 整个删掉它照样通过。已删除
- [x] **** `FrameTimingStats` 的「窗口满则丢帧」分支无测试覆盖，
      删掉守卫整套测试仍全绿。已补一个断言越界帧被丢弃且不污染窗口的用例
- [x] **** 引导 URL 字段两轮改动的两次都不对：`LaunchedEffect` 无条件覆盖与
      `remember(bootstrapUrl)` 重新播种都挡不住「回流带着更旧的文本抵达」
      （用户在 300ms 写入窗口内继续打字 → 字符被静默删除且不经 `onValueChange`）。
      已回到「编辑中不采纳外部值」的守卫，并确认四个方向（打字、预设按钮、
      清除应用数据、重组）都成立
- [x] **** `clearAppData` 的失败日志措辞写成「未删除」，而合成条目
      `cache(不可枚举)` 表示的是 I/O 失败。措辞已覆盖两种情形

## 14. ：新增待办

- [x] **R25-T1** 换视图预算仍是跨线程读-改-写 —— R29 已修：预算记录装进
      `AtomicReference`，写回一律 CAS（复位落地则按新值重判，旧计数写不回去）；
      复位改整条 `set`。`decideSurfaceRecreate` 纯决策 6 例不受影响；`maybeRequestSurfaceRecreate` 读快照后写回，
      期间落地的复位（`resumeRendering` 在 `surfaceTransitionExecutor` 线程、渲染线程仍活着时执行）
      会被本次写回覆盖，计数被推回旧值，下一次失效即判为 exhausted，
      终端保持空白直到平台重新交付 Surface——正是 `TerminalRuntime:3180` 那段注释要治的病。
      把预算的读-改-写并入 `sessionLock`，或改用 `AtomicReference<SurfaceRecreateBudget>.updateAndGet`
- [x] **R25-T2** `RenderWatchDog` 以 `getStart()` / `getDone()` 两读判定「帧在飞」 ——
      R29 已修：起止收进单条 `FrameMarks` 记录（渲染线程唯一写者，整条发布），
      看门狗一次读出同一帧；构造器改 `getMarks`，5 例测试同步更新，全过；
      二者是 `SessionEntry` 上两个独立 `@Volatile`。跨帧混读可造出假的 `start > done`，
      若同时已超 10s 即误报挂起并重启健康的渲染线程。窗口只有一帧宽且需真的长帧，
      目前自限，但与 R25-T1 同族

## 15. ：已修

- [x] **** `resolveThemeName()` 返回的是清除**之前**的快照，
      而 `buildConfig` 随即用 `BuiltInThemes.byName`（未知名上 `error`）解析它：
      存有未知名的那一次建会话直接失败，用户看不到终端，要等下一次 surface 分支才恢复。
      UI 半边早已用 `byNameOrNull(...) ?: draculaPlus` 容忍同一情形，两半不一致。
      已改为清除成功后重读快照（`clearUnknownThemeNames` 返回是否真的清除过）
- [x] **** `TerminalState.sessionId` 是只写不读的死状态（5 处写、0 处读）。已删除
- [x] **** `DocumentQueries.resolveLinkEntry` 在 `decodeDocId` 之后又做一次
      根内校验，而 `decodeDocId` 内部已无条件校验并返回规范化 File：
      同一次判定多两次文件系统往返。已删除重复
- [x] **（自身回归）** 为「空载荷 OSC 52」加的告警落在 `EventDispatcher`，
      而 `Bridge.parseEvent`（`Bridge.kt:365`）已把 `""` 映射成 `null`：
      该分支不可达，序列仍被静默丢弃，注释还宣称可达。已删除该死分支，
      改在台账登记（见 D13）
- [x] **** `keymap.rs` 的测试注释拿 `KEYCODE_SYSRQ` 当「未映射」的反例，
      而它其实映射为 `PrintScreen`（200 是 `KEYCODE_CAPTIONS`）。已更正
- [x] **** `keymap.rs` 的 `every_android_code_has_unique_mapping` 实测不了唯一性
      （两个 Android 码映射到同一 `Key` 也会通过），其真实价值只是「表与 match 分支同步」。
      已按实际语义改名
- [x] **** `glyph_cache.rs` 自述「独立于 FontPipeline 以便单独测试淘汰策略」，
      但全仓没有任何用例跨越过容量，淘汰策略零覆盖。已补：
      跨 `OUTLINE_CACHE_CAPACITY` 断言被淘汰的正是最久未用的一条，
      且刚被 `get` 触碰的条目存活（`peek` 不刷新 LRU 次序，故必须用 `get`）

## 16. ：新增待办

- [ ] **D13（需用户裁决）**【R29 建议实现清空语义需先定三态合并规则，见条内】OSC 52 的空载荷（`\e]52;c;\a`）在 xterm 语义里是
      「清空剪贴板」。当前 `Bridge.parseEvent` 把空串映射成 `null`（null = 本帧无剪贴板事件），
      于是该序列被静默忽略。`DESIGN.md` 只声明「通过终端序列（OSC 52）与用户交互
      读写系统剪贴板」，未声明清空语义；按「不允许实现任何未在 docs/specification/
      声明的功能」本轮未实现。要清空需要把 `PollResult.clipboard` 从 `String?`
      扩成能区分「无事件 / 写入文本 / 清空」的三态，并明确空载荷优先于同帧文本的合并规则。
      请裁决是否在本仓实现
- [x] **R27-T1** `Bridge.parseEvent` 与 `resolveThemeName` 都没有单元测试覆盖 ——
      R29 已全修：`resolveThemeName` 的选择逻辑提为顶层纯函数 `selectThemeName`
      并补 5 例；`Bridge.parseEvent`（私有实例方法）经反射补 6 例映射测试
      （空剪贴板→null、未知退出码保持 null、读请求/振铃字段）；
      （前者 private、后者需要完整 runtime）；本轮三处改动全靠人工推演验证。
      `resolveThemeName` 的重读逻辑可提取为纯函数以便测试

## 17. ：已修与待办

- [x] **** `test_helpers.rs` 的 `assert_background_exact` 挂着
      `#[allow(dead_code)]` 与「并非每个测试二进制都调用每个方法」的注释，
      而它在本文件内就被调用两次——抑制什么都没守住，注释也与事实相反。
      已删除抑制（AGENTS.md 禁止用 `#[allow]` 掩盖）
- [x] **** `MockPtyHandle` 的 `inject_output` / `drain_written` / `resize`
      全仓零调用；`output_buffer` 只由 `inject_output` 填充，故 `MockPty::read` 的
      「部分拷贝 + 余量回推」分支永不可达。已删掉这三个方法与 `output_buffer` 字段，
      `read` 改为直陈替身不承载输出侧（输出经 `try_clone_reader_fd` 的 `/dev/null`）
- [x] **** `handleSessionExit` 的 `aliveMs` 形参无人消费（整条 alive_ms 链路的
      现状见 R28-T1）。已在形参注释中写明，并把它落到 debug 日志——
      参数不再「传了但完全不可见」，也不再需要任何抑制属性

- [x] **R28-T1** `alive_ms` 是端到端死链路 —— R29 已删：原生测量→事件序列化→
      Kotlin 三级（`PollEvent`/`PollResult`/`ExitInfo`）→形参→日志整条清除，
      共 6 个 Rust 文件（`spawned_at` 一并消失）、3 个 Kotlin 文件与合并测试；
      `[Process completed]` 只含退出码的语义不变；544 + 43 单测全过；原生在 `session.rs:322` 测量存活时长、
      `ffi.rs:1173` 随退出事件发出、`event.rs:36` 序列化（且有 JSON 断言），
      Kotlin 侧 `PollEvent.Exit.aliveMs` → `PollResult.exitAliveMs` → `ExitInfo.exitAliveMs`
      → `handleSessionExit` 形参，最终无人消费（`[Process completed]` 提示按 Termux
      只含退出码）。彻底清理要同时改 6 个 Rust 文件、3 个 Kotlin 文件与十余处测试断言；
      与 §3 已登记的 N1 / N2-32~N2-34 三条同类死链路一并处理更合适。
      本轮只让形参可见，未擅自改动事件 schema
- [x] **R28-T2** `render_to_buffer`（`pass.rs:884`）在 `map_async` 超时路径上返回 ——
      R29 已修：超时前补 `dst.unmap()`（见 N14/N16 条：函数整体收归 `#[cfg(test)]`）；
      `Err` 而不 `unmap`，该读回缓冲随后永久处于 mapped 态；下次调用对其
      `copy_texture_to_buffer` 会被 wgpu-core 拒收（mapped 缓冲不是合法拷贝目标），
      乃至设备丢失。`MAP_READBACK_TIMEOUT` 只有 100ms，慢机上可达。
      该函数生产零调用（§3 N14/N16/N17 已登记待删），故本轮不修——随其删除一并消失

## 18. 文档退役

- [x] 15.1 本台账成文（含真实缺陷、裁决项、授权项、否证项四类）
- [x] 15.2 删除 `docs/REVIEW*.md` 全部 12 个文件
- [x] 15.3 确认无残留引用（`.semgrep/*.yml` 的排除项指向空集，无副作用）

## 19. fix 分支合入审计（逐项核对无内容丢失）

- [x] **M-1** 远端 fix 分支与 main 自 `48a0364e` 分叉后各走 36/48 个提交，
      30 对提交 patch-id 相同；余下 6 对同名提交逐 patch 比对，fix 侧一律是旧变体
      （单槽剪贴板、`Sender<String>` 应答、无修饰键鼠标、别名归并字族、
      墙钟断言的性能测试），main 侧均为更新、已复审的改进——未从 fix 侧回灌任何旧代码
- [x] **M-2** `main→fix` 整树 diff（38 文件）逐 hunk 核对：fix 侧的「新增」只有三类，
      均不合入——① main 已故意删除（`bench_gpu_buffer_upload_throughput` 墙钟断言、
      字族别名归并，违反 TESTING/DESIGN 且删除有据）；② fix 侧缺失的 main 改进
      （FIFO 剪贴板队列、`clipboardResult(..., null)`、鼠标修饰键、会话下沉写、
      仪器化加固），main 保留；③ fix 侧删除而 main 保留的回归测试予以保留
- [x] **M-3** 唯一真正合入项：fix 侧已把 `NativeBridgeStaticTest` 合并为
      `NativeBridgeStaticExportsTest`，main 侧两者并存属重复锁定同一不变量。
      已删除前者，后者的注解扫描＋历史注记继续锁死「导出必须静态」
- [x] **M-4** 作者核查：`21291bc..main` 全部提交作者均为 jane，无
      `Co-authored-by`/`Signed-off-by`，无线性之外的合并提交；committer 已统一为
      jane（与该基线之前的库内惯例一致）。fix 分支含 kyehn 署名提交，不进入 main；
      合入完成后删除远端 fix 分支，本地留 tag 备查（内容已由 main 全覆盖）

## 20. ：新增并已修（code-review-skill 逐项复审驱动）

- [x] **（严重）** `loadFontFile` 的 `if let Err(..) = set_font_family(..)`：
      该函数返回 `bool` 而非 `Result`，`#[cfg(target_os = "android")]` 块内的
      类型错只在 Android 目标暴露，宿主 `clippy/test` 全绿掩盖——main 的 Android
      产物自 起不可编译。已按 `setFontFamily` 导出同款语义改 `if !`。
      构建即回归测试（`cargo ndk` 全量编过）
- [x] **** `GPU_POLL_TIMEOUT` 全仓零引用（测试侧自带同名常量）：
      收归 `#[cfg(test)]` 时被 `deny warnings` 揪出，已一并门控
- [x] **** 依赖过期：`cargo update --dry-run` 显示 7 个兼容补丁落后
      （cc/font-types/libc/shuttle/shuttle-engine/tokio/yoke-derive），
      无大版本待升（BUILD.md:21 合规）。已升并重跑 544 单测
- [x] **** 外部依赖替代扫描结论：无可替的手搓实现——LRU/字体库/XML/JSON/
      base64/防抖/序列化/注入全部已走外部库；裸平台 API 只剩 bridge 必需的
      `System.loadLibrary`；通知权限已换 AndroidX 契约（R16-T8）。
      Gradle 侧 pins 均为跟踪中版本（alpha/RC 系按规范要求取最新），不动
- [x] **** 本轮 17 个提交的 code-review-skill 自查（Rust 清单 + 通用质量）：
      零新增 `unsafe`/`unwrap`/`allow`/TODO，主线程零 `Thread.sleep`，
      锁序文档与实现一致（R16-T4/R21-T1/R16-T6 三处重排已审计），
      危险语义（空剪贴板 vs 读取失败、退出码 null vs 0）零改动。
      测试注释的断言一律是行为断言，无空断言（口径）

## 21. ：已修

- [x] **（CRITICAL）** `ffi.rs` 的 `loadFontFile` 写 `if let Err(..) = set_font_family(..)`，
      而该函数返回 `bool`——**Android 目标根本不编译**（E0308）。此前所有门禁只跑宿主目标，
      `#[cfg(target_os = "android")]` 分支从未被编译过。同批另有两处 Android-only
      clippy 告警（`list_monospace_fonts` 的多余 `return`、`setExtraFontPaths` 的可折叠 if）。
      三处全修，并把 `--target x86_64-linux-android` 纳入本轮验证
- [x] **** `cell_builder` 的两遍发射循环之后无条件 `cache.update(..)`，
      把**当前**代际盖在旧 UV 的实例上；末遍若再次驱逐，缓存即被污染，
      下一帧增量判定误判为同代际→干净行照着已搬迁的 UV 采样。改为仅末遍代际稳定时写回
- [x] **** `render_to_buffer` 的超时/通道断开/map 失败三条路径都不 `unmap`，
      读回缓冲永久停在 mapped 态，后续 `copy_texture_to_buffer` 会被 wgpu 校验拒绝。
      改为无论成败都 unmap（数据在闭包内已拷出）
- [x] **** `TerminalRuntime.switchSession` 在运行期已中止切换时
      （会话不存在、surface 失效、首帧期间被关、渲染线程起不来），
      ViewModel 仍无条件发布 `activeSessionId`，与 `runtime.state`/`inputTargetSessionId` 分裂。
      `switchSessionInternal` 改返回 Boolean，ViewModel 据此决定是否发状态，
      并把 surface 读取移进协程（对齐 `createSession`）
- [x] **** `TerminalForegroundService.onTaskRemoved` 无条件取唤醒锁，
      裸 `start()` 冷启动（`sessionCount == 0`）会为不存在的会话持锁。补上与
      `onStartCommand` 同一判据；`MainActivity` 里与之矛盾的注释一并纠正
- [x] **** `DocumentMutations.copyDocument` 缺 `moveDocument` 已有的子孙守卫：
      把目录复制进自己的子孙会 `a/b/a/b/…` 递归到 ENAMETOOLONG/磁盘写满才回滚。已补
- [x] **** `BootstrapInstaller` 对 `EXECUTABLES.txt`/`SYMLINKS.txt` 只查
      `escapesStagingDir`，未查「消解后为空」——`foo/..` 通过校验，
      `Os.chmod`/`Os.symlink` 于是作用于 staging 目录自身。两处补齐该判据
- [x] **** `SecondStageRunner.runOnePostinstAttempt` 的 catch 无条件往 `errors` 追加，
      与超时/非零退出「只记末次」的判据不一致：首运异常、重试自愈仍被报成安装失败
- [x] **** `TerminalSurface.isWhitespaceCell` 把**单元格列号**当下标取 `line[col]`，
      而宽字符占两格——CJK 行上宽字符后半格恒越界，长按选词恒退化为仅粘贴菜单。
      抽出 `charIndexAtCellColumn` 作为「单元格列↔字符下标」的唯一规则，
      `snapColToWideChar` 与它共用
- [x] **** `ModifierBar` 自动重复用墙钟，而同一文件的长按阈值明确改用单调 uptime
      并写明理由（改系统时间会误判）。墙钟前跳会让 `remaining` 变负，
      循环再不 await 事件而是空转连发。改用 `SystemClock.uptimeMillis()`
- [x] **** `TerminalSurface.searchActive` 只被三处写、零处读，且其 KDoc
      描述的「触摸应抵达 Surface」由 `touchEnabled` 实际承担。删除死状态与三处写入
- [x] **** `render_inner` 在 `receive_cell_data()`（破坏性取数）**之后**才检查
      `surface.is_none()` 并返回；而 VT 的去重基线在**入队**时就推进，
      故换 Surface 空档期（releaseGpuSurface → attachSurface）取走的帧永不补发，
      输出一直缺失到下次内容变化。守卫前移到取数之前，与 `paused` 同处
- [x] **** `frameTiming` 记录的窗口从 `renderWithNewOutput()` 之前一直延伸到
      `pollAll()`＋事件派发＋标题查询之后，与 `loopTiming` 测同一个量，
      而注释声称它「只覆盖 bridge.render()」。渲染结束时刻改在 JNI 调用返回后立即取

## 22. ：否证（回读源码/实测确认不成立，记录依据以免重复排查）

- [ ] ~~Kitty `m=1` 直传 RGB 被 `pty_write` 的 `>0xF7` 清洗损坏~~：实测 `m=1` 无论载荷
      字节高低都返回 `None`（上游未实现该传输方式），`base64` 路径正常。清洗与该协议无关
- [ ] ~~图集重建无限递归~~：嵌套的 `rebuild_atlas` 携带的缓存严格递减，
      递归深度有界；`8x8` 图集实测正常返回 `None`。加标志位属无缺陷支撑的防御性代码，已回退
- [ ] ~~`RenderWatchDog.stop()` 与 `start()` 竞态~~：`start()` 只在
      `RenderWatchDog(...).also { it.start() }` 构造期调用，早于字段发布，
      不存在「拿到未 start 实例」的线程。改动无缺陷支撑，已回退
- [ ] ~~`openDocumentThumbnail` 经站外符号链接泄漏读句柄~~：`isHomeLink` 用
      `File(parentCanonical, name).canonicalPath` 解析**目标**，站外链接返回 false
      并落到 `decodeDocId` + `requireInsideRoot`。实测抛「outside the terminal home directory」
- [ ] ~~`copyDocument` 递归会让 CJK 之外的行为退化~~：实测无守卫时终态同样干净
      （`copyTree` 的错误路径会整体回滚），代价是耗尽路径长度与磁盘。回归测试因此断言
      **拒绝原因**而非终态

## 23. ：待办

- [x] **R32-T1** `openDocument_write_notifies_document_and_parent` 在整类并行跑时
      偶发判红（依赖写回线程 post 出 OnCloseListener 后显式 idle 全部 Looper）。
      12 次单独运行全绿、整类连跑偶现。需给写回通知加确定性等待而非依赖调度
      ——**本轮已修**：根因是单遍 `getAllLoopers()` 隐含了 idle 顺序假设。写回线程的
      `notifyChange` 是在它自己被 idle 之后才投进主 Looper 的；主 Looper 一旦排在
      写回线程之前被扫过，这条通知就没人跑第二趟。改为按轮次重取全部 Looper 再 idle，
      直到两个通知到齐（`idleLoopersUntilNotified`）：投递是同步的，故无需真实时间等待，
      上限只防「通知根本不发」时空转，真不到仍由原断言大声失败
- [x] **R32-T2** `copyDocument` 的子孙守卫比对的是 canonical 路径，
      故把符号链接复制进其**解析目标**下的目录会被拒——而 `copyTreeInto` 对链接不递归，
      该拒绝并无对应风险。守卫应加在「是目录且非链接」上
      ——**本轮已修**：守卫改判「`isDirectory` 且非符号链接」。`canonicalFile` 正是把
      链接解析到目标，故原判据会误伤「把链接拷进其目标子目录」这一正常操作；
      回归用例 `copyDocument_symlink_into_its_own_target_subtree_is_allowed`
      （断言复制产物仍是链接且指向不变，而非展开成目录树）

## 24. ：CI 复现与本轮新增

### 24.1 CI run 的复现结论

- [x] **N33-1** 该 run（head `f4d4af8a`）红在 `:app:connectedDebugAndroidTest`，
      14 个用例失败；`build` 工作流**有记录以来 10 次全部失败**，不是回归。
      失败签名高度一致：`SelectionDragQuantifiedTest` 唯一不需要 shell 的
      `whitespace_longpress_shows_paste_only_menu` 通过，其余「等 shell 回显/等 prompt」
      的用例全灭。本机复现：把 AVD 网络修好后重跑这 14 个用例所在的 10 个类，
      **本地全绿**。故产品侧无回归，红的是 CI 模拟器上没有可用的 shell。
- [x] **N33-2** 仪器化套件的 shell 硬依赖引导包下载：`waitForSession()` →
      `TerminalPrefix.ensureInstalled()` → `BootstrapOrchestrator` 拉 GitHub 上的
      termux bootstrap（`TerminalPrefix.kt`）。本机无网时全部此类用例报
      `terminal prefix install failed: Download failed: UnknownHostException`，
      即依赖链本身；CI 上无此报错（下载成功）却仍无 shell 输出，
      两种表现都指向同一处：**shell 在该环境下没就绪**。
      CI 侧根因未复现（需在 2 核 + swiftshader 的 runner 上复跑），如实记录不猜测。
- [x] **N33-3** 修好本机 AVD 的网络：`scripts/setup-emulator.nu` 启动参数加
      `-feature -Wifi`（宿主缺 `mac80211_hwsim` 时 guest 的 `wlan0` 停在
      `NO-CARRIER`，slirp 的 DHCP 拿不到地址）与 `-dns-server 8.8.8.8`
      （宿主 `resolv.conf` 指向 `127.0.0.53`，slirp 访问不到该桩）。
      修前本机 `ShellResponseLatencyTest` 两例均报引导包下载失败，
      修后通过。这是本轮一切本地取证的**前置条件**。

### 24.2 本轮新增并已修

- [x] **** `pasteFromClipboard` 对空剪贴板 `return 0`、对读取失败同样
      `return 0`，两处都不出声：用户点「粘贴」后什么都没发生，日志里也没有。
      空剪贴板补 warning（读取失败已由 `ClipboardAccess` 记日志，不重复）
- [x] **** `TerminalRuntime.writeToPty` 的 `entry.bridge?.writeToPty(data) ?: false`
      在 bridge 缺失时返回 false，而调用方 `pasteSink`/`InputBatchBuffer` 的
      `flushSink` 签名是 `(Long, ByteArray) -> Unit`，返回值被丢弃——
      击键/粘贴就此消失且毫无症状。补 error 日志
- [x] **** `BootstrapInstaller.normalizePath` 是手写的路径消解栈（22 行）。
      改用平台 API：`Path.normalize()` 消解点段、`Path.startsWith("..")` 按**路径元素**
      判逃逸（字符串前缀会把 `..foo` 误判成 `..`）、空路径单例判「消解回 staging 根」。
      净减 20 行，同时消掉一整类前缀判定错误

### 24.3 ：新增待办

- [ ] **N33-5（实测新证据，指向 R16-T3）** 整类 `connectedDebugAndroidTest`
      全量跑（189 例）本地 25 例判红；同一批类**单跑全绿**（10 类 0 失败）。
      失败按执行序集中在第 38～62 例，其后 `BootstrapCompatibilityTest`
      （连跑 18 条真实 shell 命令）反而全过——不是单调劣化。
      取 `MultiTapSelectionInstrumentedTest#tripleTapSelectsLine` 的 logcat 定位：
      `GoogleInputMethodService.onStartInput(com.termux)` **已经触发**，
      但同一时刻 `session 4 loop timing window: avg=137ms p95=500ms max=582ms ≈7fps`
      （空闲终端，`frame timing avg=0ms`，即渲染本身不耗时，耗时在循环里），
      随后 `pauseRendering` 停线程，测试在 20s 处报「IME 必须弹起」。
      即**渲染循环被阻塞到个位数帧率**，测试的 `runOnMainSync` 轮询排在饱和的主线程后面。
      阻塞源与 R16-T3 同形：`feedPty` 持 `session` 锁调 `Pty::write_all`，
      子进程不读 stdin 时最多等 `WRITE_DRAIN_TIMEOUT`（5s），
      而同一把锁每帧被 `render_inner` 与 `poll_event` 取得。
      根治仍需把 PTY 主端 fd 移出会话（结构性改动），本轮未擅自动手；
      先把实测证据登记在此，避免下次重新排查。

### 24.4 ：外部依赖替代扫描

- [x] **** 逐处核对本仓手写实现：路径消解（已换 `java.nio.file`）、JSON/序列化
      （kotlinx.serialization）、base64（平台 `Base64`）、防抖（协程）、LRU（外部缓存）、
      字体（fontdb/cosmic-text）、VT（上游 ghostty 绑定）均已走外部库。
      本轮新增的候选只有路径消解一项，已落地。其余手写代码（宽字符列↔字符下标映射、
      粘贴分块、网格对齐）都是产品语义本身，无可替代的外部 API，
      换库只会把语义藏进依赖。结论与 一致，无新增可替项。

## 24. （code-review-skill 复审）：已修与已推翻的既往结论

- [x] ****（同时推翻 的修法）**的前提是错的**：它认定
      `scrollbackLine` 返回的是「宽字符只占一个字符下标」的紧凑文本，故引入
      `charIndexAtCellColumn` 按宽度累加反推列↔字符映射。但原生
      `read_line_text_impl` 的契约相反——每列恰好一个字符，宽字符尾格是空格占位
      （实测 `vt_write("中文AB")` → `"中 文 AB"`，6 字符 / 6 列）。累加模型在每个
      宽字符之后整体错位一格：长按宽字符恒弹仅粘贴菜单（菜单分类与选词模型分家），
      且行内第二个宽字符的尾格不再吸附、选区把字切开——正是 想修的症状本身。
      既有单测用紧凑文本（`"中文AB"` / `"中a"`）构造用例，与实现互相背书，缺陷不可见。
      现改为经 `wideCharTailCols`（原生 `grid_ref().cell().wide()` 标出该行全部尾格列）取网格事实，
      删除 `charIndexAtCellColumn`、`snapColToWideChar` 与整份手写 wcwidth
      （`util/TextWidth.kt`：它还把组合记号、ZWJ、变体选择符一律按一格计）。
      详见归档变更 `2026-10-05-fix-wide-char-cell-mapping`
