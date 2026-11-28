# 全量代码审查记录

## 范围与依据

- 审查对象：全仓现行代码，不以某次 diff 或提交为界。`docs/REVIEW_CHECKLIST.md` 由 `git ls-files` 枚举 625 个受控文件并逐行记录行数、风险命中与核验结论；其中 `native/src` 下 147 个 Rust 文件（53,754 行）与 `android/app/src/main` 下 115 个 Kotlin 文件（30,349 行）逐个过目。
- 外部规则：`awesome-skills/code-review-skill` 的 `SKILL.md`、`review-checklist.md`、`code-quality-universal.md`、`common-bugs-checklist.md`、`architecture-review-guide.md`、`performance-review-guide.md`、`security-review-guide.md`、`kotlin.md`、`rust.md`；`android/skills` 的 `android-jni-security`、`jetpack-compose-performance`、`anr-audit-and-fix`、`r8-analyzer`、`edge-to-edge`、`testing-setup`、`agp-9-upgrade`。
- 本地规则：`docs/specification/` 全部文档，以及 `rust-review`、`rust-best-practices`、`code-review-and-quality`、`compose-expert`、`compose-screens-footguns`、`state-and-background-footguns`、`data-layer-footguns`、`security-and-hardening`。
- 重点模块：Rust FFI/JNI 边界、会话与 PTY、渲染/Kitty 图集、字体与日志；Kotlin 启动与 ViewModel、渲染监督、Compose UI、设置/DataStore、DocumentsProvider、引导下载与解压、构建与 R8。

## 发现与处置

| 模块 | 问题 | 处置 |
| --- | --- | --- |
| 引导安装 | 解压条目循环与单个大条目写入循环不检查取消，用户取消后仍继续写满 staging | 两处循环均检查 `shouldAbort()` 并抛 `CancellationException`；规范同步 |
| DocumentsProvider | 根路径包含判断用字符串前缀，`..foo` 等同级目录可能被误判为在根内 | 统一按路径元素比较的 `isInsideRoot` |
| DocumentsProvider | 回滚产生的占位文件出现在搜索结果 | 搜索过滤回滚占位 |
| 渲染监督 | Effect 取消时 render-pause token 丢失，可能永久暂停渲染 | token 跨 Effect 取消保留 |
| 渲染 | GPU readback map 超时过短，并行构建/测试下偶发失败 | 放宽超时 |
| Kitty 图集 | 图集仅按软件预算校验，可能超过设备 `max_texture_dimension_2d` | 打包、借用、复用三条路径统一按设备上限与预算双重拒绝；规范同步 |
| R8 | `-keep class terminal.emulator.** { *; }` 使 JNI 入口与全部 Kotlin 类免于 shrink | 收窄为仅保留必要的 `keepclassmembers`；R8 Analyzer 分数由 87.50% 提升至 98.70%；说明不可达 native 方法为何可安全移除 |
| JNI | `listFontFamilies` 与批量会话查询在循环中创建局部引用却不释放，长列表可能耗尽局部引用表 | 立即释放临时 class/string 局部引用 |
| 终端抽取 | 逐格分配与缺行补齐的成本未留档 | 补齐取舍台账并用测试钉住分配预算 |
| KGP | 单图整幅的零拷贝借用分支被套上字节预算判据，与规范「单图整幅照常显示」冲突：字节数超 64MiB 的单图被整幅拒掉 | 字节预算只约束需新分配的条带布局；零拷贝借用不分配新缓冲，其像素缓冲已由上游图像存储上限约束，故不受预算约束。打包与复用两条路径共用唯一判据 `check_atlas`（设备纹理上限 + 字节预算），分叉不复存在；规范同步 |
| 引导安装 | 成功路径在原子换入后才创建 `home`/`usr/tmp` 且忽略创建失败，可能留下「prefix 已发布但安装返回失败」的半成功状态 | 换入前在 staging 内准备 `tmp`，显式校验两个目录的创建结果；补回归用例钉住失败路径不发布 prefix |
| 引导下载 | 下载无体积上限：声明长度虚高或分块响应可持续写满缓存目录，而解压已有 1 GiB 上限 | 声明长度与落盘字节双向按 1 GiB 封顶，超限即删暂存并返回失败；规范同步，补拒收用例 |
| 日志 | 两处慢帧诊断各写一份 60ms 警告阈值，调整时易漏改一处 | 共用 `slow_frame_log_level` 纯函数与单一阈值常量，补分档单测 |
| JNI | `setExtraFontPaths` 的路径数组元素每次循环新建局部引用且不释放，入口参数长度不受限，足够长的数组会耗尽局部引用表（上限 512） | 读毕即删局部引用（解码失败的提前返回分支同样释放） |
| 输入 | 触摸拖动的「已转发滚轮」标记被本次编码结果覆盖：前一档已转发、后一档因 VT 忙而拿不到编码器时标记回落，本地回滚在手势中途接管，应用与本地同时滚动 | 标记在手势内只升不降；判定抽为纯函数 `wheelForwardedState` 并补 JVM 回归测试；规范同步 |
| 终端网格 | `DISCONNECTED_COLS`/`DISCONNECTED_ROWS` 是 u32 常量，却先 `as u16` 收窄再升回 u32，窄化本身有损 | 直接按上游 `cols()`/`rows()` 的 u16 取值映射到 u32，回退值用常量的原生类型 |
| JNI | 网格尺寸、VT 忙时长、PTY 投递忙时长三处各抄了一份「查会话 → 读一个值 → 会话未知给默认」的样板 | 收敛为 `session_scalar`，会话未知的回落口径只剩一处；补回归钉固定「未知会话 → 默认值」 |
| PTY | spawn 的 linker 路径、argv 收尾与含 NUL 的文本转换各写多份，含 NUL 的报错口径还分叉（有的带出错位置，有的只留一句 contains null byte） | 收敛为 `linker_path`、`push_shell_arguments` 与 `cstring_without_null`，报错统一带上出错位置；补三个回归钉 |
| 终端会话 | 剪贴板读取请求与 VT 应答的队列溢出裁剪逻辑逐字相同 | 收敛为一个泛型实现；两个上限常量与窗口计数口径不变 |
| 选区手柄 | 快速拖动路径与常规拖动路径各写一份「更新被拖端点状态 + 重定位两侧手柄」 | 收敛为 `applyDragResult`，端点语义不再可能分叉 |
| JNI | 等待输出与唤醒渲染两处各抄了一份「取注册表读锁 → 查会话 → 克隆唤醒器 → 放锁」 | 收敛为 `session_render_wake`，读锁持有范围与会话已销毁的处置口径只剩一处 |
| 字体图集 | 驱逐重建与字号/字体变化后的重置各写一份「重建 etagere 分配器 + 位图清零」 | 收敛为 `reset_atlas_storage`；只重建分配器而忘了清位图会让新字形与旧像素叠加 |
| 终端帧 | 帧网格在行缓存、帧邮箱与渲染侧各存一份整网格拷贝（24×80 约 184KB），每帧多两次「分配 + 拷贝」 | 帧网格改为单一所有权（`Arc` 共享），三者共用一份；分配预算护栏收紧到每帧 2 份整帧缓冲，帧内存身份比对作回归钉；取舍台账第⑨项 |
| 前台服务 | 唤醒锁默认自动持有，并设 30 分钟超时 + 半程续期定时器，与 DESIGN「不自动申请、不自动持锁、锁由通知栏按钮切换、无超时机制」冲突 | 默认改为不持锁，仅由通知栏按钮申请/释放；移除超时与续期定时器，锁只由用户释放或会话归零释放；补「默认不取锁」「按钮双向切换」「长时间不失锁」三条用例；规范同步 |
| 前台服务 | `specialUse` 类型未声明 `PROPERTY_SPECIAL_USE_FGS_SUBTYPE`，系统与 Play 审核无从识别该服务的用途 | 在 `<service>` 内声明终端会话保活的用途说明，与既有 `FOREGROUND_SERVICE_SPECIAL_USE` 权限配套 |
| 启动入口 | `EXTRA_INSTALL_BOOTSTRAP_URL` 在 release 构建中同样可达，任何应用都能借官方地址触发一次约 150 MB 的下载与安装 | 与另一测试 extra 统一收进 `BuildConfig.DEBUG`：两个使用方（`NixBootstrapTest`、`TermuxBootstrapRealTerminalTest`）都跑 debug 变种 |
| 现行规范 | Gradle wrapper 已升到 9.9.3，而 `build-and-validation` 与 `code-health` 两处规范仍写 9.9.2，「唯一弃用警告只有一条」那条判据无法按记录的版本复现 | 两处规范同步到 9.9.3，并实测复核：9.9.3 + AGP 9.6.0-alpha06 下 `:app:compileDebugKotlin --warning-mode all` 的弃用警告仍恰好一条（`Configuration.setVisible(boolean)`） |
| PTY | shell 路径与解释器路径各写一份 prefix 判据，分隔符 `/` 容易漏写；同一处还留着恒真的 `contains('\0')`（含 NUL 的路径已由 `cstring_without_null` 在 fork 前以 `?` 拒绝） | 判据收敛为 `under_prefix` 共享实现，按路径元素比较（`<prefix>_evil/…` 不再算 prefix 下）；删除恒真分支；补边界用例 |
| PTY | shell 与脚本解释器两条 linker 路径各写一份逐字相同的 `CString::new(linker_path())` 及其错误映射 | 收敛为 `linker_cstring`；该文件在严格重复率口径下归零 |
| 静态检查 | 方案调研文档照录被禁 API 的事实，触发 `docs/` 下的字面量禁令规则误报，质量门红灯 | 按既有豁免口径按文件路径豁免（与 `docs/persistence/*`、`docs/specification/*` 同类）；探针验证代码侧仍被拦下 |

重复度扫描（`jscpd`，阈值 8 行/80 token）：Rust 0 处克隆；Kotlin 1 处 11 行重复，且为两个设置分区的相同 import 块，不构成逻辑重复。另以更严的阈值（5 行/40 token）复扫一遍，全量命中 128 处 / 926 行 / 1.09%；逐类判定后，生产代码内的命中归为四类且均不构成逻辑重复：Kotlin import 段与 Compose 脚手架（18 处）、Android 资源文件（3 处）、Rust 函数签名形状（JNI 导出与转发函数的参数表，20 处）、文档注释（6 处）。唯一两处可收敛的真重复已处理：`pty.rs` 的 prefix 判据与 linker CString 构造，收敛后该文件在严格口径下归零。判定明细见 `openspec/changes/archive/2026-11-24-code-health-audit/audit.md` 的严格口径一节。

## 验证

- `./scripts/check.nu` 全量质量门通过（fmt、clippy、arm64 ndk clippy、cargo-machete、cargo test、rustdoc `-D warnings`、bench、release 构建、semgrep 55 条规则 0 命中、markdownlint 0 issue、openspec strict 55/55、Gradle 全套 `BUILD SUCCESSFUL`）。
- Rust：`cargo test --all-targets` 为 991 + 1 + 2 + 1 个测试全部通过；`cargo clippy --all-targets -- -D warnings` 零告警。
- Android：`testDebugUnitTest` 为 1174 个单元测试全部通过；`detekt`、`spotlessCheck`、`app:dokkaGenerate`、`lintDebug`、`lintVitalRelease`、`assembleDebugAndroidTest`、benchmark 与 baselineprofile 编译全部 `BUILD SUCCESSFUL`。
- JNI 边界：Rust 侧 78 个 `Java_*` 导出与 Kotlin `NativeBridge.kt` 78 个 `external fun` 双向差集为空（名字、参数个数与类型均核对）。
- 反汇编 release `classes.dex`：69 个可达 JNI 方法全部保留；9 个仅诊断/测试用且 Kotlin 侧无可达调用点的方法随死代码移除，原生侧不存在通过 JNI 反射回调 Kotlin 的路径。
- 全部修复均已提交并推送到 `origin/main`。

## 审查结论（rust-review 口径）

审查对象为 `native` crate 全部 147 个 Rust 文件与 `android/app/src/main` 全部 115 个 Kotlin 文件（53,755 + 30,343 行）。`native` 是应用型 crate（cdylib + lib，无 feature 开关、MSRV 1.99、edition 2024），故按「简洁性优先」的适用规则审，不套用库的严格 API 契约。clippy、测试与全量质量门均已实跑。

### Must fix

无。全部已发现缺陷均已修复并落库，本轮新发现的三项（规范版本漂移、PTY prefix 判据与恒真分支、方案调研文档误触静态规则）已分别提交并推送。

### Should fix

无未决项。

### Consider

- `android/app/src/main/java/terminal/emulator/ui/BackgroundRenderPause.kt:42` 与 `SurfaceAttachGate.kt:37` 是两个刻意同构的单槽暂停凭证持有者（各 9 行重复）。两者线程契约不同（前者 `@Volatile` 供 IO 线程读，后者限主线程），释放与清凭证的顺序也因此相反；合并需要引入模式参数，复杂度反超重复本身，故保留现状。
- `native/src/terminal/ghostty_terminal/public_api/encoding.rs` 的公开版与「通道外置版」成对出现（形似重复）。这是为让每次鼠标移动不持会话锁而刻意分出的两条入口，签名形状相同属必然。

### What's already good

- JNI 边界干净：Rust 导出与 Kotlin 声明双向一一对应，无孤儿方法；局部引用在长列表路径上逐个释放。
- 帧内存路径做到单一所有权：`Arc<Vec<CellData>>` 共享给行缓存、帧邮箱与渲染侧，每帧省两次整网格拷贝，并有分配预算护栏与帧内存身份比对作回归钉。
- 渲染热路径的代际稳定性处理扎实：图集代际推进时降级为全量重建并作废缓存写回，避免用已搬迁的 UV 采样。
- 失败路径不留幻觉：会话未知、PTY 未排空、execve 失败等都有明确的错误类型与日志，不靠吞异常伪装成功。

## 已知残余风险

- `qlty check --all` 仍有 36 条：31 条 `zizmor` 命中受保护的 `.github/workflows/`（action 未固定到 hash、`persist-credentials` 未关闭、自仓引用语法、多余 action），须用户明确授权后才能改；1 条 OSV 提示 `ttf-parser@0.25.1` 已停止维护，它由 `cosmic-text 0.19` 经 `fontdb 0.23` 间接引入，`Cargo.toml` 受保护且上游版本约束未放开，本地无法单独替换；余下 4 条是 `ripgrep` 对源码注释中 `TODO` / `NOTE` / `XXX` 的提示，不是运行期缺陷。
- 未连接真机执行 GPU/JNI 压力测试与 Android instrumentation 测试；设备特定纹理上限已按运行时 `limits()` 兜底，但真机路径仍需在有设备环境复验。
- 同一仓库存在并行提交者：其提交会把工作区暂存的内容一并卷入（本次「滚轮转发标记只升不降」的修复即随另一笔 `refactor(pty)` 提交落库，随后该提交被改写）。提交前须逐文件核对暂存区，勿把他人的半成品计入自己的提交；跨提交的归属以文件内容与本记录为准，不以提交标题为准。
