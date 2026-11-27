# plyra-terminal 深度审查

- 来源：https://github.com/J-x-Z/plyra-terminal
- 审查快照：2026-11-27，默认分支工作树；不记录提交哈希。
- 覆盖方式：枚举全部受版本控制文件后逐文件完整读入并计算 SHA-256；核心实现按声明与关键片段逐行复核。二进制与许可证/锁文件/生成表仍完整读入并登记哈希，但不逐行复述内容。
- 本地核对：下列本地文件均已实际打开阅读，核对结论只落在既有结构上。

## 审查结论

仓库定位：跨桌面与 Android 的 GPU 终端，核心卖点是特权隔离与可验证构建，而不是单纯的终端功能。

### 分层

`docs/ARCHITECTURE.md` 给出的组件划分与源码一致：

- `terminal-core/`：共享屏幕、有界历史、选择、校验过的屏幕协议与 OSC 8 元数据。
- `session-manager/`：拥有 PTY、结构化可执行参数、输入队列、resize、鼠标与焦点协议。
- `terminal-render/`：wgpu/Metal 表现层，`glyphon` + `cosmic-text` 字形布局，裁剪窗格文本、选择和 IME preedit，WGSL 材质。
- `system-api/`：有界快照、日志与连接查询、文件系统事件、托管恢复、沙箱插件。
- `privilege-policy/` 与 `privilege-broker/`：能力与目标身份校验、独立认证 IPC 进程、一次性确认票据、执行时再验证、撤销与审计。
- `apps/desktop/`：winit 事件循环、标签与分割树、按需设置面板；`apps/android/`：Java 入口与 smoke 实现。

### 关键机制

- 终端数据来自 macOS helper：helper 持有真实 PTY 与提取出的 libvterm 0.3.3 解析器，通过有界 JSON 屏幕/滚动事件送到 Rust 读线程；读线程校验表示、更新共享屏幕并合并 wakeup。输出背压有界，但历史裁剪前的输出仍保留。
- 屏幕事件是“已校验协议”，不是原始字节直接进 UI；这使渲染层与 PTY 层解耦，也把不可信输出挡在模型边界外。
- 空闲时事件循环睡眠，只有终端输出、输入、布局变化或显式动画请求帧；`Composition cancellation` 与 late-commit 过滤阻止输入跨窗格焦点切换。
- OSC 8 ID 存在 libvterm cell 上，光标移动、交替缓冲区、滚动与 reflow 都保留关联；`PROVENANCE.json` 记录 vendor patch 与源码指纹。
- 特权操作由 broker 处理：能力绑定到具体目标，返回一次性票据，执行前重新校验；管理员授权走系统原生提示，撤销结束授权会话。

### 验证与风险

- 14 个测试文件、`crates/mcp-client/tests/protocol.rs`、`apps/android/tests/*.java`；大量详尽文档与 Gate 报告说明成熟度，但源码中 `TODO/FIXME` 较多，`unsafe` 计数很高（含依赖与生成代码），不能据此认为零风险。
- Android 仍被文档标为 smoke/lifecycle 验证，桌面 active target 是 Apple Silicon；把其 Android 路径当作参考实现会比桌面路径弱。
- 特权 broker、插件沙箱、MCP 与系统查询远超终端核心需求；引入任一模块都会扩大攻击面。

### 本地取舍

可借鉴“屏幕事件校验后再进渲染”和“空闲不请求帧”；不可采纳其特权代理、插件和 MCP 设计。本仓的渲染已走自有管线，不需要第二套屏幕协议。

## 本地代码核对记录

| 本地文件 | 核对点 |
| --- | --- |
| `native/src/terminal/output_processor.rs` | 对照 OSC/CSI 分派先于渲染的边界 |
| `native/src/render/cell_builder.rs` | 对照 `compute_dirty_bands` 与 `diff_dirty_rows_into` 的脏行合并 |
| `native/src/render/pipeline.rs` | 对照 `GpuUniforms` 的 std140 填充与共享绑定组 |
| `native/src/render/mod.rs` | 确认无 CPU/Canvas 回退、`GpuError` 可恢复分类 |

## 文件覆盖清单

状态含义：`TEXT` 为逐行复核范围内的文本文件；`BINARY` 为含 NUL 的二进制资源；`DECLARED_EXCLUSION` 为许可证、锁文件或生成表。三者都经过完整读入与哈希登记。

| 路径 | 状态 | 字节 | 行 | SHA-256（前 12 位） |
| --- | --- | ---: | ---: | --- |
| `.cargo/config.toml` | TEXT | 1248 | 5 | `957d7c26a9ba` |
| `.github/workflows/build.yml` | TEXT | 6077 | 141 | `2e1661265418` |
| `.gitignore` | TEXT | 69 | 8 | `d28c7e855b7a` |
| `Cargo.lock` | DECLARED_EXCLUSION | 129040 | 5240 | `1c5a96fa8f2c` |
| `Cargo.toml` | TEXT | 1915 | 30 | `ded1015142c6` |
| `LICENSE` | DECLARED_EXCLUSION | 34674 | 232 | `fb981668c18a` |
| `README.md` | TEXT | 4449 | 73 | `2cb7e1313cf1` |
| `apps/android/AndroidManifest.xml` | TEXT | 1702 | 35 | `de8c14cfc806` |
| `apps/android/Cargo.toml` | TEXT | 2428 | 46 | `f9aa75b1b73d` |
| `apps/android/README.md` | TEXT | 4097 | 44 | `ae2f6eff3303` |
| `apps/android/build_apk.py` | TEXT | 19582 | 426 | `b6ac0a761d81` |
| `apps/android/build_arch_bootstrap.py` | TEXT | 18867 | 351 | `613fe59e86af` |
| `apps/android/build_arch_runtime.py` | TEXT | 24099 | 381 | `543ce451d50a` |
| `apps/android/java/org/plyra/glass/terminal/GlassTerminalActivity.java` | TEXT | 37800 | 834 | `e385f72b381b` |
| `apps/android/java/org/plyra/glass/terminal/ManagedObjectProvider.java` | TEXT | 10302 | 211 | `376e593c5f17` |
| `apps/android/java/org/plyra/glass/terminal/TerminalAndroidApi.java` | TEXT | 9740 | 222 | `58c9b3f7f8ee` |
| `apps/android/java/org/plyra/glass/terminal/TerminalGlassWindow.java` | TEXT | 8860 | 194 | `b065b2f14fe2` |
| `apps/android/java/org/plyra/glass/terminal/TerminalInputState.java` | TEXT | 9349 | 220 | `43e70d824bc1` |
| `apps/android/java/org/plyra/glass/terminal/TerminalTouchState.java` | TEXT | 3713 | 78 | `500a86191d39` |
| `apps/android/native/glass_android_api.c` | TEXT | 8727 | 211 | `06f966e955ae` |
| `apps/android/res/drawable-xxxhdpi/glass_terminal_icon_foreground.png` | BINARY | 30923 | 127 | `c5d47ebb7a7c` |
| `apps/android/res/mipmap-anydpi-v26/glass_terminal_icon.xml` | TEXT | 286 | 5 | `2c0e5ac6938a` |
| `apps/android/res/mipmap-xxxhdpi/glass_terminal_icon.png` | BINARY | 18618 | 77 | `649650a0fe7a` |
| `apps/android/res/values/icon_colors.xml` | TEXT | 129 | 4 | `3a3f8a4b07d2` |
| `apps/android/res/values/styles.xml` | TEXT | 558 | 11 | `fbd39b89d199` |
| `apps/android/src/arch_import.rs` | TEXT | 32560 | 890 | `0bb9b3837f75` |
| `apps/android/src/bin/root_broker.rs` | TEXT | 1546 | 16 | `81eb06655dfb` |
| `apps/android/src/lib.rs` | TEXT | 16581 | 439 | `c591dea2307a` |
| `apps/android/tests/TerminalInputStateTest.java` | TEXT | 6944 | 100 | `f9362b0f0e41` |
| `apps/android/tests/TerminalTouchStateTest.java` | TEXT | 4707 | 70 | `d378ae29fe76` |
| `apps/desktop/Cargo.toml` | TEXT | 2403 | 36 | `b6d027441555` |
| `apps/desktop/build.rs` | TEXT | 1921 | 20 | `6696e04ccb2e` |
| `apps/desktop/src/android_arch.rs` | TEXT | 6595 | 177 | `76f1be1ad47e` |
| `apps/desktop/src/android_input.rs` | TEXT | 22188 | 556 | `40d5e877a1a9` |
| `apps/desktop/src/config.rs` | TEXT | 45671 | 1268 | `3cdede0a954b` |
| `apps/desktop/src/diagnostics.rs` | TEXT | 5950 | 139 | `48b22bf6c653` |
| `apps/desktop/src/extensions.rs` | TEXT | 10745 | 238 | `aa7615d2a6eb` |
| `apps/desktop/src/lib.rs` | TEXT | 145716 | 3814 | `1e04d81ba13d` |
| `apps/desktop/src/macos_menu.rs` | TEXT | 41202 | 1201 | `a62e32251301` |
| `apps/desktop/src/macos_window.rs` | TEXT | 21148 | 528 | `37ba2374b772` |
| `apps/desktop/src/main.rs` | TEXT | 5508 | 112 | `044bc59e403b` |
| `apps/desktop/src/mcp_panel.rs` | TEXT | 22234 | 557 | `4bdbbe83fa1c` |
| `apps/desktop/src/pane.rs` | TEXT | 44703 | 1249 | `b9795ed3e46b` |
| `apps/desktop/src/probe.rs` | TEXT | 19319 | 405 | `56079aa6b141` |
| `apps/desktop/src/system_panel.rs` | TEXT | 81465 | 1975 | `3805ccb7cec9` |
| `apps/desktop/src/ui.rs` | TEXT | 85749 | 1930 | `70c7f4a95032` |
| `apps/desktop/src/ui/settings_pages.rs` | TEXT | 18206 | 427 | `dde051d094b0` |
| `apps/desktop/src/ui/settings_panel.rs` | TEXT | 7902 | 179 | `265d69b6fd24` |
| `apps/desktop/src/windows_probe.rs` | TEXT | 21021 | 485 | `09e65c017905` |
| `apps/desktop/src/workspace.rs` | TEXT | 17704 | 516 | `e47c9335d4e7` |
| `assets/app-icon/GlassTerminal.icns` | BINARY | 262944 | 1102 | `5bbb7276ffa8` |
| `assets/app-icon/glass-terminal.ico` | BINARY | 50610 | 214 | `24bb43982b2c` |
| `assets/app-icon/glass-terminal.png` | BINARY | 27403 | 132 | `aad69eeebbc6` |
| `assets/app-icon/settings-preview.png` | BINARY | 77978 | 271 | `5b59338831da` |
| `assets/app-icon/window-64.rgba` | BINARY | 16384 | 5 | `95a3f244187a` |
| `assets/app-icon/windows-aarch64.obj` | BINARY | 51502 | 215 | `2b3cc4b0a3d7` |
| `assets/app-icon/windows-x86_64.obj` | BINARY | 51502 | 215 | `8d4c7682c139` |
| `assets/terminal-fonts/DejaVuSansMono.ttf` | BINARY | 340712 | 1599 | `b4a6c3e4faab` |
| `assets/terminal-fonts/LICENSE.txt` | DECLARED_EXCLUSION | 8816 | 187 | `7a083b136e64` |
| `assets/terminal-fonts/SOURCE.json` | TEXT | 1391 | 29 | `c3cdfa386be0` |
| `assets/terminal/GLASS_THIRD_PARTY_NOTICES.txt` | TEXT | 6254433 | 118144 | `789e7ac0bc05` |
| `assets/terminal/LICENSE.compiler-rt` | DECLARED_EXCLUSION | 17966 | 340 | `6c9c603e4b06` |
| `assets/terminal/LICENSE.libvterm` | DECLARED_EXCLUSION | 1098 | 23 | `66c627cf44f0` |
| `assets/terminal/LICENSE.musl` | DECLARED_EXCLUSION | 6204 | 193 | `f9bc44237323` |
| `assets/terminal/bin/plyra-terminal-linux-arm64` | BINARY | 148816 | 1314 | `4095b6cc40b6` |
| `assets/terminal/bin/plyra-terminal-macos-arm64` | BINARY | 125848 | 734 | `d73346a121a1` |
| `assets/terminal/manifest.json` | TEXT | 5419 | 89 | `b64d9b97fc9c` |
| `assets/terminal/upstream-notices/10d30a673cd5e9349bdc02aeb48f14b3386d27d0da32df8f0a555d4aa16aa551.txt` | TEXT | 10818 | 201 | `10d30a673cd5` |
| `assets/terminal/upstream-notices/12121dfa588677007848ab922a45a1e3248868f6ff063598fceb3141d7897314.txt` | TEXT | 347 | 6 | `12121dfa5886` |
| `assets/terminal/upstream-notices/1c7cf76689c837a68ed8d704994e52a0f2940c087958f860d17f3186afbdcc0c.txt` | TEXT | 1066 | 21 | `1c7cf76689c8` |
| `assets/terminal/upstream-notices/1d85bd754b04ceec93e98e890edd1fa3c6a22e81bcb32135806beeccefa51cd1.txt` | TEXT | 1071 | 19 | `1d85bd754b04` |
| `assets/terminal/upstream-notices/508a77d2e7b51d98adeed32648ad124b7b30241a8e70b2e72c99f92d8e5874d1.txt` | TEXT | 1036 | 19 | `508a77d2e7b5` |
| `assets/terminal/upstream-notices/5226e27d5fbc683db56c52853cdb9d15b69abadb525a60f95c862c2dbd77cdc8.txt` | TEXT | 1080 | 25 | `5226e27d5fbc` |
| `assets/terminal/upstream-notices/58317934bf818c379e4215fad7e8dfb6812edff32c9557b8b039343f69711e08.txt` | TEXT | 569 | 13 | `58317934bf81` |
| `assets/terminal/upstream-notices/6bec8c944a6d4251be50ac86bb4ca360bcce5aea472505dee2800e160bf1cdbf.txt` | TEXT | 574 | 13 | `6bec8c944a6d` |
| `assets/terminal/upstream-notices/7e98bed7f100747aaae7d82ab94fbe6b76fd6b84b09a3aaa413bff8196d0fbb1.txt` | TEXT | 1070 | 21 | `7e98bed7f100` |
| `assets/terminal/upstream-notices/7f976f7e9cb2d87df7230606feb932c3f21ac0e664045a775b600046ff850c54.txt` | TEXT | 1339 | 34 | `7f976f7e9cb2` |
| `assets/terminal/upstream-notices/8173d5c29b4f956d532781d2b86e4e30f83e6b7878dce18c919451d6ba707c90.txt` | TEXT | 10850 | 201 | `8173d5c29b4f` |
| `assets/terminal/upstream-notices/95ca92f5f8ea5231f1580b3a2a799e8260af3114b900e1def5355a7f44bcf60c.txt` | TEXT | 1092 | 25 | `95ca92f5f8ea` |
| `assets/terminal/upstream-notices/a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2.txt` | TEXT | 10847 | 201 | `a60eea817514` |
| `assets/terminal/upstream-notices/a6cba85bc92e0cff7a450b1d873c0eaa2e9fc96bf472df0247a26bec77bf3ff9.txt` | TEXT | 10173 | 176 | `a6cba85bc92e` |
| `assets/terminal/upstream-notices/b40930bbcf80744c86c46a12bc9da056641d722716c378f5659b9e555ef833e1.txt` | TEXT | 11357 | 201 | `b40930bbcf80` |
| `assets/terminal/upstream-notices/c6596eb7be8581c18be736c846fb9173b69eccf6ef94c5135893ec56bd92ba08.txt` | TEXT | 11358 | 202 | `c6596eb7be85` |
| `assets/terminal/upstream-notices/c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4.txt` | TEXT | 11357 | 201 | `c71d239df917` |
| `assets/terminal/upstream-notices/c7fea58d1cfe49634cd92e54fc10a9d871f4b275321a4cd8c09e449122caaeb4.txt` | TEXT | 1078 | 21 | `c7fea58d1cfe` |
| `assets/terminal/upstream-notices/c8167fdeeed46d3f244d3f85c5bf998ce889343691c32be2c61a8bc4b5c08333.txt` | TEXT | 1093 | 21 | `c8167fdeeed4` |
| `assets/terminal/upstream-notices/c9bff75738922193e67fa726fa225535870d2aa1059f91452c411736284ad566.txt` | TEXT | 1338 | 23 | `c9bff7573892` |
| `assets/terminal/upstream-notices/ce7bc3499fee93d5022ef430d5e4201e79a6d9154f3974e42f41349f0569e09b.txt` | TEXT | 1073 | 19 | `ce7bc3499fee` |
| `assets/terminal/upstream-notices/cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30.txt` | TEXT | 11358 | 202 | `cfc7749b96f6` |
| `assets/terminal/upstream-notices/d027e91dbc9cdbb2f1190068e498bd6b61cff022b6a032b191021ba658d96111.txt` | TEXT | 3733 | 75 | `d027e91dbc9c` |
| `assets/terminal/upstream-notices/e353f37b12aefbb9f9b29490e837cfee05d9bda70804b3562839a3285c1df1e5.txt` | TEXT | 1066 | 21 | `e353f37b12ae` |
| `assets/terminal/upstream-notices/f72143f7e9d9fb8bd336b139fa22f825a299591c29f2f7338c5566347b137613.txt` | TEXT | 352 | 6 | `f72143f7e9d9` |
| `assets/terminal/upstream-notices/f774bf68151714985324d90adbd59e8f271b7ba518608d871b6ba16464692879.txt` | TEXT | 1075 | 25 | `f774bf681517` |
| `assets/terminal/upstream-notices/fea1d5bf3dd71605ce5d7d2ff695c1837e914c77195e523a86b5391716477960.txt` | TEXT | 1104 | 21 | `fea1d5bf3dd7` |
| `assets/terminal/upstream-notices/ff92bed461f50338dd703a9ba9aee496a425957873df1d91192776e4bdf5dda7.txt` | TEXT | 3693 | 74 | `ff92bed461f5` |
| `assets/terminal/upstream-notices/manifest.json` | TEXT | 73301 | 1729 | `3b0f48c8a489` |
| `assets/terminal/upstream-notices/runtime/rust-1.98.0-nightly-f28ac764c/COPYRIGHT-library.html` | TEXT | 1435128 | 28091 | `7807d12c0c8a` |
| `assets/terminal/upstream-notices/runtime/rust-1.98.0-nightly-f28ac764c/LICENSE-APACHE.txt` | DECLARED_EXCLUSION | 10280 | 73 | `074e6e32c86a` |
| `assets/terminal/upstream-notices/runtime/rust-1.98.0-nightly-f28ac764c/LICENSE-MIT.txt` | DECLARED_EXCLUSION | 1078 | 9 | `b85dcd3e453d` |
| `assets/terminal/upstream-notices/runtime/rust-1.98.0-nightly-f28ac764c/LICENSE.compiler-builtins.txt` | DECLARED_EXCLUSION | 15078 | 275 | `ab6eec6caf0f` |
| `assets/terminal/upstream-notices/runtime/rust-1.98.0-nightly-f28ac764c/LICENSE.portable-simd-MIT.txt` | DECLARED_EXCLUSION | 1071 | 19 | `eb07d497d26e` |
| `assets/terminal/upstream-notices/runtime/rust-1.98.0-nightly-f28ac764c/LICENSE.stdarch-MIT.txt` | DECLARED_EXCLUSION | 1071 | 25 | `29662666b44d` |
| `assets/terminal/upstream-notices/runtime/zig-0.17.0/COPYING.mingw-w64.txt` | DECLARED_EXCLUSION | 2326 | 43 | `99a696609811` |
| `assets/terminal/upstream-notices/runtime/zig-0.17.0/LICENSE.Zig.txt` | DECLARED_EXCLUSION | 1080 | 21 | `5c537d6853e0` |
| `assets/terminal/upstream-notices/runtime/zig-0.17.0/LICENSE.libunwind.txt` | DECLARED_EXCLUSION | 16706 | 311 | `b5efebcaca80` |
| `assets/terminal/upstream-notices/runtime/zig-0.17.0/LICENSE.winpthreads-mingw.txt` | DECLARED_EXCLUSION | 1120 | 21 | `89a665a4dddb` |
| `assets/terminal/upstream-notices/runtime/zig-0.17.0/LICENSE.winpthreads-mutex.txt` | DECLARED_EXCLUSION | 1160 | 22 | `ef9037661f83` |
| `assets/terminal/upstream-notices/runtime/zig-0.17.0/NOTICE.winpthreads-public-domain.txt` | TEXT | 215 | 5 | `5ffbd86993a2` |
| `assets/ui-fonts/Geist.ttf` | BINARY | 169056 | 1267 | `73894e0448ca` |
| `assets/ui-fonts/OFL.txt` | TEXT | 4387 | 93 | `1781d2806a07` |
| `assets/ui-fonts/SOURCE.json` | TEXT | 551 | 17 | `b5f37fdfd935` |
| `crates/mcp-client/Cargo.toml` | TEXT | 2662 | 39 | `34e1f0137b13` |
| `crates/mcp-client/src/bin/owned_fixture.rs` | TEXT | 8320 | 194 | `6204ac0bf26d` |
| `crates/mcp-client/src/client.rs` | TEXT | 32457 | 863 | `01a7f55aff6a` |
| `crates/mcp-client/src/config.rs` | TEXT | 10849 | 263 | `8cbe58b71700` |
| `crates/mcp-client/src/lib.rs` | TEXT | 3264 | 58 | `a4cb9009ec6d` |
| `crates/mcp-client/src/transport.rs` | TEXT | 7264 | 185 | `4ba72f2b9b44` |
| `crates/mcp-client/src/types.rs` | TEXT | 3502 | 88 | `2200a6360e5a` |
| `crates/mcp-client/tests/protocol.rs` | TEXT | 11401 | 266 | `f2b43a188a77` |
| `crates/privilege-broker/Cargo.toml` | TEXT | 1851 | 33 | `3c4995f2de59` |
| `crates/privilege-broker/src/android_root.rs` | TEXT | 5761 | 116 | `f15a1a26607b` |
| `crates/privilege-broker/src/lib.rs` | TEXT | 28945 | 731 | `895902db15b1` |
| `crates/privilege-broker/src/macos_admin.rs` | TEXT | 5104 | 104 | `3611ffb2c44e` |
| `crates/privilege-broker/src/main.rs` | TEXT | 1325 | 8 | `3099689b8c76` |
| `crates/privilege-broker/src/windows.rs` | TEXT | 20416 | 523 | `a52ccd6d1854` |
| `crates/privilege-broker/tests/ipc.rs` | TEXT | 6975 | 167 | `4f039a23b09f` |
| `crates/privilege-policy/Cargo.toml` | TEXT | 1415 | 14 | `155b48c30701` |
| `crates/privilege-policy/src/lib.rs` | TEXT | 8013 | 188 | `07d4db225866` |
| `crates/session-manager/Cargo.toml` | TEXT | 1664 | 22 | `574d46dec904` |
| `crates/session-manager/build.rs` | TEXT | 3531 | 59 | `5d4c9ffa6238` |
| `crates/session-manager/examples/aether_shell_acceptance.rs` | TEXT | 7930 | 189 | `72af53a60e84` |
| `crates/session-manager/examples/macos_compat.rs` | TEXT | 8900 | 200 | `ec3e503b8a1b` |
| `crates/session-manager/examples/ssh_loopback.rs` | TEXT | 5387 | 123 | `2131a12a31c4` |
| `crates/session-manager/examples/throughput.rs` | TEXT | 4954 | 106 | `5bf827ba107e` |
| `crates/session-manager/src/aether_events.rs` | TEXT | 37083 | 938 | `da4bffde377f` |
| `crates/session-manager/src/lib.rs` | TEXT | 110354 | 3068 | `d65a3a409e90` |
| `crates/session-manager/src/shell_integration.rs` | TEXT | 5142 | 122 | `3281591d0ef7` |
| `crates/session-manager/tests/fixtures/portable_ansi_cjk_alternate.json` | TEXT | 19551 | 163 | `80896cd1cd31` |
| `crates/system-api/Cargo.toml` | TEXT | 2192 | 45 | `4a759929d267` |
| `crates/system-api/src/android_root.rs` | TEXT | 15107 | 412 | `4dc9febb455b` |
| `crates/system-api/src/android_system.rs` | TEXT | 6288 | 146 | `ecee29ddb32a` |
| `crates/system-api/src/audit.rs` | TEXT | 9887 | 245 | `4979261a3de5` |
| `crates/system-api/src/backend.rs` | TEXT | 29515 | 849 | `9c31974c393c` |
| `crates/system-api/src/bin/glass-system-bridge.rs` | TEXT | 11624 | 294 | `071cf067615f` |
| `crates/system-api/src/broker_client.rs` | TEXT | 42000 | 1121 | `eab719ebac74` |
| `crates/system-api/src/identity.rs` | TEXT | 8326 | 218 | `e64f6caa8380` |
| `crates/system-api/src/ipc.rs` | TEXT | 5197 | 139 | `0d8f898ef1cd` |
| `crates/system-api/src/lib.rs` | TEXT | 11000 | 274 | `7232579c23b5` |
| `crates/system-api/src/linux_admin.rs` | TEXT | 5624 | 132 | `5bdbcaa49365` |
| `crates/system-api/src/linux_observation.rs` | TEXT | 21358 | 538 | `5e5f9469c149` |
| `crates/system-api/src/linux_system.rs` | TEXT | 18801 | 517 | `118c8015e515` |
| `crates/system-api/src/macos_admin.rs` | TEXT | 19959 | 551 | `bef5fef97a5e` |
| `crates/system-api/src/observation.rs` | TEXT | 26550 | 656 | `ca12a887799d` |
| `crates/system-api/src/operations.rs` | TEXT | 44886 | 1126 | `95c960a10a97` |
| `crates/system-api/src/plugin.rs` | TEXT | 8317 | 196 | `71481112bf89` |
| `crates/system-api/src/power.rs` | TEXT | 5329 | 114 | `146930a595bf` |
| `crates/system-api/src/recovery.rs` | TEXT | 20444 | 554 | `80db7a6633ef` |
| `crates/system-api/src/recovery/windows.rs` | TEXT | 24919 | 693 | `79ad769af603` |
| `crates/system-api/src/types.rs` | TEXT | 6241 | 201 | `40038d5d1da4` |
| `crates/system-api/src/windows.rs` | TEXT | 1808 | 20 | `8875589439ea` |
| `crates/system-api/src/windows_broker.rs` | TEXT | 12816 | 338 | `6124b124d07b` |
| `crates/system-api/src/windows_identity.rs` | TEXT | 10518 | 306 | `5f024fd062e2` |
| `crates/system-api/src/windows_ipc.rs` | TEXT | 12328 | 323 | `086615f198d4` |
| `crates/system-api/src/windows_services.rs` | TEXT | 13231 | 355 | `5ac685e10124` |
| `crates/system-api/src/windows_system.rs` | TEXT | 14522 | 387 | `9aca067ee9c0` |
| `crates/system-api/src/windows_trust.rs` | TEXT | 7926 | 199 | `32aa79f8e60b` |
| `crates/terminal-core/Cargo.toml` | TEXT | 1433 | 14 | `94578f2cec25` |
| `crates/terminal-core/src/history.rs` | TEXT | 8372 | 229 | `3046341b9187` |
| `crates/terminal-core/src/lib.rs` | TEXT | 22375 | 602 | `48ae7863d04b` |
| `crates/terminal-core/tests/fixtures/ansi_cjk_alternate.events.json` | TEXT | 5057 | 143 | `e4cd9fc4faf1` |
| `crates/terminal-core/tests/fixtures/ansi_cjk_alternate.screens.json` | TEXT | 4727 | 123 | `1624ba0add9e` |
| `crates/terminal-render/Cargo.toml` | TEXT | 2462 | 47 | `c0f3954037c6` |
| `crates/terminal-render/src/android_glass.rs` | TEXT | 5720 | 148 | `13ece4349337` |
| `crates/terminal-render/src/glass.rs` | TEXT | 17389 | 449 | `54cf02b40bc4` |
| `crates/terminal-render/src/glass.wgsl` | TEXT | 4606 | 84 | `e8e0b0d821b4` |
| `crates/terminal-render/src/lib.rs` | TEXT | 87307 | 2305 | `c513af672f70` |
| `crates/terminal-render/src/linux_glass.rs` | TEXT | 18567 | 491 | `4dd453659471` |
| `crates/terminal-render/src/native_glass.rs` | TEXT | 22807 | 513 | `6d954f2b12ef` |
| `crates/terminal-render/src/rectangles.wgsl` | TEXT | 1673 | 23 | `5a415a85d293` |
| `crates/terminal-render/src/rounded_window.rs` | TEXT | 12065 | 281 | `870fc5863235` |
| `crates/terminal-render/src/rounded_window.wgsl` | TEXT | 2197 | 27 | `d00c73ea24c3` |
| `crates/terminal-render/src/shaped_runs.rs` | TEXT | 8589 | 215 | `c423266d6d92` |
| `crates/terminal-render/src/windows_glass.rs` | TEXT | 9975 | 271 | `78ee9bc22a6a` |
| `crates/terminal-render/src/windows_surface.rs` | TEXT | 9152 | 227 | `3cfaf6008b61` |
| `docs/ARCHITECTURE.md` | TEXT | 5501 | 46 | `a9419323b779` |
| `docs/DECISIONS/ADR-001-rust-spike-and-reuse.md` | TEXT | 1048 | 15 | `dfcdc83b05fb` |
| `docs/DEPENDENCY_LICENSES.md` | TEXT | 79759 | 649 | `3a334b036ce2` |
| `docs/RELEASE_VERIFICATION.md` | TEXT | 1376 | 19 | `ade60f2f3a96` |
| `docs/SECURITY.md` | TEXT | 8434 | 75 | `680218010209` |
| `packaging/linux/README.md` | TEXT | 3051 | 50 | `31bf7f5cf569` |
| `packaging/linux/install.sh` | TEXT | 1320 | 6 | `05971108607c` |
| `packaging/linux/install_broker.py` | TEXT | 4734 | 86 | `ef8b9d1fab22` |
| `packaging/linux/installer.py` | TEXT | 13067 | 271 | `ee3dc55fe8fd` |
| `packaging/linux/org.plyra.glass-terminal.desktop` | TEXT | 275 | 11 | `95ae5f5e67a7` |
| `packaging/linux/org.plyra.glass-terminal.policy` | TEXT | 868 | 17 | `9ad98de92c1c` |
| `packaging/linux/uninstall.sh` | TEXT | 1322 | 6 | `b5bec1e61ebc` |
| `packaging/windows/install-shortcut.ps1` | TEXT | 2879 | 41 | `96a5aa48f420` |
| `project.godot` | TEXT | 843 | 24 | `3197c4326360` |
| `provenance/release-key.json` | TEXT | 168 | 4 | `edef925098b6` |
| `provenance/source-inventory.json` | TEXT | 133306 | 3045 | `08fb8e92a0d6` |
| `runtime-manifest.json` | TEXT | 8384 | 238 | `f6ab00e5d538` |
| `scripts/terminal/terminal_canvas.gd` | TEXT | 13584 | 244 | `0b40622569f4` |
| `scripts/terminal/terminal_input.gd` | TEXT | 3096 | 52 | `8a2873beef59` |
| `scripts/terminal/terminal_screen.gd` | TEXT | 8175 | 154 | `fd73335d90c8` |
| `scripts/terminal/terminal_session.gd` | TEXT | 22972 | 417 | `3cd9b1a5f16e` |
| `scripts/terminal/terminal_style.gd` | TEXT | 5817 | 78 | `7c85cee31096` |
| `scripts/terminal/terminal_view.gd` | TEXT | 13376 | 233 | `839f4d542d4d` |
| `shaders/terminal/acrylic.gdshader` | TEXT | 2996 | 36 | `209e77cdca13` |
| `standalone/main.gd` | TEXT | 4323 | 74 | `eb4424b328f0` |
| `standalone/main.tscn` | TEXT | 269 | 10 | `ab6f0bc074d5` |
| `tests/native_window_probe.gd` | TEXT | 5768 | 76 | `74f4315c8908` |
| `tests/profile_probe.gd` | TEXT | 1372 | 7 | `a58ed1b1fb46` |
| `tests/standalone_close_test.gd` | TEXT | 1916 | 17 | `abae35c74417` |
| `tests/terminal_screen_test.gd` | TEXT | 6701 | 69 | `f5219a2019bd` |
| `tests/terminal_session_test.gd` | TEXT | 12026 | 163 | `a23f467c95b5` |
| `tests/terminal_view_test.gd` | TEXT | 7302 | 107 | `983b0b860172` |
| `tools/build_helper.py` | TEXT | 2056 | 23 | `b10e834e8ae0` |
| `tools/build_linux_app.py` | TEXT | 11005 | 198 | `da8f8f65d162` |
| `tools/build_macos_app.py` | TEXT | 13163 | 216 | `f621f8243027` |
| `tools/check.py` | TEXT | 5960 | 73 | `51b4991b7517` |
| `tools/check_native_window.py` | TEXT | 4486 | 50 | `ba9ca69ac9b8` |
| `tools/generate_app_icon.py` | TEXT | 6014 | 122 | `4cc5d75d3944` |
| `tools/glass_licenses.py` | TEXT | 18440 | 380 | `86cb3ba55de4` |
| `tools/install_windows_broker.ps1` | TEXT | 11131 | 178 | `05d80d3ea533` |
| `tools/mobile/terminal/README.md` | TEXT | 7682 | 124 | `1dbd0f4b0939` |
| `tools/mobile/terminal/build.py` | TEXT | 6556 | 61 | `c6bf36831dbd` |
| `tools/mobile/terminal/plyra_terminal.c` | TEXT | 33110 | 530 | `e1b0d5b1d3f5` |
| `tools/mobile/terminal/test_backend.py` | TEXT | 14832 | 204 | `eab3ec2c76d7` |
| `tools/mobile/terminal/vendor/PROVENANCE.json` | TEXT | 3350 | 96 | `6310cfc94391` |
| `tools/mobile/terminal/vendor/libvterm/LICENSE` | DECLARED_EXCLUSION | 1098 | 23 | `66c627cf44f0` |
| `tools/mobile/terminal/vendor/libvterm/include/vterm.h` | TEXT | 21540 | 642 | `53ddfaaf21f8` |
| `tools/mobile/terminal/vendor/libvterm/include/vterm_keycodes.h` | TEXT | 1106 | 61 | `2a8a667b8f82` |
| `tools/mobile/terminal/vendor/libvterm/src/encoding.c` | TEXT | 5752 | 230 | `f5a91b627ee0` |
| `tools/mobile/terminal/vendor/libvterm/src/encoding/DECdrawing.inc` | TEXT | 757 | 36 | `d8c25047d5f4` |
| `tools/mobile/terminal/vendor/libvterm/src/encoding/uk.inc` | TEXT | 119 | 6 | `9dad74b7ced6` |
| `tools/mobile/terminal/vendor/libvterm/src/fullwidth.inc` | TEXT | 2544 | 111 | `41c4b4a0722c` |
| `tools/mobile/terminal/vendor/libvterm/src/keyboard.c` | TEXT | 6159 | 226 | `3d0f3ac0c46a` |
| `tools/mobile/terminal/vendor/libvterm/src/mouse.c` | TEXT | 2507 | 99 | `2824a884d4ac` |
| `tools/mobile/terminal/vendor/libvterm/src/parser.c` | TEXT | 10837 | 402 | `237220e912ab` |
| `tools/mobile/terminal/vendor/libvterm/src/pen.c` | TEXT | 17535 | 607 | `f97dde69a6e7` |
| `tools/mobile/terminal/vendor/libvterm/src/rect.h` | TEXT | 2022 | 56 | `8ac51df5e5c7` |
| `tools/mobile/terminal/vendor/libvterm/src/screen.c` | TEXT | 35987 | 1215 | `af234b141ad0` |
| `tools/mobile/terminal/vendor/libvterm/src/state.c` | TEXT | 63253 | 2373 | `66a7911483a2` |
| `tools/mobile/terminal/vendor/libvterm/src/unicode.c` | TEXT | 14254 | 313 | `a3905a313d88` |
| `tools/mobile/terminal/vendor/libvterm/src/utf8.h` | TEXT | 1040 | 39 | `2a1c6ffd176d` |
| `tools/mobile/terminal/vendor/libvterm/src/vterm.c` | TEXT | 10703 | 430 | `f8620e99da94` |
| `tools/mobile/terminal/vendor/libvterm/src/vterm_internal.h` | TEXT | 7293 | 301 | `a27df4e1ec76` |
| `tools/mobile/terminal/vendor/runtime-licenses/LICENSE.compiler-rt` | DECLARED_EXCLUSION | 17966 | 340 | `6c9c603e4b06` |
| `tools/mobile/terminal/vendor/runtime-licenses/LICENSE.llvm-compiler-rt` | DECLARED_EXCLUSION | 16708 | 311 | `1a8f1058753f` |
| `tools/mobile/terminal/vendor/runtime-licenses/LICENSE.llvm-compiler-rt-8` | DECLARED_EXCLUSION | 4289 | 91 | `dd27f8c290bc` |
| `tools/mobile/terminal/vendor/runtime-licenses/LICENSE.musl` | DECLARED_EXCLUSION | 6204 | 193 | `f9bc44237323` |
| `tools/mobile/terminal/vendor/runtime-licenses/LICENSE.zig` | DECLARED_EXCLUSION | 1080 | 21 | `5c537d6853e0` |
| `tools/mobile/terminal/vendor/runtime-licenses/PROVENANCE.json` | TEXT | 3696 | 73 | `f972a3f42f55` |
| `tools/package_windows_app.py` | TEXT | 4388 | 79 | `ab739e9bdcdc` |
| `tools/refresh_manifest.py` | TEXT | 2691 | 32 | `bcf42389efeb` |
| `tools/test_profile.py` | TEXT | 2747 | 33 | `f47e2e863eac` |
| `vendor/block/Cargo.toml` | TEXT | 648 | 25 | `7bd75ac7a2ca` |
| `vendor/block/PROVENANCE.json` | TEXT | 2065 | 27 | `4b27a8f8f585` |
| `vendor/block/README.md` | TEXT | 1264 | 42 | `01c56329fa14` |
| `vendor/block/src/lib.rs` | TEXT | 12791 | 397 | `70f4eb382cfb` |
| `vendor/block/src/test_utils.rs` | TEXT | 754 | 31 | `271bd579b67e` |
| `vendor/block/tests/block_runtime.rs` | TEXT | 848 | 34 | `342097ad6a18` |
| `vendor/vt100/.cargo_vcs_info.json` | TEXT | 94 | 6 | `a897cc49134f` |
| `vendor/vt100/CHANGELOG.md` | TEXT | 11182 | 396 | `c0a75dec3b5c` |
| `vendor/vt100/Cargo.lock` | DECLARED_EXCLUSION | 14732 | 540 | `4553783691a9` |
| `vendor/vt100/Cargo.toml` | TEXT | 1561 | 75 | `bd6cbf4770d7` |
| `vendor/vt100/Cargo.toml.orig` | TEXT | 732 | 28 | `066bfd0fa6bd` |
| `vendor/vt100/LICENSE` | DECLARED_EXCLUSION | 1079 | 21 | `53a04b0a1107` |
| `vendor/vt100/PROVENANCE.json` | TEXT | 2944 | 38 | `d108f16a5f18` |
| `vendor/vt100/README.md` | TEXT | 1050 | 36 | `01d073eaeefb` |
| `vendor/vt100/src/attrs.rs` | TEXT | 4132 | 163 | `4cc732bd257d` |
| `vendor/vt100/src/callbacks.rs` | TEXT | 2880 | 69 | `45b2a1b3f8a3` |
| `vendor/vt100/src/cell.rs` | TEXT | 5859 | 206 | `e6a49560b913` |
| `vendor/vt100/src/grid.rs` | TEXT | 25725 | 742 | `d0643ac5f3ce` |
| `vendor/vt100/src/hyperlink.rs` | TEXT | 5641 | 153 | `27e56f6f2d29` |
| `vendor/vt100/src/lib.rs` | TEXT | 1873 | 65 | `47fcb3b47a0e` |
| `vendor/vt100/src/parser.rs` | TEXT | 2824 | 96 | `c5ea946a8f0a` |
| `vendor/vt100/src/perform.rs` | TEXT | 10361 | 283 | `04b195933cdb` |
| `vendor/vt100/src/row.rs` | TEXT | 16614 | 474 | `133e1f6e110e` |
| `vendor/vt100/src/screen.rs` | TEXT | 48120 | 1371 | `284531801bc2` |
| `vendor/vt100/src/term.rs` | TEXT | 14308 | 562 | `eb88230936fb` |
| `vendor/wgpu-hal/.cargo_vcs_info.json` | TEXT | 102 | 6 | `e085ab29dd74` |
| `vendor/wgpu-hal/Cargo.lock` | DECLARED_EXCLUSION | 58190 | 2246 | `1855bb300dc5` |
| `vendor/wgpu-hal/Cargo.toml` | TEXT | 7877 | 347 | `afe826c2170d` |
| `vendor/wgpu-hal/Cargo.toml.orig` | TEXT | 7892 | 239 | `c7d368c2ec77` |
| `vendor/wgpu-hal/LICENSE.APACHE` | DECLARED_EXCLUSION | 17 | 1 | `93322bf14207` |
| `vendor/wgpu-hal/LICENSE.MIT` | DECLARED_EXCLUSION | 14 | 1 | `10b693f40602` |
| `vendor/wgpu-hal/PROVENANCE.json` | TEXT | 1775 | 31 | `0e0c086cebca` |
| `vendor/wgpu-hal/README.md` | TEXT | 5412 | 120 | `f013dde4920e` |
| `vendor/wgpu-hal/build.rs` | TEXT | 903 | 17 | `987c89c78da2` |
| `vendor/wgpu-hal/examples/halmark/main.rs` | TEXT | 32940 | 884 | `9ebcd62dc5c3` |
| `vendor/wgpu-hal/examples/halmark/shader.wgsl` | TEXT | 1204 | 50 | `f4bbeaaa9c5e` |
| `vendor/wgpu-hal/examples/raw-gles.em.html` | TEXT | 446 | 16 | `b0bacaa51e6c` |
| `vendor/wgpu-hal/examples/raw-gles.rs` | TEXT | 13395 | 339 | `d26a51eca431` |
| `vendor/wgpu-hal/examples/ray-traced-triangle/main.rs` | TEXT | 44141 | 1185 | `b13165027e0d` |
| `vendor/wgpu-hal/examples/ray-traced-triangle/shader.wgsl` | TEXT | 1315 | 37 | `ff9cc0b8608a` |
| `vendor/wgpu-hal/src/auxil/dxgi/conv.rs` | TEXT | 12918 | 306 | `cdc6bc8d5e0c` |
| `vendor/wgpu-hal/src/auxil/dxgi/exception.rs` | TEXT | 3596 | 98 | `30ca24d15005` |
| `vendor/wgpu-hal/src/auxil/dxgi/factory.rs` | TEXT | 5873 | 165 | `0f6b0dcdb447` |
| `vendor/wgpu-hal/src/auxil/dxgi/mod.rs` | TEXT | 85 | 5 | `5931289b2a39` |
| `vendor/wgpu-hal/src/auxil/dxgi/result.rs` | TEXT | 1252 | 32 | `a81cfe5a57ba` |
| `vendor/wgpu-hal/src/auxil/dxgi/time.rs` | TEXT | 3810 | 93 | `6c5f2f8c9465` |
| `vendor/wgpu-hal/src/auxil/mod.rs` | TEXT | 3918 | 117 | `d7f8487c78e4` |
| `vendor/wgpu-hal/src/auxil/renderdoc.rs` | TEXT | 4780 | 140 | `4e18fd491411` |
| `vendor/wgpu-hal/src/dx12/adapter.rs` | TEXT | 35978 | 844 | `5298d9c056aa` |
| `vendor/wgpu-hal/src/dx12/command.rs` | TEXT | 59401 | 1463 | `27c43da859b9` |
| `vendor/wgpu-hal/src/dx12/conv.rs` | TEXT | 17065 | 400 | `d322691c8726` |
| `vendor/wgpu-hal/src/dx12/descriptor.rs` | TEXT | 10369 | 323 | `de08234462e4` |
| `vendor/wgpu-hal/src/dx12/device.rs` | TEXT | 87473 | 2136 | `5bace86fc7ac` |
| `vendor/wgpu-hal/src/dx12/instance.rs` | TEXT | 5555 | 149 | `b6d6b18f8a79` |
| `vendor/wgpu-hal/src/dx12/mod.rs` | TEXT | 46907 | 1381 | `357561455f09` |
| `vendor/wgpu-hal/src/dx12/shader_compilation.rs` | TEXT | 11485 | 349 | `ed1d432e33fc` |
| `vendor/wgpu-hal/src/dx12/suballocation.rs` | TEXT | 14467 | 407 | `f947c8e5b338` |
| `vendor/wgpu-hal/src/dx12/types.rs` | TEXT | 1244 | 39 | `a3d6a478f7a0` |
| `vendor/wgpu-hal/src/dx12/view.rs` | TEXT | 15973 | 346 | `5261864299c5` |
| `vendor/wgpu-hal/src/dynamic/adapter.rs` | TEXT | 2135 | 67 | `e7399ca7b061` |
| `vendor/wgpu-hal/src/dynamic/command.rs` | TEXT | 21471 | 655 | `82cea1589d92` |
| `vendor/wgpu-hal/src/dynamic/device.rs` | TEXT | 19916 | 530 | `997458c1f15f` |
| `vendor/wgpu-hal/src/dynamic/instance.rs` | TEXT | 2275 | 68 | `e57fd8cdcc4a` |
| `vendor/wgpu-hal/src/dynamic/mod.rs` | TEXT | 8147 | 206 | `ab269c0f1595` |
| `vendor/wgpu-hal/src/dynamic/queue.rs` | TEXT | 1849 | 54 | `fd05dfc8eaba` |
| `vendor/wgpu-hal/src/dynamic/surface.rs` | TEXT | 2258 | 71 | `2516cd1c32f5` |
| `vendor/wgpu-hal/src/empty.rs` | TEXT | 15189 | 513 | `770fdb545076` |
| `vendor/wgpu-hal/src/gles/adapter.rs` | TEXT | 54375 | 1290 | `e66e5031dfd1` |
| `vendor/wgpu-hal/src/gles/command.rs` | TEXT | 46353 | 1216 | `9650e8c1ee67` |
| `vendor/wgpu-hal/src/gles/conv.rs` | TEXT | 20572 | 428 | `043bd3eecf7c` |
| `vendor/wgpu-hal/src/gles/device.rs` | TEXT | 65423 | 1645 | `a9d20ee88dee` |
| `vendor/wgpu-hal/src/gles/egl.rs` | TEXT | 56894 | 1514 | `93c77473a5e0` |
| `vendor/wgpu-hal/src/gles/emscripten.rs` | TEXT | 1005 | 26 | `f368b40cd842` |
| `vendor/wgpu-hal/src/gles/mod.rs` | TEXT | 34163 | 1149 | `a799a9004d2a` |
| `vendor/wgpu-hal/src/gles/queue.rs` | TEXT | 86285 | 1882 | `4975a7776fe5` |
| `vendor/wgpu-hal/src/gles/shaders/clear.frag` | TEXT | 189 | 7 | `29ce6d9f41f9` |
| `vendor/wgpu-hal/src/gles/shaders/clear.vert` | TEXT | 217 | 9 | `bd523bb37b1c` |
| `vendor/wgpu-hal/src/gles/shaders/srgb_present.frag` | TEXT | 530 | 16 | `66aeac34f56a` |
| `vendor/wgpu-hal/src/gles/shaders/srgb_present.vert` | TEXT | 419 | 18 | `d1889cc144d3` |
| `vendor/wgpu-hal/src/gles/web.rs` | TEXT | 16775 | 444 | `c0a9de63eb06` |
| `vendor/wgpu-hal/src/gles/wgl.rs` | TEXT | 30248 | 876 | `c4214b0600b9` |
| `vendor/wgpu-hal/src/lib.rs` | TEXT | 98957 | 2536 | `27d0f79f1294` |
| `vendor/wgpu-hal/src/metal/adapter.rs` | TEXT | 53507 | 1233 | `d4f7720d97f6` |
| `vendor/wgpu-hal/src/metal/command.rs` | TEXT | 52751 | 1313 | `e1deb04710d8` |
| `vendor/wgpu-hal/src/metal/conv.rs` | TEXT | 11932 | 366 | `d0225eacb231` |
| `vendor/wgpu-hal/src/metal/device.rs` | TEXT | 65592 | 1539 | `0df754cc1ec0` |
| `vendor/wgpu-hal/src/metal/layer_observer.rs` | TEXT | 7311 | 190 | `280d533027bf` |
| `vendor/wgpu-hal/src/metal/mod.rs` | TEXT | 29818 | 991 | `5615182c5eb3` |
| `vendor/wgpu-hal/src/metal/surface.rs` | TEXT | 9231 | 227 | `62ea7bc85054` |
| `vendor/wgpu-hal/src/metal/time.rs` | TEXT | 1043 | 38 | `c3c1b44a6c01` |
| `vendor/wgpu-hal/src/vulkan/adapter.rs` | TEXT | 112748 | 2526 | `9be689dadc9a` |
| `vendor/wgpu-hal/src/vulkan/command.rs` | TEXT | 43783 | 1163 | `46a1d54095b7` |
| `vendor/wgpu-hal/src/vulkan/conv.rs` | TEXT | 41129 | 998 | `96d26ad6f379` |
| `vendor/wgpu-hal/src/vulkan/device.rs` | TEXT | 107266 | 2692 | `9448a7e05de4` |
| `vendor/wgpu-hal/src/vulkan/instance.rs` | TEXT | 46172 | 1118 | `6bb468a92fa5` |
| `vendor/wgpu-hal/src/vulkan/mod.rs` | TEXT | 52692 | 1460 | `b4a923bd282f` |
| `vendor/wgpu-hal/src/vulkan/sampler.rs` | TEXT | 7137 | 178 | `1152247b754a` |

## 复算命令

```sh
git clone --depth 1 https://github.com/J-x-Z/plyra-terminal /tmp/opencode/external/plyra-terminal
git -C /tmp/opencode/external/plyra-terminal ls-files -z | xargs -0 sha256sum
git -C /tmp/opencode/external/plyra-terminal ls-files | wc -l
```
