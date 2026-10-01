# 第 11 轮全面审查（缓存/脏跟踪机制 + UI 与安装器状态机）

审查日期：2026-09-30
基线提交：`c7d57c2`
编号说明：本轮最初以 `119f5e4` 为基线起草，撰写期间仓库被并发推进
（`21b7394`、`c6d78b8`、`c7d57c2`），且 `c7d57c2` 已占用 `docs/REVIEW-ROUND10.md`。
本轮文档顺延为第 11 轮，`REVIEW-ROUND10.md` 已按 `c7d57c2` 原样恢复。
全部结论已按新基线 `c7d57c2` 复核：`native/**` 未被并发提交触及，
受影响的唯一 Kotlin 文件 `ui/TerminalScreen.kt` 的 N2-29 已在 `:81` / `:721` 重新核对，行号与结论不变。
方法：第 6–9 轮已覆盖 JNI 编排、上游进程/表面生命周期、吞错点与锁、FFI 契约。本轮换两个新维度：
(1) **VT 适配层的缓存/脏跟踪机制**（`row_cache`、`RenderState` 生命周期、快照缓存），
(2) **Kotlin UI 与安装器/文档提供器的状态机与持久化**。
关键结论逐条回读源码核实，其中行缓存一项用可执行实验证伪/证实。
本轮只审查，**未改动任何源码**（临时探针已删除，`git status` 干净）。

---

## 一、本轮的两个决定性发现

### N0-13 `row_cache` 的读路径在生产中**永不命中**——整条 zelland 行缓存是纯开销

`native/src/terminal/ghostty_terminal/internal.rs:1466` 每次 `build_cell_data` 都新建迭代器：

```rust
let (mut render_state, mut row_iter, mut cell_iter) = create_render_iterators()?;
```

`create_render_iterators`（`internal.rs:72-99`）每次调用 `RenderState::new()` / `RowIterator::new()` /
`CellIterator::new()`。而缓存的读路径要求 `!is_dirty`（`internal.rs:1505-1507`）：

```rust
if !is_dirty
    && !selection_active
    && let Some(cached) = row_cache.get(row_idx)
{
    data.extend_from_slice(cached);
```

**行脏标志只存在于 `RenderState` 实例内。** 上游 `render.zig:1045` 在 `RowBuilder.row()` 里写
`b.dirties[vy] = true`，`render.zig:603-612`（`page_row.dirty = false`）在**同一次** `update()` 内消费它。
新建的 `RenderState` 走 `render.zig:148` 的 `empty` 初值（`rows=0, cols=0`），`beginUpdate` 因此在
`render.zig:385-390` 命中「维度变化 → 全量重建」分支，**每一行都被标脏**。

**实验证据**（探针已删除）：对同一 `Terminal` 连续构造全新 `RenderState` 并统计 `row.dirty()`：

```text
== 每帧新建 RenderState（生产实际） ==
fresh frame#1                rows=80   dirty=80   clean=0
fresh frame#2                rows=80   dirty=80   clean=0
fresh frame#3                rows=80   dirty=80   clean=0
== 复用同一 RenderState ==
reused update#2 (无输出变化)      rows=80   dirty=80   clean=0
```

连复用同一实例也是 `clean=0` —— 因为 `RenderState::empty` 的 `rows=0` 每次都触发全量重建，
只有第二次 `update` 之后维度才被填上。上游文档 `render.rs:45-51` 明确写着
「The user of the render state API is expected to unset both of these」，
本项目从未调用 `row.set_dirty(false)`（`render.rs:610`）。

**三重代价**：

1. **缓存零收益**：每帧仍走完整逐单元 FFI 路径（`internal.rs:1514-1651`），
   即 `row_cache` 声称要省掉的 `N` 次 FFI 往返一次都没省。
2. **缓存纯成本**：写路径 `internal.rs:1656` 每行 `row_data.clone()`，
   `CellData` 为 96 字节（`types.rs:66-85`，测试 `cell_data_size` 断言），
   80 列 × 24 行 = **184 KB 逐帧深拷贝 + 184 KB 逐帧分配**，全部丢弃。
3. **测试是假阳性**：`tests.rs:1610 row_cache_returns_consistent_cell_data_across_writes` 与
   `tests.rs:1651 row_cache_invalidated_on_resize` **在缓存永不命中的情况下依然通过** ——
   它们断言的是「输出一致」，而非「缓存被命中」。这违反 `TESTING.md:8`
   「每个测试必须断言具体行为」。

**与规范的关系**：`internal.rs:1444` 的注释「The cache is invalidated by the caller on resize」只覆盖
失效面，完全没有意识到**命中面**从未存在；`internal.rs:1503` 与 `internal.rs:1652` 两处
「zelland row-cache pattern」注释描述的是一个未生效的机制。

**修法**（二选一，倾向后者）：

- **删除**：`row_cache` 全部读写点 + `RowBuilder` 相关注释 + 两个名不副实的测试。
  符合 `STYLE.md:63`「不得保留死代码」与 `STYLE.md:64`「代码量不得超过 ghostty-android-terminal + termux」。
- **修复**：把 `RenderState`/`RowIterator`/`CellIterator` 提到 VT 线程主循环里**跨帧复用**
  （`internal.rs:725` 的 `while` 循环之外），并在消费完 `row.dirty()` 后调用
  `row.set_dirty(false)`（`render.rs:610`），最后在 `RowIterator` 侧对应清除。
  这样才真正兑现 `render.rs:26-31` 承诺的增量更新。

> 注意：本项若选「修复」，必须同时处理 `RenderState` 的 `snapshot` 借用生命周期
> （`render.rs:38-40` 要求 `RowIteration` 的借用不越过 `Snapshot`），
> 属于非平凡改动，**需要用户确认方向后再动手**。

### N0-14 `push_cell_data` 的空闲去重使**行级滚动完全无画面反馈**

同一函数的去重逻辑（`internal.rs:1222-1232`）：

```rust
let unchanged = match last_push {
    Some((last_cells, last_cursor)) => {
        *last_cursor == data.1
            && bytemuck::cast_slice::<CellData, u8>(last_cells)
                == bytemuck::cast_slice::<CellData, u8>(&data.0)
    }
    None => false,
};
if unchanged {
    return;
}
```

**实验证据**（探针已删除）：4×20 网格写入 8 行后 `scroll_viewport(3)` + `flush()`，
`receive_cell_data()` **3 秒内无任何帧**：

```text
  [base] cells=80 scrollback=5 cursor=(3,0)
    row0 = "L5                  "
  [scrolled] NO FRAME (deduped as unchanged)
```

`ffi.rs:3176` 的实参是 `scroll_viewport(-(delta as isize))`，正 delta 向下滚到底部；
此时视口已在底部、网格内容不变，于是**逐字节完全相同** → 被判为 `unchanged` → 不推送。
`ffi.rs:3193-3197` 的 `render_state.dirty.store(true)` 只能解锁 Idle 门控，
但 Idle 帧读的是**上一次缓存的 CellData**（`ffi.rs:1518` 附近的 `FrameData::Idle`），
画出来仍是滚动前的画面。

**与规范的关系**：`ffi.rs:3190-3192` 的注释声称「行级滚动必须立即重绘……否则会吞掉本次滑动」，
但去重把这条补救路径也一并吞掉。`DESIGN.md:184`（支持按像素流畅滚动）、
`openspec/specs/scroll-physics-drift/spec.md` 均要求滚动有可见反馈。

**修法**：`last_push` 的去重键必须纳入视口偏移。当前 `CursorInfo`（`types.rs:48-59`）
带 `scrollback_length` 但**不带视口偏移**，两者在「向底部滚且已在底部」时相同。
最小改法：把 `entry.last_scroll_offset` 或等价的 `viewport_offset` 字段并入比较键；
或对 `Command::ScrollViewport` 直接跳过 `unchanged` 判定（滚动是显式用户动作，
本就该强制推帧）。

---

## 二、新的 P1

> 维护注：N1-21（主题应用后台调度）、N1-22（多击计数复位）、N1-24（表面销毁清尺寸）、N2-27（清除缓存递归）、N2-28（安装包残留删除）已修复并验证，对应小节/行删除；其余编号保持不变。

### N1-20 `pollEvent` 仍在注册表读锁 + 会话锁内执行 5 秒有界 `flush()`

第 9 轮 N0-4 的**残留**。`ffi.rs:1122-1169`：

```rust
let registry = rlock_session_registry();
if active_id != 0 && let Some(entry) = registry.get(&active_id) {
    let mut session = entry.session.lock();
    session.process_output();          // → session.rs:507 / :517 self.terminal.flush()
```

`flush()`（`public_api.rs:190-205`）是 `rx.recv_timeout(Duration::from_secs(5))`，
`FLUSH_TIMEOUT_SECS = 5`（`types.rs:237`）。后台会话分支（`ffi.rs:1151-1167`）同样在锁内调用
`poll_pty_output` → `flush()`。

**后果**：VT 线程卡死时，`destroySession` / `initSession` 的写锁（`ffi.rs:217` `wlock_session_registry`）
被阻塞 `5s × (1 + 后台会话数)`。`session.rs:499-500` 的注释「限制每帧处理量，避免单次渲染调用
长时间持有会话锁」只限制了**块数**，没有限制**flush 的等待上限**。

第 9 轮已修好的是**退出码**那一半（`ffi.rs:1202-1211` 在锁外读码），这一半未动。

**修法**：`flush()` 旁加 `flush_with_timeout(Duration) -> bool`，
`pollEvent` 路径用零/极短期限；返回 false 时跳过 `drain_callback_events` /
`drain_pty_write_back`（`session.rs:508-511`）。

### N1-23 `BootstrapDownloader` 拒绝 `http://`，与 `DESIGN.md:126` 字面冲突

`BootstrapDownloader.kt:47-51`：

```kotlin
if (!url.startsWith("https://", ignoreCase = true)) {
    return@withContext Result.failure(
        Exception("Bootstrap URL must be https (got non-https URL)"),
    )
}
```

`docs/specification/DESIGN.md:126` 原文：「**Bootstrap**：支持 HTTP/HTTPS URL 与本地文件安装」。

代码侧的判断写在 `BootstrapDownloader.kt:44-46` 的注释里（刻意为之），
但 `docs/specification/` 是**最高权威**，且 `AGENTS.md` 明确规定「用户提示和文档中使用的
『代码』一词不包括『注释』」——注释不是授权来源。

**修法**（需用户裁决，见第五节 Q1）：放宽 `:47` 的 scheme 门禁，
只保留 `:58-65` 的重定向后最终 scheme 校验（那一条防的是 https→http 降级，有独立价值）；
或修改 `DESIGN.md:126` 为「仅 HTTPS」（需授权改保护文件）。

---

## 三、新的 P2

| 编号 | 位置 | 问题 |
| --- | --- | --- |
| N2-25 | `ffi.rs:682` + `session.rs:408` | `ResizeOutcome::Dropped` 在 JNI 边界被静默丢弃（`resize_inner` 只 match `Err`）。`session.rs:396-404` 的 `grid_dirty` 重试使它在**下一次** resize 事件自愈，但若不再有 resize 事件（如旋转后输入法再也不弹出），PTY 停留在新尺寸而网格与渲染器停留在旧尺寸，且无任何日志。 |
| N2-26 | `session.rs:128-133` + `ffi.rs:1520` | `Session::grid_size()` 只记录 Rust 侧发起的 resize。渲染器用它来排布 `Vec<CellData>`，而 `CursorInfo`（`types.rs:48-59`）不带 `rows`/`cols`。`CSI ?3h`（DECCOLM）会在上游内部改网格且不发 `Command::Resize`，届时 `CellData.col` 可达 159 而渲染器按 80 列排布。 |
| N2-29 | `TerminalScreen.kt:81` vs `internal.rs:2109` | UI 允许 256 字符查询（`SEARCH_QUERY_MAX_LENGTH = 256`），原生上限是 `MAX_SEARCH_QUERY_CHARS: usize = 128`；129–256 字符的查询被 `compile_search_pattern` 静默返回 `None` → 空结果，用户看到「无匹配」而实际有匹配。两侧常量必须同源。 |
| N2-30 | `ffi.rs:2840` vs `NativeBridge.kt:243` | 注释/文档矛盾**仍未对齐**：Rust 侧 `ffi.rs:2840` 写「`None` 清除覆盖（跟随终端）」，Kotlin 侧 `NativeBridge.kt:243` 仍写「`0xFFFFFFFF` 哨兵值表示清除覆盖」。实现（`ffi.rs:2853` 无条件写 `Some(...)`）**没有清除路径**。第 9 轮 N1-19 只改了一侧。当前无 Kotlin 调用方依赖清除（`Bridge.setTheme` 总是显式下发颜色），故为文档级残留，但下一位读者会被误导。 |
| N2-31 | `TerminalViewModel.kt:698-702` | `bridge?.listFontFamilies().orEmpty()` 在 bridge 缺席时伪造「空字体库」，随后 `SystemFonts.availableFontFamilies`（`SystemFonts.kt:9-12`）抛 `IllegalStateException`，被 `:712-717` 记录后**重新抛出**导致进程终止。把 `NativeQueryPort` 的「null = 无数据，绝不可伪造」契约变成了崩溃。 |
| N2-32 | `native/src/terminal/ghostty_terminal/keymap.rs`（261 行） | 整条上游 key 编码链在生产中不可达：`map_android_key_code` 只被 `internal.rs:299` 的 `Query::KeyEncode` 引用，而 `public_api.rs:344`/`:387` 的 `key_encode`/`key_encode_submit` 无非测试调用方；`NativeBridge.kt` 也没有任何 `keyCode` 入参的导出（唯一按键入口是 `writeKey(sessionId, key: String, mods, text)`，`ffi.rs:901-945`）。但 `ffi.rs:940-942` 的注释声称「完整 Kitty 键盘协议编码由上游 key::Encoder 经 `Query::KeyEncode` 承担」——**注释与事实相反**。违反 `STYLE.md:63`。 |
| N2-33 | `public_api.rs:284-297` + `internal.rs:1740-1899` | 快照缓存（`Command::TakeSnapshot`、`build_snapshot`、`GridSnapshot`、`CellSnapshot`、`cached_snapshot`、`snapshot_needs_rebuild`）在生产中不可达：`.take_snapshot()` 只出现在测试文件；渲染走 CellData 通道。`public_api.rs:288`/`:294` 还是 `panic!`。`snapshot_cache_unit_tests.rs`（18 行）只为测试这段死代码而存在。 |
| N2-34 | `session.rs:446 send_signal`、`public_api.rs:481 read_visible_text` | 二者均无非测试生产调用方。且 `ReadVisibleText` 会整行丢弃纯空白行（`internal.rs:207`），输出丢失行结构。 |
| N2-35 | `ffi.rs:1674` → `pass.rs:438-477` + `cell_builder.rs:947-1004` | 整个纵向滚动 blit 是死代码：生产者硬编码 `let scroll_up_rows: Option<u32> = None;`，`detect_vertical_shift` 是 `#[cfg(test)] pub fn`。且 `pass.rs:447-473` 的 `copy_texture_to_texture` 存在**重叠区域**，而 wgpu 只校验 Z 层不相交（`wgpu-core/src/command/transfer.rs` `validate_copy_within_same_texture`），正确性依赖 API 不提供的顺序保证。 |
| N2-36 | `pipeline.rs:229-256` | `ensure_kgp_pipeline` 每帧新建 `TextureView` **和** `BindGroup`（`context.rs:196` 每帧调用），而同函数上方的 sampler/pipeline/buffer 都有 `is_none()` 守卫。只有「屏幕上有图片」时才走这条路，即稳态分配抖动。 |
| N2-37 | `context.rs:582` | `kgp_atlas_data` 是 KGP 图集的**永久 CPU 全量副本**，全仓仅 3 处出现（声明 `:119`、初始化 `:308`、写入 `:582`），从不读取。对 `kitty.rs:63` 文档化的 64 MiB 存储预算而言等于内存翻倍。违反 `DESIGN.md:20`（低内存友好）。 |
| N2-38 | `font/pipeline.rs:674` vs `:781` | `cjk_glyph_cache` 的**写**是无条件的，**读**却被 `code_point >= CJK_IDEOGRAPHIC_START`（0x2E80）门控。U+2E80 以下的符号字形（▶ ★ ♥ ✓ ✗ 等）每次都重跑整条回退链 + 全字体库扫描（`cjk.rs:116-131`），且 `cell_builder.rs:550-561` 的 `warm_frame_glyphs` 每帧第二次调用。`font/mod.rs:1069` 的测试只断言缓存**含有**该条目，从不断言被命中。 |
| N2-39 | `android/ffi.rs:399-403` | `initSession` 把空 shell 入口改写为 `/system/bin/sh`，这是 `DESIGN.md:188`「失败不回退，无其他任何回退」之外的**额外回退**。当前 Kotlin 侧已实现 bash→login→`/system/bin/sh` 的解析顺序（`TerminalRuntime.kt:1682`、`Bridge.kt:118`），该分支不可达，但它会掩盖 Kotlin 侧回归。 |
| N2-40 | `pty.rs:311-316`、`:337-343` | 子进程 `setsid` 失败 → `_exit(2)`、`TIOCSCTTY` 失败 → `_exit(3)`，两者**不写任何 PTY 输出**（对比 `execve` 路径会写 `execve failed: errno=N`，`:394-425`）。由这两者引起的启动失败会表现为 `[Process completed (code 3)]`，而非 `DESIGN.md:194` 要求的「保留输出显示」。 |
| N2-41 | `pty.rs:128` | `openpty` 的两个 fd 未设 `FD_CLOEXEC`（`nix-0.31.3/src/pty.rs:274` 直接包 `OwnedFd`）。当前不可利用（`pty.rs:676-708` `close_stray_fds` 关闭子进程中所有 ≥3 的 fd，且 `fork()` 是全仓唯一 fork 点），但一旦新增任何 spawn 路径即成 fd 泄漏。 |
| N2-42 | `TerminalRuntime.kt:1854`、`:1894` | 生产代码读 `System.getProperty("test.minSurface")` / `("test.bootstrapUrl")`。ART 从不填充 `java.util.Properties`，二者恒为 `null`；若曾生效则静默绕过最小 surface 守卫与引导 URL。`STYLE.md:63`（死代码）+ `STYLE.md:65`（未声明功能）。 |
| N2-43 | `TerminalViewModel.kt:1074`（及 `:782`、`:1338`、`:1366`、`:1400`、`:1450`） | 六处 `catch (exception: Exception)` 吞掉 `CancellationException`，破坏结构化并发。对照 `TerminalRuntime.kt:2051`、`:2191`、`:2329`、`:2460` 显式 `if (exception is CancellationException) throw exception`。 |
| N2-44 | `ModifierBar.kt:99-101`、`:502-517` | `DESIGN.md:152` 要求「修饰键栏的 `->`/`<-` 将可见区域左右移动」，但四个方向键全部经 `toolbarKeyClickHandler` 发转义序列（`ESC [ C` / `ESC [ D`）。全仓检索无任何横向视口偏移（`scrollToRow`/`currentViewportTopGrid` 均为纯行向）。 |
| N2-45 | `internal.rs:420` | `Query::EncodeMouseEvent` 无 modifier 字段，`mouse_event.set_mods(...)`（`mouse.rs:261`）从未调用，事件保持 `mouse::Event::new()` 的零值。Ghostty 把 `mods.shift`/`mods.alt` 折进按键字节，故 Shift 点击与 Ctrl+点击到达 vim/tmux/htop 时与普通左键不可区分。违反 `DESIGN.md:182`。 |
| N2-46 | `cell.wgsl:129-131` | SGR 2（dim，flag bit 7 → `128`）乘在**最终**颜色上，即背景与 SGR 58 下划线/上划线色也被减半，而 Ghostty 只减暗前景。着色器在 `:116-127` 的 deco pass 之后才应用，此时背景与装饰色已写入 `color`。 |

---

## 四、本轮的正面结论（经核实为健康，记录以免重复怀疑）

1. **WGSL ↔ Rust 实例布局逐字节吻合**：`CellInstance` 96 字节、偏移 0/8/16/24/40/56/72/80/84/92，
   `mod.rs:88-99` 的 location 1,2,3,4,5,10,6,7,8,9 与 `cell.wgsl:29-39` 完全对应；
   `KittyGraphicsInstance` 40 字节含必需的 `_padding`@36。
2. **uniform buffer 无下溢**：`GpuUniforms` 80 字节 = WGSL `Uniforms`（mat4@0、vec2@64、f32@72、f32@76，
   对齐 16 → 80），两处 bind group 均按 `size_of::<GpuUniforms>()` 精确分配
   （`context.rs:753`、`pipeline.rs:210`）。
3. **`write_texture` 的非 256 对齐 `offset` 合法**：wgpu-core 30.0.1 的
   `Queue::write_texture` 以 `aligned = false` 走 `validate_texture_buffer_copy`。
4. **图集 generation 与缓存 UV 一致**：`atlas_generation` 仅在驱逐（`atlas.rs:171`）与重建
   （`:308`）时递增，普通分配不递增；`cell_builder.rs:473-522` 的双遍循环在 generation 中途变动时
   退化为全量重建。驱逐不可能让在途帧采样到已搬迁的 rect。
5. **`RenderState` 每帧新建不构成泄漏**：`render.rs:368` / `:555` / `:676` 三个 `Drop` 分别
   `free` 掉 state / row iterator / cell iterator。
6. **KGP 载荷越界不可能 panic**：`kitty.rs:120-132` 用 `saturating_sub` 钳位源矩形，
   `:151-164` 的 `copy_sub_rect` 用 `get`/`get_mut` + `break`；`frame.image_rgba` 长度从不被信任。
7. **脏跟踪的撤销路径完整**：`collect_highlight_rows`（`ffi.rs:1428-1435`）并入**当前与上次**的高亮行，
   故移除高亮会重绘该行。
8. **Surface 获取不会阻塞渲染线程**：`pass.rs:115-167` 在专用 worker 上以 2s 超时 + 非阻塞
   `try_send` 获取纹理；`Lost`/`Outdated` 走 reconfigure 并丢帧。
9. **`CellData` 是 `Pod` 且 96 字节**（`types.rs:87-94` 双测试锁定），逐字节 memcmp 去重安全。
10. **`jni_export_guard!` 的 panic 语义健全**（第 9 轮结论复核仍成立）：jni 0.22.4
    `with_env`（`src/env.rs:4801`）用 `catch_unwind` 包裹，`Outcome::Panic` 转 Java 异常，
    没有 panic 能跨 `extern "system"` 边界。
11. **第 9 轮 N1-14（`getTitle` 护栏）已修复**：`Bridge.kt:564`/`:566-567` 均已包
    `runCatchingCancellable`，`TerminalViewModel.kt:955` 的直调也已加护栏。
12. **N1-15（`scrollbackLine` 负行号）已修复**为返回 `null` 而非抛异常，与
    `NativeQueryPort` 的「无数据」通道对齐。
13. **输出通道冻结已修复**：`render_inner`（`ffi.rs:1504-1506`）在消费 CellData **之前**
    就对 `paused`/无 surface 早返回；`pollEvent` 无条件排空活动会话输出。
14. **环境变量白名单精确合规**：`pty.rs:711-779` 只注入 `DESIGN.md:131-140` 列出的变量
    - 13 个「宿主存在才透传」项，无 `LD_LIBRARY_PATH`/`PWD`/`LD_PRELOAD`/`PATH`，无 `termux.env`。
15. **OSC-52 扫描器无损**：`output_processor.rs:85-206` 的每个分支都重发缓冲字节，
    `MAX_SCAN_BYTES` 溢出回退到直通；`ESC`/`0x07` 在选择名内被拒绝，
    因此应答写回 PTY（`session.rs:592-596`）时不会注入分隔符。
16. **`pty_write` 的 LF→CRLF 幂等**：`public_api.rs:155-172` 的 `last_pty_write_byte`
    正确处理跨块的 `\r`/`\n` 拆分与 NUL 跳过。
17. **所有通道有界**：`OUTPUT_CHANNEL_BOUND=128`（阻塞发送 = 正确背压）、
    `COMMAND_CHANNEL_CAPACITY=1024`、`QUERY_CHANNEL_CAPACITY=256`、
    `CELL_DATA_CHANNEL_CAPACITY=4`、`EVENT_CHANNEL_CAPACITY=16`，
    命令/查询/单元数据全部 `try_send`，从不阻塞 VT 线程。
18. **zip slip 已封堵**：`BootstrapInstaller.kt:153-161` 在分发前拒绝绝对名与 `..` 段；
    已在真实 JDK 上验证 `new File(parent, "/etc/passwd)` 产生 `parent + "/etc/passwd"`（不逃逸）。
19. **`SYMLINKS.txt` 解析正确**：容忍 CRLF、过滤空行、相对目标经本地 `normalizePath()`
    （`:222-244`）归一后再查逃逸，绝对目标经 `canonicalPrefix` 约束。
20. **符号链接安全的删除**：`BootstrapInstaller.delete`（`:352-361`）与
    `DocumentMutations.deleteWithoutFollowingSymlinks`（`:186-208`）都以 `Files.isSymbolicLink`
    为递归闸门，可防自指环与树外目标。
21. **下载器防护到位**：逐跳与最终 scheme 校验、OkHttp 默认 20 跳上限、
    1 GiB 上限同时对 `Content-Length` 与运行时总量生效、读循环内检查 `isActive`。
22. **`SecondStageRunner` 无 shell 注入面**：`postinstCommand` 以 `File.zipPath` 构造 argv，
    **不存在 `sh -c`**；cwd 为 `/`，env 恰为 `DESIGN.md:131-138` 白名单；输出管道由 daemon 线程排空
    （无 64 KB 管道死锁）；每脚本 30s 超时 + `destroyForcibly()`。
23. **`TestBackdoorReceivers` 在 release 中惰性**：`MainActivity.kt:185-186`、`:268-269`
    以 `BuildConfig.DEBUG` 门控 `register()`/`unregister()`，且全部接收器用
    `RECEIVER_NOT_EXPORTED`。
24. **Manifest 与代码一致**：`BootstrapInstallService` / `TerminalForegroundService` 均
    `exported="false"`；5 个声明权限全部被使用且无过度声明；
    `allowBackup="false"` + `data_extraction_rules.xml` 全量排除。
25. **JNI 契约 57↔57 完全对称**（第 9 轮结论复核仍成立）。
26. **色空间自洽**：图集是 `Rgba8Unorm` 线性覆盖率掩码，交换链显式为非 sRGB 格式
    （`context.rs:423-431`），着色器在同一空间 `mix`。
27. **`cell.wgsl:104` 的非均匀 `textureSample` 不是违规**：`textureSample` 的结果被存入局部
    （`S::Store`），naga 的 `IMPLICIT_LEVEL` 要求只在 `S::Emit` 语句内报告；
    Lavapipe 上的管线创建测试也通过。

---

## 五、需要用户裁决的问题

1. **`DESIGN.md:126`（HTTP/HTTPS）vs `BootstrapDownloader.kt:47`（仅 HTTPS）** —— N1-23。
   改代码还是改规范？`docs/specification/` 是保护文件，改它需要明确授权。
2. **`row_cache` 的方向** —— N0-13。删除（诚实但放弃优化）还是修复
   （跨帧复用 `RenderState` + `set_dirty(false)`，兑现真正的增量更新）？
   后者是非平凡改动。
3. **`DESIGN.md:152`（修饰键栏左右移动可见区域）未实现** —— N2-44。
   需要新增渲染器/运行时的列偏移通道，是非平凡特性。确认是否本轮实现。
4. **`DESIGN.md:237`（启动时检查应用数据兼容性，可清除应用数据）未实现** ——
   `android/app/src/main` 全域检索 `compatib`/`数据兼容` 零命中。
   `clearAppData` 存在但启动时无人调用。是有意推迟还是缺失？
5. **`DESIGN.md:95`（设置出错「重置应用数据」）vs `clearFontFamily`（只删一个键）** ——
   `TerminalViewModel.kt:681-685`。较窄的行为实际更安全，确认是有意为之。
6. **`AnrWatchDog`（5s 主线程无响应即 `Process.killProcess`）与 `PROHIBITED.md:10`**
   （禁止会话持久化/恢复，即会话不可恢复）以及 `DESIGN.md:192`（shell 崩溃保留现场）
   三者不相容 —— 第 9 轮 D3 未决。每次触发销毁全部 shell 且不可恢复。
7. **`DESIGN.md:170`（打开链接「不重复实现链接识别」）vs `openspec/specs/text-selection/spec.md`**
   （URL 形态或 OSC 8 二者取一）。代码按 DESIGN 实现（仅 OSC 8）。确认 DESIGN 优先。
8. **全选后菜单可见性** —— `DESIGN.md:173`（全选后复制必须能正常工作）与
   `openspec/specs/text-selection/spec.md`（「两侧均无空间时 MUST 隐藏」）在选区填满视口时互斥。

---

## 六、修复顺序建议

1. **N0-13 `row_cache`**（先裁决方向）—— 一个机制同时是零收益优化 + 逐帧 368 KB 无用拷贝 + 两个假阳性测试。
2. **N0-14 滚动去重** —— 用户可见的功能缺失（滑动无反馈），几行可解。
3. **N1-20 `flush` 锁区** —— 加带期限的变体，关闭写锁饥饿。
4. **N2-29 搜索长度双常量**、
   **N2-30 文档矛盾** —— 各自几行。
5. **N2-32 / N2-33 / N2-34 / N2-35 死代码** —— 按 `STYLE.md:63` 应删除；需确认后再动。
6. 其余 N2 项。

---

## 七、收敛状态

- 本轮**不是**「无新问题」的一轮：新增 **2 个 P0、5 个 P1、22 个 P2**，外加 **27 组经核实的正面结论**。
- 「连续五次无新问题」计数**第一次归零**，从第 10 轮重新开始。
- 与第 6–9 轮一致的规律再次成立：**换一个审读维度就立刻在未被该维度覆盖的位置产出 P0**。
  第 6 轮换到上游进程/表面层，第 7 轮换到类别横切，第 8 轮换到锁与吞错，
  第 9 轮换到 FFI 契约，本轮换到**缓存/脏跟踪机制**与 **UI/安装器状态机** —— 立即产出 2 个 P0。
- 累计（第 6–10 轮）：**11 个 P0、33 个 P1、约 77 个 P2/P3**，外加 34+ 处死代码。
- **仍然建议：先修复再复审。** 理由同前：`row_cache` 一项就同时命中「零收益的复杂度」
  （`STYLE.md:63`）、「不必要的内存与拷贝」（`DESIGN.md:20`）、「测试是假阳性」
  （`TESTING.md:8`）三条规范，是全仓性价比最高的单点改动。
- 本轮任务限定「不实际修改代码」，上述内容以文档形式留存，等待授权后按第六节顺序执行。
