# 任务：审计遗留项台账

编号沿用审计文档原编号，便于对照历史记录。状态口径见 `design.md` 第 2 节。

## 1. 高危：用户可观察的错误状态

- [ ] N3「全选→复制」经 VT 线程同步做约 110 万次 ghostty FFI，
      500ms 超时后回落空网格 —— 静默告诉用户「终端是空的」
- [ ] N1-7渲染暂停（打开设置页／输入法弹出）时 OSC 52 剪贴板写入
      被丢弃；已改记日志但仍丢弃，未队列化延后
- [x] N1-9OSC 52 读取失败时回「成功但为空」—— 本轮已修：
      读取失败经 `clipboardResult(..., null)` 上报，原生不写回 OSC 52 应答
      （空串等于告诉远端「用户清空了剪贴板」）
- [x] N1-10选区查询失败被合并为「无选区」—— 本轮已修：
      空选区 `""` 与查询失败 `null` 分开，失败记 error 不再静默
- [ ] N2-26`CursorInfo` 不带 rows/cols，`CSI ?3h`(DECCOLM) 后
      `CellData.col` 可达 159 而渲染器仍按 80 列排布
- [ ] N2-50`writeToPty` 在**执行时**才解析 `sessions[activeSessionId]`，
      跨会话切换时粘贴尾部写进新会话（同库 OSC 52 路径却在请求时捕获 `Arc<Session>`）
- [ ] N2-45`Query::EncodeMouseEvent` 无 modifier 字段，
      Shift/Ctrl 点击到达 vim/tmux/htop 与普通左键不可区分（违反 DESIGN:182）
- [ ] N2-44DESIGN:152 要求的修饰键栏左右移动可见区域完全未实现

## 2. 中危：资源与契约

- [ ] N9 / N2-23`themes.xml` 硬编码 `#1E1E2E` 且无 `values-night/`，
      日间主题下系统窗口恒为夜间配色（违反 DESIGN:106/108/200）
- [ ] N21`ModifierBar` 的 DRAWER 长按落到 `else -> null`，长按粘贴从未接线
- [ ] N2-1`FontUtils` 把 `mono/monospaced/sans` 硬编码改写，
      真名为 "Sans" 的字族被静默换成另一个（违反 DESIGN:101/102）
- [ ] N2-2`~/.termux/fonts` 只在会话创建时扫一次，运行中拷入字体须重启进程
- [ ] N2-2459 个 `external fun` 只对 43 个加 `@JvmStatic`，
      CheckJNI 报错、`RegisterNatives` 路径断裂
- [ ] N2-37`kgp_atlas_data` 是 KGP 图集的永久 CPU 全量副本且从不读取，
      64MiB 预算下等于内存翻倍
- [ ] N2-42生产代码读 `System.getProperty("test.minSurface"/"test.bootstrapUrl")`
- [ ] N2-68`Session::drop` 可阻塞 JNI 调用方约 1.1s
- [ ] N2-6 / N2-40fork 子进程 `setsid`/`TIOCSCTTY` 失败裸 `_exit(2/3)`，
      不写 fd 2，用户只见 `[Process completed (code 3)]`（违反 DESIGN:16/194）
- [ ] N2-8 / N2-11`focus_event` 持 session 锁做 50ms RPC；
      `Event::Clipboard` 载荷无上限
- [ ] N2-9 / N2-10`ClipboardRead` 被满队列淘汰，子应用收到**空剪贴板**
      而非自己的答案，且该 RPC 永不重发
- [ ] N2-12`MAX_SCAN_BYTES` 超限静默丢弃 OSC 52 请求
- [ ] 吞错批次 N2-1～N2-4、N2-5～N2-7、N2-10～N2-14：字体 JNI 失败仍返回字族名、
      高亮包格式错误使上一帧高亮永留屏、`ensure_frame_texture` 错误被丢弃、
      搜索 JSON 解码失败冒充 0 匹配、`InputBatchBuffer` 空 catch 丢击键、
      `chdir` 失败继续在 `/` 下运行

## 3. 低危：死代码与死资源（STYLE:63）

- [ ] N1 / N2-32 / N2-33 / N2-34（REVIEW.md、）快照链、键盘编码链、
      `send_signal` / `read_visible_text` 生产零调用
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

- [x] 目录 `FLAG_SUPPORTS_DELETE` 与实现不符 —— `deleteWithoutFollowingSymlinks`
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

## 7. 文档退役

- [x] 7.1 本台账成文（含真实缺陷、裁决项、授权项、否证项四类）
- [x] 7.2 删除 `docs/REVIEW*.md` 全部 12 个文件
- [x] 7.3 确认无残留引用（`.semgrep/*.yml` 的排除项指向空集，无副作用）
