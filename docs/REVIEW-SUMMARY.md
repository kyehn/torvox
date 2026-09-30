# 审查结论索引（第 6–9 轮）

最后更新：2026-09-30
基线提交：`b7c9b96`
详细记录：[REVIEW-ROUND6.md](REVIEW-ROUND6.md)、[REVIEW-ROUND7.md](REVIEW-ROUND7.md)、[REVIEW-ROUND8.md](REVIEW-ROUND8.md)、[REVIEW-ROUND9.md](REVIEW-ROUND9.md)（第 1–5 轮见 [REVIEW.md](REVIEW.md)）

---

## 一、基线状态

| 检查 | 结果 |
| --- | --- |
| `scripts/check-rust.nu` | **exit 0** — fmt / clippy / machete / semgrep（32 规则 0 发现）/ test（504 全过）/ rustdoc / markdownlint（111 文件 0 问题）/ bench |
| `scripts/check-gradle.nu` | **exit 0** — semgrep（16 规则 0 发现）/ detekt / spotlessCheck / lintDebug / lintVitalRelease / assembleDebugAndroidTest / testDebugUnitTest |
| `aislop 0.16.1` | 12 warnings（8 文件超 1000 行、2 函数超 120 行、2 叙述式注释块） |
| `jscpd 5.3.3`（10 行 / 50 词） | 113 clones / 3.01%（第 6 轮前为 108 / 3.02%） |
| JNI 契约 | 57 ↔ 57 完全对称，零缺失/零孤儿/零签名不符 |
| `jni_export_guard!` | panic 无法跨 `extern "system"` 边界（jni 0.22.4 `catch_unwind`） |
| JNI 全局引用 | `NewGlobalRef` 出现 **0** 次，零泄漏 |
| 字节数组线格式 | 全部经长度校验，不可能让 Kotlin 解析器失步 |

**五个自动化工具全绿，同时存在 9 个 P0。** 这是第 6 轮的核心结论，也是本轮次全部工作的出发点。

### jscpd 增量的正确读法

113 比 108 多出的 5 个克隆**全部来自本文档族本身**。jscpd 会把 Markdown 里的围栏代码块按其语言计入统计，因此 `docs/REVIEW-ROUND6.md`、`ROUND8.md`、`ROUND9.md` 中的 ```rust 代码块被当作 rust 源文件分析（报告中出现 `docs/REVIEW-ROUND6.md:rust` 这类条目）。

按实际代码文件核对，受影响的 rust 源文件集合与第 6 轮之前**完全一致**：`ffi.rs`、`event.rs`、`cell_builder.rs`、`font/mod.rs`、`font/rasterization.rs`、`render/tests.rs`、`ghostty_terminal/{mod,tests}.rs`、`test_helpers.rs` —— **没有引入任何新的代码重复**。总重复率也从 3.02% 微降到 3.01%（分母同步增大）。

这暴露了 `jscpd` 配置的一个已知缺口：它会解析 `docs/**` 里的代码块，而 `docs/` 恰恰是最不该计入代码统计的目录。与 `REVIEW.md` 第五节第 5 条（建议把 `openspec/changes/archive/**` 加入 ignore）同源，建议一并把 `docs/**` 排除。

---

## 二、第 1–5 轮「已收敛」判定作废

`REVIEW.md:502` 曾记载「第 5 轮无任何新问题，连续四轮未发现新的 P0/P1 逻辑缺陷。审查收敛」。

第 6–9 轮用四种不同维度复查，**每一轮都产出新的 P0**：

| 轮次 | 审读维度 | 新增 P0 | 新增 P1 | 计数 |
| --- | --- | --- | --- | --- |
| 6 | JNI 编排层 + ghostty 适配层 + Kotlin runtime/surface | 4 | 11 | 归零 |
| 7 | 上游进程/表面生命周期 + 设置/主题/安装/文档提供器 | 2 | 5 | 归零 |
| 8 | 类别横切：吞错点 / 锁与阻塞 | 2 | 7 | 归零 |
| 9 | FFI 契约 + Android 组件与资源生命周期 | 1 | 6 | 归零 |

第 1–5 轮的样本覆盖了 JNI 层与 UI 骨架，但从未逐行读过：VT 线程析构路径、回滚区搜索算法、`BootstrapOrchestrator` 的返回值消费方、PTY 写入循环、渲染器表面生命周期、设置/安装/文档提供器、CI 工作目录的实际内容。

**原 P0/P1 清单逐条复核后全部仍然成立，无一被修复。**

---

## 三、9 个 P0（按建议修复顺序）

| # | 位置 | 一句话 | 建议改动量 |
| --- | --- | --- | --- |
| 1 | `ghostty_terminal/mod.rs:41-53` | `Drop` 里无界 `handle.join()`；同文件 `session.rs:661-681` 已有 `join_with_timeout` 却没用。注释写「不得阻塞析构」，代码恰好相反 | 1 处 |
| 2 | `installer/SecondStageRunner.kt:90` + `BootstrapOrchestrator.kt:95-103` + `TerminalViewModel.kt:1132` | 二段安装失败（`Result(false, …)`）被三处忽略，一律上报 `INSTALLED` | 3 处 |
| 3 | `android/ffi.rs:1601-1611`（+`:2745-2761`、`:1876-1883`） | `take_kitty_placements` 在持 `RENDER_STATE` 时再取注册表 + 会话锁，与 `setTheme`/`renderWithNewOutput` 构成 ABBA **硬死锁** | 1 处（同时关闭三个缺陷） |
| 4 | `android/ffi.rs:1122-1165` → `session.rs:507/518` → `public_api.rs:201` | `pollEvent` 持注册表读锁 + 会话锁调 `flush()`，`recv_timeout(5s)` × (1 + 后台会话数)；同时 `output_rx` 无独立泵，渲染失败即冻结全应用 shell | 1 处（与第 5 条同源） |
| 5 | `terminal/session.rs:271` + `TerminalRuntime.kt:1274-1300` | 输出通道是**阻塞**发送、唯一消费者只在 `render() >= 0` 分支被调用 | 1 处 |
| 6 | `ghostty_terminal/internal.rs:2167-2202` | 回滚区搜索 O(n²)：向前回溯拼接软换行（每行 O(cols) 次 FFI 往返）+ `insert_str(0, …)` 前插；同时造成列号越过网格宽度 | 1 处（同时修掉 P1 列号错位） |
| 7 | `DocumentQueries.kt:36-49` | `rootDir.canonicalPath`（解符号链接）对比 `Path.normalize()`（纯词法）—— Android 上 `/data/data` → `/data/user/0`，**每个符号链接条目都抛异常** | 1 处 |
| 8 | `installer/BootstrapInstaller.kt:119-126, 160-167` | `EXECUTABLES.txt` 的行未经 `..` 校验就 `Os.chmod`；同文件对 zip 条目名与 `SYMLINKS.txt` **都有**校验 | 1 处 |
| 9 | `android/ffi.rs:114-121` + `:1727-1733` | `last_frame` 缓存帧不带会话 id → 切会话后可能把 A 的网格画到 B | 1 处 |

累计：**9 个 P0、28 个 P1、约 55 个 P2/P3、30+ 处死代码**。全部细节见各轮文档。

---

## 四、阻塞性决策项

以下不是「修哪个」的问题，而是**规范之间互相矛盾**，需要先定口径：

- **D1** `DESIGN.md:24/142`（不做未声明的 Fallback / 出现问题正常报错就是）与 `pty.rs:651-651` 注释自述的「错误静默忽略（不致命）」、`TerminalForegroundService.kt:121-136`（`startForeground` 失败后继续）直接冲突。**保留 Fallback 还是删掉？**
- **D2** `STYLE.md:61`（`fish`/`dash`/`zsh` 不得出现在任何文件）vs `.semgrep/rust-deny-patterns.yml:105-109`（规则只扫 `[rust, kotlin]`，且恰好漏掉 `bash`/`sh`）。`fmt.yml:44` 就在用 `bash -c`。**禁止范围要扩展到 `.yml`/`.nix`/`.md` 吗？**
- **D3** `PROHIBITED.md:10`（会话数据持久化/恢复禁止）vs 五个监控类（`AnrWatchDog`/`BootGuard`/`MemoryMonitor`/`ThermalMonitor`/`TerminalForegroundService`，约 670 行）。`AnrWatchDog` 5 秒主线程卡顿即 `Process.killProcess`，**每次触发都销毁全部 shell 且不可恢复**。**删除整块，还是补规范声明后保留？**
- **D4** `BUILD.md:7`「`ANDROID_NDK_HOME` 已预设」是事实错误 —— `flake.nix` 从未声明 NDK，本机实测是 r27d 而 `BUILD.md:20` 要求 r30。**改 `flake.nix` 还是在 `BUILD.md` 删掉这条？**
- **D5** `public_api.rs:149-150`（`pty_write` 文档：「二进制 VT 数据应改用 `vt_write`」）vs 生产路径只有 `session.rs:497` 调 `pty_write`。**删除改写（依赖内核 `ONLCR`）还是接受当前行为？**
- **D6** `PROHIBITED.md:19`（禁止内嵌 bootstrap/预装发行版）字面 vs `DESIGN.md:126-142`（完整的下载式安装声明）。现有实现是下载式，两份文档字面矛盾。**改哪一份？**
- **D7** 发布链路（`build-apk.nu:6-13` 删两个变体 + `build.yml:2-5` 无 `push: tags`）使打 tag 产出空 release。**先修发布还是先修运行时？**
- **D8** `docs/specification/BUILD.md:15-17` 要求的 `.so` 三项校验（`NEEDED` / 体积 / APK 含 `.so`）在 `scripts/build-android-libs.nu` 里全缺。**（保护文件，需授权）**
- **D9** `REVIEW.md` 第七节 D1–D9 原有的 9 条仍未决，本轮新增第 10、11 条（`TESTING.md:9/12` 与「静默测不出东西」的冲突）。

---

## 五、保护文件的改动请求

按 `AGENTS.md`，以下文件不可修改，全部只报告，需明确授权：

`.github/workflows/{build,check,fmt}.yml`、`scripts/*.nu`、`flake.nix`、`rust-toolchain.toml`、`Cargo.toml`、`android/**/build.gradle.kts`、`settings.gradle.kts`、`detekt.yml`、`lint.xml`、`.semgrep/*`、`.gitignore`、`docs/specification/*`。

各轮文档的「修复顺序」小节末尾都附有该轮的完整改动请求清单（第六节）。

---

## 六、下一步

**建议停止审查、改为修复。** 理由：

1. 缺陷密度高到任何抽样都会漏 P0 —— 四轮、四种维度、四次验证，每次都产出新的 P0。
2. 上表 9 个 P0 中至少 7 个是「几行改动消除一个永久错误状态」，边际收益远高于继续读码。
3. 「连续四次无新问题」这个验收标准，**只有建立在已修完的代码上才有意义**；在未修的代码上，它衡量的是抽样运气，不是代码质量。

修复完成后再重跑四轮复审 —— 那时若确实连续四轮无新问题，才构成有效的收敛证据。
