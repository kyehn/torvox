# 参考项目与依赖源码事实核验报告

审查日期：2026-09-26。基准：`main` @ `84e59d0`。本报告是 [REVIEW.md](REVIEW.md) 的第二阶段，聚焦「规范文档所述的外部事实是否属实」。

## 审查方法

把 `docs/specification/REFERENCE.md` 引用的 **10 个参考项目全部克隆到本地**，把 Rust 依赖源码定位到 cargo registry 与 git checkout，把上游 ghostty 单独克隆，然后逐条把文档断言与真实源码比对：

| 类别 | 来源 | 数量 |
| --- | --- | --- |
| 参考项目 | `/tmp/refs/` 浅克隆 | 11 个仓库 |
| Rust 依赖源码 | `/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` | 369 个 crate |
| git 依赖 | `/home/runner/.cargo/git/checkouts/libghostty-rs-*/5988a0b/` | libghostty-vt + libghostty-vt-sys |
| 上游 ghostty | `/tmp/refs/ghostty` | HEAD `6301810a48aa` |
| 产物实测 | `readelf -d`、`strings`、`nm` on `target/x86_64-linux-android/debug/libnative.so` | — |

文档中不可验证的描述（需 git 历史才能判定的意图、无法离线复现的设备行为）标注为「不可验证」，不计入问题数。

克隆清单（全部成功）：`termux-app`、`ghostty-android-terminal`、`Haven`、`termlib`、`zed-android-port`、`wgpu-in-app`、`zelland`、`termux-kotlin-app`、`gnome-console`、`nix-on-droid`、`ghostty`。

## 结论摘要

**问题 31 项**，其中 **REFERENCE.md 有 14 条断言与真实参考项目不符（1 条完全编造、1 条说反了、1 条把 torvox 已做对的事记在参考项目头上）**。

| 维度 | P0 | P1 | P2 | P3 | 小计 |
| --- | --- | --- | --- | --- | --- |
| C 参考项目事实错误 | 0 | 8 | 5 | 1 | 14 |
| D 依赖源码事实错误 | 2 | 3 | 6 | 1 | 12 |
| F 规范引用缺失/空引用 | 0 | 0 | 1 | 4 | 5 |
| 合计 | 2 | 11 | 12 | 6 | 31 |

最严重的三项：

1. `DESIGN.md:72`「上游 `libghostty-vt` / `libghostty-vt-sys` 固定跟踪 git master」**被证伪**：它钉死的是上游 ghostty 的某个具体 commit，落后 master **7 周**，且两边版本号完全相同（都是 `1.3.2-dev`），从版本号上根本看不出落后。
2. `DESIGN.md:70`「GPU Vulkan 渲染（无 CPU/OpenGL 回退）」**部分不成立**：运行时确实只走 Vulkan，但 GLES 后端被编译进产物，`.so` 里能搜到 `libEGL.so.1` 与 `wgpu_hal::gles::egl`。
3. `REFERENCE.md:4`「用 `termux-exec` 做 `LD_PRELOAD` 重定向」**完全是编造的**：termux-app 仓库里既没有 `termux-exec`，也没有任何 `LD_PRELOAD` 字样。而这条反例恰好与 `DESIGN.md:141`「不得设置 `LD_PRELOAD`」自相矛盾。

## C 类：REFERENCE.md 与真实参考项目不符

以下每条都经我本人打开参考源码复核过。

### C-P1

#### C1. `REFERENCE.md:3`「`TextSelectionCursorController` 锚定」— termux-app 没有锚点模型

- 类确实存在：`termux-app/terminal-view/src/main/java/com/termux/view/textselection/TextSelectionCursorController.java:21`
- 但内部只有四个坐标 `mSelX1, mSelX2, mSelY1, mSelY2`（`:30`），没有 anchor 对象，也**没有越过时的归属权交换**；在该包内 `grep -rn 'anchor\|Anchor'` 零命中
- termux 的实际模型是「起点固定 + 终点移动」：`mSelX1/mSelY1` 为静态起点，`mSelX2/mSelY2` 为拖动端；坐标原样传给模拟器，由后者处理反向选择——`TextSelectionCursorController.java:362-371` `getSelectors` 不排序，`:374-376` `getSelectedText` 直接透传
- 唯一带「锚」语义的动作是**抓取起点手柄时把起点重锚到原终点**：同文件 `:240` `mSelX1 = mSelX2;`（拖拽分支内 `:259` 起）
- 唯一带钳制性质的只有宽字符吸附 `getValidCurX`（`:307-336`），与手柄交叉无关
- 对比：termlib 的 `applyHandleDrag` 才有真正的锚点交叉翻转（`termlib/lib/src/main/java/org/connectbot/terminal/Terminal.kt:2183-2217`，`:2170-2172` 注释：「the moving handle passes the anchor, ownership flips and the anchor is restored to its pre-cross position so it doesn't jump to the dragged handle's column」）
- 另需注意 `TextSelectionHandleView.java:236-241` 是**朝向翻转**（手柄越出 clip 时 `changeOrientation`），`:276-281` 是 `isPositionVisible()`，都不是选择钳制

`REFERENCE.md:3` 把 termlib 的特性记在 termux 名下，同时 `:9` 又把同一特性再记一次给 termlib。

#### C2. `REFERENCE.md:3`「手柄拖动状态机」— 没有状态机

termux 只有两个布尔加一个 `MotionEvent` 分派：`TextSelectionHandleView.java:30` `private boolean mIsDragging;`、`:291-319` 对 `ACTION_DOWN/MOVE/UP/CANCEL` 的 `switch`、控制器侧 `:26` `mIsSelectingText`。最接近状态机的是手柄朝向（`TextSelectionHandleView.java:88-113`，50 ms 节流的重新推导），也不是状态机。

#### C3. `REFERENCE.md:4`「用 `termux-exec` 做 `LD_PRELOAD` 重定向」— 编造

- `find -iname '*exec*'` 在 termux-app 只命中一个无关文件 `termux-shared/.../shell/command/ExecutionCommand.java`
- 全仓 `grep -rn 'LD_PRELOAD'`（java/kt/c/cpp/md）**零命中**
- 且 `DESIGN.md:141` 明写「不得设置 `LD_LIBRARY_PATH` `PWD` `LD_PRELOAD`」——这条反例同时与事实和本仓规范冲突

#### C4. `REFERENCE.md:4`「其 CPU 逐格 `Canvas.drawText`」— 说反了

termux 用的是**按 run 批量**绘制：`terminal-view/src/main/java/com/termux/view/TerminalRenderer.java:260` `canvas.drawTextRun(text, startCharIndex, runWidthChars, ...)`（API 23+），`:262` 退化为 `drawText(text, startCharIndex, runWidthChars, ...)`（按 run，不按格）。run 边界在样式/光标/选择变化处断开（`:137`），并有逐 run 的宽度失配检测（`:133-135`）触发 `canvas.scale()` 校正（`:217-223`）。

这条反例的真实教训应当是「**按 run 批量 + 逐 run 宽度校正**」，与文档写的「逐格」正好相反。

#### C5. `REFERENCE.md:5`「无行内关闭按钮、长按重命名」— 长按重命名存在，且结论前提不成立

- 无行内关闭按钮：**属实**。`app/src/main/res/layout/item_terminal_sessions_list.xml` 只有一个 `MaterialTextView`
- 长按重命名：**不属实**。`app/src/main/java/com/termux/app/terminal/TermuxSessionsListViewController.java:103-107`：

  ```java
  public boolean onItemLongClick(AdapterView<?> parent, View view, int position, long id) {
      final TermuxSession selectedSession = getItem(position);
      mActivity.getTermuxTerminalSessionClient().renameSession(selectedSession.getTerminalSession());
      return true;
  }

  ```

  另有键盘入口 `TermuxTerminalViewClient.java:265`、`:485`（ctrl+alt+m）

`REFERENCE.md:5` 据此得出「本项目保留关闭按钮（DESIGN 声明），不实现重命名」——「termux 无重命名」这个前提是错的。

#### C6. `REFERENCE.md:6`「PTY 摄取 feed 前 NUL 剥离走 `memchr` 快路径」— torvox 根本没这么实现

参考项目确实这么做：`ghostty-android-terminal/app/src/main/cpp/terminal_jni.c:325` `if (len > 0 && memchr(bytes, 0, (size_t)len))`，命中才 malloc+拷贝（`:326-333`），否则无拷贝直送（`:322-323`、`:335`）。

但 torvox：

- `memchr` **不是**直接依赖（`Cargo.toml`、`native/Cargo.toml` 都没有；`Cargo.lock` 里只是传递依赖）
- `grep -rn 'memchr' native/src` 零命中
- 三处剥离全是标量循环：`native/src/terminal/session.rs:346-349`（`to_vec` + `contains(&0)` + `retain(|&b| b != 0)`）、`native/src/terminal/ghostty_terminal/public_api.rs:148-152`、`:180-183`
- 更关键：torvox 有测试**断言 NUL 必须保留**——`native/src/terminal/ghostty_terminal/tests.rs:2032-2044` `kitty_graphics_stray_nul_still_stores`

即 torvox 与参考项目在同一处采取了**相反策略**，而文档把参考项目做法写成了 torvox 的做法。

#### C7. `REFERENCE.md:12`「`acquire` 五分支处理」— 分支数错、变体漏一个、且 `Occluded` 在 Android 不可达

wgpu 30.0.1 中**不存在 `AcquireError` 类型**（`grep -rn 'AcquireError' wgpu-30.0.1/src/` 零命中），也没有 `Surface::acquire`；实际 API 是 `Surface::get_current_texture`（`wgpu-30.0.1/src/api/surface.rs:148`）返回 `CurrentSurfaceTexture`，共 **7 个变体**（`wgpu-30.0.1/src/api/surface_texture.rs:48-80`）：

| 变体 | 行 | 说明 |
| --- | --- | --- |
| `Success(SurfaceTexture)` | `:50` | 成功 |
| `Suboptimal(SurfaceTexture)` | `:53` | 建议重新 configure |
| `Timeout` | `:57` | 跳过本帧 |
| `Occluded` | `:62` | 窗口被遮挡 |
| `Outdated` | `:66` | 需重新 configure |
| `Lost` | `:68` | 需重建 surface |
| `Validation` | `:78` | 文档完全没提 |

参考项目本身是 **4 个 match 臂**（`wgpu-in-app/app-surface/src/lib.rs:222-236`）：`Success｜Suboptimal` 直用、`Timeout｜Outdated｜Lost` 重新 configure 后**重试一次**、`Occluded => return None`、`Validation => panic!`。所以「五分支」既不是参考项目的臂数，也不是 wgpu 的变体数。

另：`Occluded` 在 Android/Vulkan 上**不可达**——唯一产出点是 Metal 后端（`wgpu-hal-30.0.1/src/metal/surface.rs:368`），Vulkan swapchain 在 `wgpu-hal-30.0.1/src/vulkan/swapchain/native.rs:489-493` 只映射 `TIMEOUT` 与 `ERROR_SURFACE_LOST_KHR`，`:617-618` 另一处只映射 `ERROR_OUT_OF_DATE_KHR` / `ERROR_SURFACE_LOST_KHR`——都不产生 `Occluded`。

#### C8. `REFERENCE.md:12`「JNI 会话句柄用全局注册表，不用 `Box::into_raw` 裸指针」— 参考项目做的恰好相反

- 参考项目：`wgpu-in-app/wgpu-in-app/src/ffi/android.rs:18` `Box::into_raw(Box::new(canvas)) as jlong`，`:24`/`:32` 经 `&mut *(obj as *mut WgpuCanvas)` 恢复，`:39` `Box::from_raw` 释放；iOS 侧 `:17` 同款。**没有任何全局注册表**
- torvox 自己才有：`native/src/android/ffi.rs:139` `static SESSION_REGISTRY: LazyLock<RwLock<HashMap<u64, SessionEntry>>>`（另有 `:319` 的 `REQUEST_REGISTRY`）

即 REFERENCE.md 把 torvox 已经做对的事记在了参考项目头上。风险很实际：若有人按文档「向参考项目看齐」，会把正确的注册表改回裸指针。

### C-P2

#### C9. `REFERENCE.md:3`「手柄 `PopupWindow` 加 300ms 防误关」— 300ms 不在 PopupWindow 上

- `PopupWindow` 属实：`TextSelectionHandleView.java:23`、`:69-70`、`:134`（`TYPE_APPLICATION_SUB_PANEL`）
- 300ms 防误关在**控制器的 `hide()`** 里：`TextSelectionCursorController.java:57-64`

  ```java
  public boolean hide() {
      if (!isActive()) return false;
      // prevent hide calls right after a show call ...
      if (System.currentTimeMillis() - mShowStartTime < 300) { return false; }

  ```

  PopupWindow 本身没有任何计时保护

#### C10. `REFERENCE.md:15`「BSD 四态机 `ArgumentTokenizer`」— 是 DrJava 派生，不是 BSD

`termux-kotlin-app/termux-shared/src/main/kotlin/com/termux/shared/shell/ArgumentTokenizer.kt:1-39` 是 DrJava / JavaPLT（Rice University）Apache-2.0 版权块。四态机本身属实（`:51-54` `NO_TOKEN_STATE=0 / NORMAL_TOKEN_STATE=1 / SINGLE_QUOTE_STATE=2 / DOUBLE_QUOTE_STATE=3`，`:84-88` 消费），但来源标注错误；且唯一调用方是 `AmSocketServer.kt`，与终端启动路径无关。

#### C11. `REFERENCE.md:15`「搜索高亮覆盖层」— termux-kotlin-app 没有任何终端内搜索

`terminal-view/` 与 `terminal-emulator/` 下 `grep -rli 'search'` 只命中 `WcWidth.kt`（无关子串）。全仓唯一的匹配高亮是命令面板的 `app/src/main/kotlin/com/termux/app/ui/compose/commandpalette/CommandPalette.kt:314`、`:331` `highlightMatches`，且**无匹配计数、无上一个/下一个、无当前匹配区分**，无法支撑 `DESIGN.md:216-220`。

#### C12. `REFERENCE.md:9` 反例「普通/PASSWORD 双输入模式切换」— termlib 无此开关，且该实现与 `DESIGN.md:178` 冲突

- termlib 里 `PASSWORD` 只出现在 `ImeInputView.kt:193-198`，且是把 `TYPE_TEXT_VARIATION_PASSWORD` 与 `TYPE_TEXT_VARIATION_VISIBLE_PASSWORD` **同时或上**（注释：`Keeps text visible (we handle display ourselves)`），目的是隐藏数字行键盘，**不是面向用户的开关**
- termlib 真实的模式开关是 3 态编辑器类型：`lib/src/main/java/org/connectbot/terminal/ImeShortcutInputMode.kt:20-29` `DISABLED / TYPE_NULL / FORCE_ASCII`，由是否按住 Ctrl/Alt 驱动（`ImeInputView.kt:178-201`）
- `DESIGN.md:178` 要求「支持全功能输入法（不限制输入法特性）」，而 termlib 的 `TYPE_NULL` + 门控 `TYPE_CLASS_TEXT` 正是被禁止的那类限制。结论方向没错，但标签写错了

#### C13. `REFERENCE.md:8` Haven 的 `cursorKeyAppMode` 来自手写 VT 状态机，与 `DESIGN.md:58` 冲突

- 字段与 DECCKM 解析属实：`Haven/feature/terminal/src/main/kotlin/sh/haven/feature/terminal/MouseModeTracker.kt:57-59`、`:66` `private const val DECCKM = 1`、`:148-150`
- SS3/CSI 编码分支属实：`TerminalScreen.kt:2441-2443`

  ```kotlin
  internal fun arrowKeyBytes(up: Boolean, appMode: Boolean): ByteArray {
      val prefix = if (appMode) "\u001bO" else "\u001b["

  ```

  另有 `:2450-2462` 的按住滑动版本
- 但 `MouseModeTracker` 是 **Haven 自己手写的 VT 解析器**（`State.GROUND/State.ESC` 状态机从原始 PTY 字节里解析）。`DESIGN.md:58` 规定「以 Ghostty 作为终端状态的单一来源，不重复实现 Ghostty 已有功能」。只能取编码分支，不能取解析器；REFERENCE.md 未做此区分

### C-P3

#### C14. `REFERENCE.md:7` `TerminalFontStore` 四槽 — 属实，但 torvox 有意偏离

四槽确认：`ghostty-android-terminal/app/src/main/java/io/github/sylirre/terminal/ui/TerminalFontStore.java:22-25` `DEFAULT=0 / ITALIC=1 / BOLD=2 / BOLD_ITALIC=3`。torvox 单槽是 `DESIGN.md:102`「只有一项主字体选择」的有意决定（`settings/SettingsRepository.kt:20`），且 REFERENCE.md 用的是「对照」措辞。**记为引用正确、非缺陷**，仅提示审查时不要误判。

## D 类：依赖源码事实错误

### D-P0

#### D16. `DESIGN.md:72`「上游 `libghostty-vt` / `libghostty-vt-sys` 固定跟踪 git master」— 证伪

`libghostty-vt-sys` 自己钉死了上游 ghostty 的 commit：

```rust
// libghostty-vt-sys/build.rs:6-7
const GHOSTTY_REPO: &str = "https://github.com/ghostty-org/ghostty.git";
const GHOSTTY_COMMIT: &str = "22d13172cde98a0a4dda05d3d6a3fcb0dd8ed018";

```

实测两者：

| 对象 | commit | 日期 | `build.zig.zon` 版本 |
| --- | --- | --- | --- |
| libghostty-vt-sys 钉死的上游 | `22d13172cde98a0a4dda05d3d6a3fcb0dd8ed018` | 2026-08-06 | `1.3.2-dev` |
| 上游 ghostty master HEAD | `6301810a48aaa3426887a4316668f18833a40138` | 2026-09-25 | `1.3.2-dev` |

**落后 7 周**，且两边版本字符串完全相同，从版本号上完全看不出落后。torvox 钉的 `libghostty-rs` rev `5988a0b7`（2026-09-01）已经是该 fork 的 master HEAD，所以「libghostty-rs 层在 master」成立，但「底层 ghostty 在 master」不成立。`REFERENCE.md:6` 提到该 fork 的 `ws_xpixel` 行为，说明 fork 落后未影响被引用的行为。

#### D18. `DESIGN.md:70`「GPU Vulkan 渲染（无 CPU/OpenGL 回退）」— 部分不成立，GLES 后端被编入产物

- 运行时确实只走 Vulkan：`native/src/render/wgpu_backend.rs:50` `let backends = wgpu::Backends::VULKAN;`，且 `force_fallback_adapter: false`；全仓无 `Backends::GL`、无 CPU device 路径
- 但 `native/Cargo.toml:34` 只写了 `wgpu = { workspace = true, features = ["vulkan"] }`，**没有关默认特性**。`cargo tree -e features` 显示 `gles`、`dx12`、`metal`、`webgpu` 全部启用，`wgpu-hal` 连带引入 `glow 0.17.0`、`khronos-egl 6.0.0`、`ndk-sys`
- 产物实测：`strings target/x86_64-linux-android/debug/libnative.so` 命中 `libEGL.so.1`、`wgpu_hal::gles::egl`、`failed to initialize EGL display`、`error in create_pbuffer_surface`
- 同时违反 `DESIGN.md:18`「最小体积，不做任何多余或不必要功能」

修法很小：`wgpu = { version = "30", default-features = false, features = ["std", "wgsl", "vulkan", "parking_lot"] }`。

### D-P1

#### D17. `BUILD.md:15`「若为动态链接，需将 `libghostty-vt.so` 复制到 `jniLibs/<abi>/`」— 该分支不可达

- `libghostty-vt-sys/Cargo.toml:9` `links = "ghostty-vt"`，`:15-16` `[features] default = ["vendored"]`，`:19` `link-dynamic = []` 需显式开启；`build.rs:21-28` `LinkMode::current()` 默认 Static，`build.rs:299` `println!("cargo:rustc-link-lib=static=ghostty-vt")`，动态分支在 `build.rs:280`
- torvox 未开启该 feature（`native/Cargo.toml:20` 只给 `libghostty-vt` 加了 `features = ["png"]`）
- 产物实测 `readelf -d target/x86_64-linux-android/debug/libnative.so` 的 `NEEDED` 只有 `libandroid.so`、`liblog.so`、`libdl.so`、`libm.so`、`libc.so`——**没有 `libghostty-vt.so`**
- `nm --defined-only | grep -c ghostty` = 1447（符号已静态内联）
- 全仓除 `Cargo.toml` 外无任何文件引用 `libghostty-vt.so`

`BUILD.md:15` 的静态/动态分支判断在当前配置下是死规则。这也解释了为什么 `scripts/build-android-libs.nu` 没有实现它——但规范里留着一条永远走不到的分支本身就是缺陷（对应 REVIEW.md 的 A10）。

#### D22. torvox 自己的注释与代码互相矛盾（且以参考项目为依据）

- `native/src/render/context.rs:388` 注释：「acquire failures (Timeout/Outdated/Lost) **must reconfigure + retry**」（引 zelland + wgpu-in-app）
- `native/src/render/context.rs:514` 注释：「the acquire path **already reconfigure+retries on Lost**」
- 实际代码 `native/src/render/pass.rs:130-140`：`Lost | Outdated => { surface.configure(...); None }`——**重新 configure 后直接返回 `None`，不重试**；`Timeout`/`Occluded`/`Validation` 全部落进 `_ => None` 静默丢弃
- `native/src/render/pass.rs:105-113` 有一处注释**正确地**记录了这个有意偏离（「Our worker thread handles Lost and Outdated inline … we treat a hung acquire as permanent」），但它与 `context.rs:388`、`:514` 的说法直接冲突

同一件事在三个注释里有两种说法。

#### D24. `REFERENCE.md:21`「spawn 前先应用初始 winsize（含像素字段）」— torvox 播种的是 0

- 参考项目在 `fork()` **之前** ioctl 写入真实像素：`ghostty-android-terminal/app/src/main/cpp/pty_jni.c:102-106` 设 `ws_xpixel = cols * cell_w`、`ws_ypixel = rows * cell_h`，`:113` 才 `fork`
- torvox `native/src/terminal/pty.rs:152-157` 播种 `ws_xpixel: 0, ws_ypixel: 0`；改为每次 grid resize 之后再推送（`ui/TerminalSurface.kt:463` → `bridge/Bridge.kt:193` → `NativeBridge.setPixelSize`）

即 spawn 到首次布局之间，子进程读到的是 0。规范与代码不一致。

### D-P2

#### D19. fontdb 无法升到 0.24 是永久性约束（不是遗漏）

crates.io 稀疏索引全部 **43 个** cosmic-text 版本中，fontdb 依赖要求为：

| cosmic-text 版本 | fontdb 要求 |
| --- | --- |
| 0.5.6 – 0.9.0 | `^0.10.0` – `^0.14.1` |
| 0.15.0 – 0.19.0（9 个，含最新 0.19.0） | `^0.23` |
| 任何版本 | **无一声称 `^0.24`** |

fontdb 0.24 相对 0.23 的实际差异（逐项 diff 确认）：

- 公共 API 集合**完全一致**（`pub fn/struct/enum/const/type/trait` 差集为空）
- `rust-version` 1.60 → 1.71
- 新增 `[features]` 块（`std` / `fs` / `memmap` / `fontconfig`）
- 移除 `ttf-parser = "0.25"` 依赖，改为内联私有模块 `src/ttf_parser/`（5 文件 1474 行）

所以升级阻塞**纯粹来自 cosmic-text 的版本声明**。REVIEW.md 的 A15 应据此把 `fontdb = "0.23"` 记为「现实强制的例外」，并在规范中显式记录，而不是当作「未更新」。

#### D20. 无任何依赖要求高于 1.98 的 rust-version — 通过

扫 `Cargo.lock` 全部 353 个包对应 manifest：**0 个**声明高于 1.98。最高者 `ordered-float 5.5.0 = 1.90`、`cosmic-text 0.19.0 = 1.89`、`smol_str 0.3.6 = 1.89`、`cucumber 0.23.0 = 1.88`；95 个未声明。git 依赖 `libghostty-vt(-sys)` 为 `rust-version = "1.90"`。`BUILD.md:22` 通过。

#### D21. `lru 0.18.5` 的 `Cargo.toml` 没有 `edition` 键

`lru-0.18.5/Cargo.toml:12-15` 只有 `rust-version` / `name` / `version`，按 edition **2015** 编译。`BUILD.md:22`「`edition` 最低为 2024」因此只约束本仓 crate，不约束依赖树——规范措辞需要限定范围，避免被读成「全树必须 edition 2024」。

#### D23. torvox 无 `ANativeWindow` 引用计数 RAII

- 参考项目：`wgpu-in-app/app-surface/src/android.rs:51` `a_native_window: Arc<Mutex<*mut ndk_sys::ANativeWindow>>`，`:90` `impl Drop for NativeWindow`，`:93` 调 `ANativeWindow_release`；`:64` 注释说明 `ANativeWindow_fromSurface` 会 `+1` 引用计数
- torvox：`native/src/render/context.rs:380-384` 注释「The caller guarantees `ptr` is a valid `ANativeWindow*` that stays alive … ownership is transferred to wgpu」，实际释放是手动 FFI 调用 `native/src/android/ffi.rs:116`（声明）与 `:1526-1528`（`unsafe { ANativeWindow_release(ptr) }`）。`impl Drop for Renderer`（`context.rs:274`）只丢 wgpu 资源，不负责窗口引用计数

`REFERENCE.md:12` 的「`ANativeWindow` 引用计数 RAII」这条 torvox 未实现。

#### D25. `nix 0.31.3` 是最新发布且 API 全部可用 — 通过

torvox 用到的 API 在 `nix-0.31.3` 中均存在：`pty::Winsize`（`src/pty.rs:4` `pub use libc::winsize as Winsize`）、`openpty`（`:256`）、`unistd::fork`（`src/unistd.rs:278`）、`ForkResult`（`:209`）、`setsid`（`:334`）、`getpid`（`:294`）、`tcgetpgrp`（`:359`）、`sys::wait::waitpid`（`src/sys/wait.rs:291`）、`FcntlArg`（`src/fcntl.rs:862`，门控 `:727`）。feature 门控与 `Cargo.toml:22` 声明的 `["term","process","signal","fs"]` 匹配（`src/lib.rs:169` `term` → `pty`，`:211` `process` → `fork`）。稀疏索引确认 0.31.3（2026-05-11）是最新发布，无更高版本或预发布。

#### D26. SGR 53 / SGR 58 双端确认 — 通过

- 上游 ghostty（vendored 在 `target/.../libghostty-vt-sys-*/out/ghostty-src/src/terminal/sgr.zig`）：`:385` `53 => return .overline`、`:388` `58 => if (slice.len >= 2) {`、`:215` `4, 38, 48, 58 => {}` 颜色参数列表
- torvox 映射：`native/src/terminal/ghostty_terminal/internal.rs:933` `data.overline = style.overline;`、`:927-928` 下划线颜色、`:936-944` 5 种下划线形状、`:1700-1702` `OVERLINE` 标志位、`:1706-1708` 双下划线
- 标志位布局 `types.rs:49` `OVERLINE = 6` → bit 64，断言 `types.rs:146`；着色器消费 `native/shaders/cell.wgsl:117`、`:126`
- 测试 `internal.rs:2305` `sgr53_overline_reaches_dumped_grid`、`:2275` `sgr58_underline_color_reaches_dumped_grid`、`:2290` 回退用例

`DESIGN.md:76` 通过。

### D-P3

#### D27. 除 fontdb 外，所有直接依赖的锁定版本都等于各自分号区间的最新发布 — 通过

逐个比对 crates.io `max_stable_version`：base64 0.23.1、bytemuck 1.25.2、cosmic-text 0.19.0、flume 0.12.0、futures 0.3.34、guillotiere 0.7.0、jni 0.22.4、log 0.4.34、lru 0.18.5、nix 0.31.3、png 0.18.1、raw-window-handle 0.6.2、serde 1.0.229、serde_json 1.0.151、swash 0.2.10、thiserror 2.0.21、wgpu / wgpu-types 30.0.1、criterion 0.8.2、cucumber 0.23.0、proptest 1.11.0、tokio 1.53.1、shuttle 0.9.4、foldhash 0.2.0、linkify 0.11.0、parking_lot 0.12.5、regex 1.13.1、roxmltree 0.21.1 全部等于锁定版本。

`libc` 锁定 0.2.189，最新稳定即 0.2.189（1.0.0-alpha.4 是预发布，按 `BUILD.md:23` 不采用）。

**唯一落后的是 fontdb 0.23.0（最新 0.24.0），原因见 D19，属现实强制。**

## F 类：规范引用缺失与空引用

torvox 声明了行为但 REFERENCE.md 没有给出对应证据，或给出的引用其实支撑不了该声明。

### F28（P2）`DESIGN.md:170-176` 的四项选择区要求，机制证据只被记成算式

- 「菜单始终不遮挡被选择文本」的真实机制是 `onGetContentRect` **加手柄高度偏移再加底部钳制**：`termux-app/.../TextSelectionCursorController.java:206-212`（`:200-204` 还会在 `x1 > x2` 时交换左右）。REFERENCE.md:3 只写了「等于列/行乘字体像素加 `TopRow` 滚动偏移」
- 「拖动时菜单隐藏」的真实出处是 `TerminalView.java:1507-1518` `updateFloatingToolbarVisibility`：MOVE 隐藏、UP/CANCEL 恢复，并按 `ViewConfiguration.getDoubleTapTimeout()`（`:1494`）延时。REFERENCE.md 完全未提
- 「长按文本」的词扩展是 `setInitialTextSelectionPosition`（`TextSelectionCursorController.java:93-107`），`DESIGN.md:170` 未提「词选择」
- 另有一处规范与实现的坐标系不一致值得注意：`TextSelectionCursorController.java:197` 用 `(mSelY1 - 1 - getTopRow()) * fontLineSpacing`，而实际定位手柄的 `TerminalView.getPointY`（`TerminalView.java:1062-1064`）用 `round((cy - mTopRow) * fontLineSpacing)`，两者差一行。REFERENCE.md 引用前者，torvox 应对齐后者

### F29（P3）`DESIGN.md:114-120` 修饰键栏全部要求 — REFERENCE.md 零引用

固定 2 行 7 列、粘滞键、左滑进入文本输入框、不与全面屏手势冲突，四项无任何参考项目引用。

### F30（P3）`DESIGN.md:122-125` Shell 启动入口参数拆分 — 引用不成立

唯一的 tokenizer 引用（`REFERENCE.md:15`）是 DrJava 派生的 `ArgumentTokenizer`，唯一调用方是 `AmSocketServer`，与终端启动无关。规范没有说明 torvox 在何处把参数串拆成 argv。

### F31（P3）termux-app 不存在 OSC 8 / 全选 / 打开文件 / 终端内搜索 / 脏跟踪 的任何对应物

对 termux-app 检索结果：OSC 8 超链接（`TerminalEmulator.java` 仅有 `:415` 一条关于下划线颜色的无关注释）、`selectAll`（零命中，动作栏只有 `TextSelectionCursorController.java:117-119` 的 COPY/PASTE/MORE）、打开文件、终端内搜索（`terminal-view`/`terminal-emulator` 零命中）、脏跟踪（`TerminalView.onDraw:1021-1037` 每帧全量重绘，无脏检查）。

这五项在 `DESIGN.md` 中写作「参考 Termux / 参考 ghostty-android-terminal」，但参考项目里没有对应物——属于空引用。

### F32（P3）gnome-console 的 `contains` 技巧是 VTE 专属

`gnome-console/src/kgx-tab.c:234-235` 用 `g_strrstr`（反向子串搜索）判定「搜索词被收窄」，理由写在 `:218-233`。但 `:218-219` 自述根因是「VTE doesn't automatically highlight the search match and doesn't have an API to do that」。torvox 以 Ghostty 为终端状态单一来源（`DESIGN.md:58`），这类 bug 大概率不会出现，引用应标注为「意图参考」而非可移植技术。

## 已核验通过的外部事实

| 断言 | 结论 | 证据 |
| --- | --- | --- |
| termux-app 宽字符吸附 | 通过 | `TextSelectionCursorController.java:307-336` `getValidCurX` 累加 `WcWidth.width` 落在宽字符中段时返回 `cend`（`:326-328`），双侧手柄均应用（`:261`、`:301`） |
| termux-app `WcWidth` 按 Unicode 15 判定 | 通过（仅 javadoc） | `terminal-emulator/.../WcWidth.java:3-4` `Implementation of wcwidth(3) for Unicode 15`；`:8-12` 有同步告警；`:514` 二分查表；`:536` 代理对入口。**树内无版本常量或生成脚本**，属不可完全验证 |
| termux-app `TextStyle` 64 位打包 | 通过 | `TextStyle.java:5-6` javadoc + `:9-13` 布局（16 标志位 + 24 fg + 24 bg）、`:51-56` `encode(int,int,int)`；`TerminalRow.java:49` `final long[] mStyle` |
| termux-app 选择文本 wrap 感知拼接 | 通过 | `TerminalBuffer.java:167-168` 条件换行；三模式 `joinBackLines`（`:121`）/`joinFullLines`（`:125`）/ `getTranscriptTextWithFullLinesJoined`（`:113`）；`:151-154` wrap 行保留尾随空格 |
| termux-app 列转 `char` 走宽字符换算 | 通过 | `TerminalRow.java:94-110` `findStartOfColumn`；`TerminalBuffer.java:144-147` 选中宽字符起点时 `findStartOfColumn(x2 + 1)` |
| termux-app 抽屉行格式 | 通过 | `TermuxSessionsListViewController.java:71` `"[" + (position+1) + "] "`、`:77` 粗体 span、`:78` 换行后斜体 span |
| termux-app 点击切换并关闭抽屉 | 通过 | 同文件 `:98` `setCurrentSession(...)`、`:99` `closeDrawers()` |
| ghostty-android-terminal 选择状态归模拟器 + `tapCount` | 通过 | `term/TerminalEmulator.java:193-196` `// --- Selection. The terminal owns it (tracked refs) ...`；`cpp/terminal_jni.c:1460`；`ui/TerminalView.java:1071` `tapCount = continues ? tapCount + 1 : 1;` |
| ghostty-android-terminal `Callback2` + `onGetContentRect` | 通过 | `ui/TerminalView.java:1438`、`:1484` |
| ghostty-android-terminal `selectionGeometryKey` | 通过 | `ui/TerminalView.java:1172`，12 位/坐标 + `Long.MIN_VALUE` 哨兵 |
| ghostty-android-terminal 边缘滚动 | 通过 | `ui/TerminalView.java:1267-1275` `scrollBy(±1)` |
| ghostty-android-terminal 搜索覆盖层不改尺寸 | 通过 | `ui/SearchBarView.java:35-37` 注释自述 no `SIGWINCH`；`:54` `DEBOUNCE_MS = 150`；`term/TerminalEmulator.java:249-250` 复用 selection 槽位 |
| ghostty-android-terminal `TerminalFontStore` 四槽 | 通过 | `ui/TerminalFontStore.java:22-25` |
| wgpu-in-app Android 单格式 | 通过 | `app-surface/src/lib.rs:339` `vec![format]`（Android 分支 `:329-339`） |
| wgpu-in-app 零尺寸钳制 | 通过 | `app-surface/src/lib.rs:71-73` `normalize_view_size` + `:451` 单测 |
| wgpu-in-app `acquire` 4 臂 | 通过（对照 C7） | `app-surface/src/lib.rs:222-236`：`:223-224` Success/Suboptimal、`:225-227` Timeout/Outdated/Lost、`:235` Occluded→None、`:236` Validation→panic! |
| wgpu-in-app `resize` 幂等 | 通过 | `app-surface/src/lib.rs:138-140` `if config.width == size.0 && config.height == size.1 { return false; }`；`:474` 单测 |
| wgpu-in-app `ANativeWindow` RAII | 通过 | `app-surface/src/android.rs:51` 字段、`:90-93` `impl Drop for NativeWindow` + `ANativeWindow_release` |
| wgpu-in-app `jni_fn` 宏反例 | 通过 | `wgpu-in-app/src/ffi/android.rs:6`、`:11`、`:22`、`:29`、`:37`；`Cargo.toml:40` 声明依赖 |
| zelland `PENDING_SIZE` | 通过 | `src-tauri/src/renderer/mod.rs:112` `static PENDING_SIZE: Lazy<Mutex<Option<(u32, u32)>>>`，写入 `:114-115`、消费 `:320-321`；`renderer/android.rs:90` |
| zelland 鼠标映射用实时 cell 尺寸 | 通过 | `src-tauri/src/terminal.rs:105-108` 注释；`renderer/mod.rs:1094-1097` |
| Haven SS3/CSI 编码分支 | 通过 | `TerminalScreen.kt:2441-2443`（`val prefix = if (appMode) "\u001bO" else "\u001b["`）、`:2450-2462`；测试 `SwipeArrowsTest.kt:39` |
| Haven Popup 内 `startActionMode(TYPE_FLOATING)` 静默 no-op | 通过（仅源码注释） | `FloatingTextInputDialog.kt:218-220` |
| termlib `applyHandleDrag` 交叉翻转 | 通过 | `Terminal.kt:2183` `internal fun applyHandleDrag(`，`:2170-2172` 注释；测试 `HandleDragTest.kt:57`、`:157`、`:199` |
| termlib 多行反向判定 | 通过 | `SelectionManager.kt:106-140`、`:474-482` 注释（`minOf/maxOf` 在「下且左」拖拽时与高亮不一致） |
| termlib `resize` 钳制选择 | 通过（需调用方主动调） | `SelectionManager.kt:375` `fun clampToDimensions(rows: Int, cols: Int)`（含 `:373-374` 用途注释），**未**被 `TerminalEmulator.resize` 调用 |
| termlib URL 尾随标点修剪含括号配对计数 | 通过 | `UrlDetection.kt:20-30` 混合策略 + `:32-43` `countOpenLessThanClose`；`:9` 朴素字符集 `.,;:!` |
| termlib `TerminalInputConnection` 组合键与 IME 显示控制 | 通过 | `ImeInputView.kt:293-296`、`:308-314`、`:325-339`、`:341-370`；修饰键掩码在 `KeyboardHandler.kt:92-94`、`:414-418` |
| zed-android-port `tcgetpgrp` 前台进程组 | 通过 | `crates/terminal/src/pty_info.rs:42-50` |
| zed-android-port 回退读 name/cwd/argv | 通过 | `pty_info.rs:33` `fallback_pid = pty.child().id()`、`:171-182` `load()`、`:82-86` `ProcessInfo`；注意 `get_child()`（`:137-140`）只被 `kill_child_process` 用 |
| zed-android-port `tcgetpgrp` 行号 | 通过 | `pty_info.rs:42-44` `let pid = unsafe { libc::tcgetpgrp(self.handle) };` |
| zed-android-port kill 先 `killpg` 再 `kill` | 通过 | `crates/terminal/src/terminal.rs:2281-2283` 注释与调用顺序；`pty_info.rs:142-149` `killpg(SIGKILL)`、`:155-156` `kill_child_process` |
| gnome-console 收窄用 `contains` | 通过 | `src/kgx-tab.c:234-235` `g_strrstr (priv->last_search, search)` |
| gnome-console `Copy` 无选择时置灰 | 通过 | `src/kgx-terminal.c:705-709` `gtk_widget_action_set_enabled(..., "term.copy", has_selection)` |

## 与第一阶段报告的关系

本报告与 [REVIEW.md](REVIEW.md) 的 41 项**不重叠计数**，但有三处需要合并修正：

| REVIEW.md 编号 | 原结论 | 本报告修正后 |
| --- | --- | --- |
| A5 + A15（fontdb） | 「fontdb 唯一落后，未更新」 | 修正为「现实强制的永久例外」，依据 D19：全部 43 个 cosmic-text 版本无一要求 `^0.24` |
| C14（`TerminalFontStore` 四槽） | — | 引用属实；torvox 单槽是 `DESIGN.md:102` 的有意决定，非缺陷，勿再列为问题 |
| A6（线程模型） | 与 `DESIGN.md:62` 不符 | 结论不变；补充 D23 说明 torvox 在 `ANativeWindow` 生命周期上也未达 `REFERENCE.md:12` 的标准 |
| A10（`BUILD.md:15/16/17` 未实现） | 脚本缺校验 | 补充 D17：`BUILD.md:15` 的动态链接分支本身在当前配置下不可达，规范该条需重写而非补脚本 |

合并后总计 **72 项**（41 + 31）。

## 建议处置顺序

1. **D16 / D18**：`DESIGN.md:70` 与 `:72` 是被证伪的明文架构声明。`:72` 需改为「钉死 `libghostty-vt-sys` 内声明的上游 ghostty commit，当前落后 master 7 周」；`:70` 需在 `native/Cargo.toml` 关掉 wgpu 默认特性后才是事实
2. **C3 / C4 / C5 / C8**（REFERENCE.md 编造与反向归因）：这四条会主动误导后续实现，尤其 C8 可能导致有人把正确的 `SESSION_REGISTRY` 改回 `Box::into_raw`
3. **C6 / C7 / D24**：涉及 torvox 自己的实现选择（`kitty_graphics_stray_nul_still_stores` 保留 NUL、acquire 不重试、winsize 播种 0 像素）——需先决定「跟随参考」还是「保留现状并写明理由」
4. **D22**：三个注释两种说法，就地修正注释即可
5. **C1 / C2 / C9 / C10 / C11 / C12 / C13 / C14**：REFERENCE.md 逐条订正
6. **F28–F32**：补齐或标注为空引用

## 复核方法

```text
# D16 ghostty 落后
sed -n '6,7p' ~/.cargo/git/checkouts/libghostty-rs-*/5988a0b/crates/libghostty-vt-sys/build.rs
git -C /tmp/refs/ghostty log -1 --format='%H %ci'

# D17 静态链接
readelf -d target/x86_64-linux-android/debug/libnative.so | grep NEEDED

# D18 GLES 编入
strings target/x86_64-linux-android/debug/libnative.so | grep -c 'libEGL'
grep -n 'wgpu =' native/Cargo.toml

# C4 termux-exec 不存在
find /tmp/refs/termux-app -iname '*exec*' -not -path '*/.git/*'
grep -rn 'LD_PRELOAD' /tmp/refs/termux-app --include='*.java'

# C5/C6/C10 termux 事实
grep -n 'drawTextRun' /tmp/refs/termux-app/terminal-view/src/main/java/com/termux/view/TerminalRenderer.java
sed -n '103,107p' /tmp/refs/termux-app/app/src/main/java/com/termux/app/terminal/TermuxSessionsListViewController.java
sed -n '57,64p' /tmp/refs/termux-app/terminal-view/src/main/java/com/termux/view/textselection/TextSelectionCursorController.java

# C8 wgpu 变体
grep -n 'pub enum CurrentSurfaceTexture' -A 45 ~/.cargo/registry/src/index.crates.io-*/wgpu-30.0.1/src/api/surface_texture.rs

# C8 参考项目用裸指针
sed -n '16,24p' /tmp/refs/wgpu-in-app/wgpu-in-app/src/ffi/android.rs

# C11 DrJava 来源
sed -n '1,6p' /tmp/refs/termux-kotlin-app/termux-shared/src/main/kotlin/com/termux/shared/shell/ArgumentTokenizer.kt

# C12 无终端内搜索
grep -rli 'search' /tmp/refs/termux-kotlin-app/terminal-view/src/main /tmp/refs/termux-kotlin-app/terminal-emulator/src/main

# D19 cosmic-text 全部版本
curl -s https://index.crates.io/co/sm/cosmic-text | python3 -c "
import sys, json
for l in sys.stdin:
    d = json.loads(l)
    fd = [x['req'] for x in d.get('deps', []) if x['name'] == 'fontdb']
    print(d['vers'], fd)
" | tail -10

# D20 rust-version 上限
grep -rh 'rust-version' ~/.cargo/registry/src/index.crates.io-*/*/Cargo.toml | sort -u | tail -5

# D22 注释与代码矛盾
sed -n '386,390p' native/src/render/context.rs
sed -n '512,515p' native/src/render/context.rs
sed -n '130,141p' native/src/render/pass.rs

```
