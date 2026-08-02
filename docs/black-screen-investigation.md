# 终端黑屏深度审查报告（ADR-0007 未实现）

日期：2026-08-02
方法：CI APK 下载实证 + 模拟器安装 + 逐像素截图 + logcat + 全仓库代码调查（2 路并行）
结论：**Android 终端内容区黑屏是确定性事实，根因是 ADR-0007（surface integration）从未实现——渲染栈内核完整但两端断路。**

---

## 1. 黑屏实证（不猜测）

### 1.1 APK 完整性（CI artifact `debug-apk`，23.7MB）

| 检查项 | 结果 |
|---|---|
| `lib/arm64-v8a/libnative.so` | ✅ 5.36MB |
| `lib/x86_64/libnative.so` | ✅ 5.90MB |
| `assets/bin/*/exec-bin` | ✅ 503KB / 476KB |
| 安装 `adb install -r` | ✅ Success |

### 1.2 启动日志（logcat，无任何错误）

```
I NativeBridge: Native library loaded: native
I native::android::ffi: NativeBridge::initLogger called
I native::terminal::session: Session::spawn: shell='/system/bin/sh', rows=24, cols=80
I native::terminal::session: Session::spawn: PtyPair::spawn OK
I native::android::ffi: FFI: initSession -> id=1
I native::terminal::session: wait thread: waiting for child pid=14270
```

无 FATAL / AndroidRuntime / UnsatisfiedLinkError。

### 1.3 UI 树（uiautomator）

Terminal 区域 `[0,128][1080,2337]`、ModifierBar（Key_ESC/TAB/CTRL/ALT...）全部存在 ✅

### 1.4 逐像素截图（1080×2400）

终端内容区（y=200/500/1000/1500/2000）**全部 = RGB(33,33,33) = #212121**（Compose 背景色，见 TerminalTheme.kt:27），**零文字渲染**。平均亮度 33.5。注意 #212121 是 Compose 背景而非 SurfaceView 默认纯黑 (0,0,0)——说明没有黑色 SurfaceView 遮挡，是终端内容区本身无任何绘制输出。

**结论：Compose UI 骨架正常（Kotlin 直接绘制），终端内容区黑屏（需要 wgpu 渲染到 ANativeWindow，该通路不存在）。**

---

## 2. 根因：ADR-0007 三处 JNI 层缺失

### 2.1 缺 JNI `render` 导出

- `ffi.rs` 17 个 `#[no_mangle]` 导出中**无 render**（仅 attachWindow/detachWindow 占位）
- `Bridge.kt:151-157` `render()` 是 stub：`return 0`，注释自认 "no native JNI export for render()"
- `TerminalRuntime.kt` 渲染线程每帧调 `bridge.render()` 拿 0 → 只跑 `pollAll()` 事件轮询

### 2.2 attachWindow 是占位

`ffi.rs:1180-1234`：
```rust
let ptr = unsafe { ANativeWindow_fromSurface(...) };
log::info!("FFI: attachWindow ptr={:p} {}x{}", ...);
// NOTE: Surface lifecycle integration is pending (ADR-0007)...
unsafe { ANativeWindow_release(ptr) };  // 拿完立即释放
```
- 不创建 wgpu surface、不发任何通道消息
- `detachWindow`（ffi.rs:1244-1262）：log-only no-op
- Kotlin 侧 `attachWindow` **从未被调用**（`TerminalSurface.kt:1942` 传 `windowPointer = 0L` 走 `updateNativeWindow` stub）

### 2.3 无 SurfaceCommand 通道

- ADR-0004 设计与 `tasks/fix-plan.md` 规划的 `flume::Sender<SurfaceCommand>` **全仓库不存在**
- `Renderer.surface: Option<Arc<wgpu::Surface>>`（context.rs:87）生产路径**永远 None**
- `Renderer::new()` / `global_gpu()` 生产代码**零调用者**（只有测试/截图用）

---

## 3. 渲染通路完整性（哪段通、哪段断）

| 段 | 状态 | 位置 |
|---|---|---|
| ① `build_cell_data`（VT 线程） | ✅ 完整 | internal.rs:800-956（spacer 跳过、wide、grapheme_extra、CursorInfo） |
| ② flume 通道发送 | ✅ 完整 | internal.rs:315-419（bounded(4)，超时自动推送） |
| ③ `receive_cell_data()` | ✅ 完整 | public_api.rs:419-426 |
| ④ `build_instances_from_cell_data → render_cell_data` | ❌ **零调用者** | cell_builder.rs / pass.rs:458-485 完整但无调用点 |
| ⑤ 帧呈现 | ❌ 断开 | Bridge.render() stub 返回 0 |
| ⑥ ANativeWindow → wgpu::Surface | ❌ **不存在** | 全库无 `create_surface` / `AndroidNdkWindowHandle` |
| ⑦ 字体/atlas 管线 | ✅ Android 就绪 | font_db.rs 扫描 /system/fonts + fonts.xml、cosmic-text+swash+guillotiere 完整 |

**一句话：render/ 内部 GPU 代码（wgpu 后端、Renderer、pass、cell_builder、字体/atlas）编译就绪且逻辑完整（CPU 实例构建基准 1362 fps + GPU 离屏提交测试 >30 fps 证明内核路径正确），但 Android 上"最后一公里"三件事全缺：① attachWindow 真交接 ANativeWindow ② 持有 Renderer+FontPipeline 的渲染线程 ③ receive_cell_data → render_cell_data 接线。**

---

## 4. 文档声称 vs 实际（系统性失实清单）

| # | 文档声称 | 实际 | 严重度 |
|---|---|---|---|
| 1 | project-health.md：功能/架构 "✅ Complete"，含 GPU render | Android 渲染未接线，终端区无输出 | 🔴 |
| 2 | architecture.md：`render/surface.rs` 管理 ANativeWindow；"ANativeWindow → Vulkan" 路径 | 文件不存在，无交接代码 | 🔴 |
| 3 | ADR-0007 Status Note："surface handoff uses `SurfaceCommand` queue" | 代码中无 SurfaceCommand 符号 | 🔴 |
| 4 | acceptance.md FR-018/FR-050 验收"通过"（surface 重建、attach/detach） | 仅 headless 测试，真实生命周期无验收 | 🔴 |
| 5 | traceability.yml FR-018/NFR-022 design → `render/surface.rs`、`jni_bridge.rs` | 两文件均不存在；verification 是无头 cargo 命令 | 🟠 |
| 6 | AGENTS.md Key Files：`render/surface.rs` | 文件不存在 | 🟠 |
| 7 | ADR-0003 Status Note："`jni_bridge.rs` handles ANativeWindow" | 文件不存在 | 🟠 |
| 8 | review-status.md "CellData fast path active" | 管线在测试可用，Android 未驱动 | 🟠 |
| 9 | srs.md FR-018/FR-050/NFR-022 "Automated Test 通过" | 设计路径失效 | 🟠 |
| 10 | 唯一准确披露：test-coverage-audit.md §2.4 "attachWindow 占位 stub" | ✅ 与代码一致 | — |

**为什么 maestro 22/22 全过却黑屏？** 所有 flow 只断言 UI 元素存在（TerminalScreen/ModifierBar/TerminalContent），截图无 OCR 终端文字断言。UI 骨架正常 ≠ 终端内容渲染正常。

---

## 5. 修复路径（实现 ADR-0007）

### 5.1 Rust 侧（核心）

1. **`attachWindow` 真交接**：ANativeWindow 指针经 `flume::Sender<SurfaceCommand>` 发给渲染线程；wgpu 用 `Surface::from_anativewindow`（wgpu 30 的 `AndroidNdkWindowHandle`）创建 surface + `request_adapter(compatible_surface)` + configure swapchain
2. **渲染线程**：持有 `Renderer + FontPipeline`，循环 `receive_cell_data() → build_instances_from_cell_data() → render_cell_data()`，事件驱动（notify + park）
3. **新 JNI `render` 导出**（或复用 pollEvent 驱动）：返回渲染状态
4. **`detachWindow`**：surface destroy 路径（暂停渲染、释放 swapchain/surface，保留下次重建）

> 注（review 修正）：wgpu 30 无 `Surface::from_anativewindow` 函数；正确 API 是
> `unsafe { instance.create_surface(wgpu::AndroidNdkWindowHandle { ptr: non_null, .. }) }`
> （raw-window-handle 0.6 的 `AndroidNdkWindowHandle`），并需先 `request_adapter(compatible_surface = Some(&surface))`。

### 5.2 Kotlin 侧

1. `Bridge.render()` 调真实 JNI render
2. `attachWindow(sessionId, surface, w, h)` 在 `onSurfaceTextureAvailable` 调用（当前传 0L stub）
3. `getCellWidth/Height` 返回真实值

### 5.3 验证

1. 模拟器截图 OCR 断言终端文字（maestro + rapidocr，现有截图管道可扩展）
2. 新增 instrumented 测试：渲染帧 → 截屏 → OCR 含 prompt 字符
3. 更新失实文档（§4 清单全部修正）

### 5.4 风险

- wgpu 30 Android surface API 需实测（`AndroidNdkWindowHandle` 生命周期、rotation 时 recreate）
- Mali/Adreno 驱动差异（pass.rs 已有 Mali-G57 acquire 超时 worker 保护）
- ghostty CellData → 实例 → shader 的 Android 首次实机验证

---

## 6. 附：CI artifact 信息

- Run：https://github.com/torvox/actions/runs/30730690481（Release，in_progress，headSha c694056）
- Artifacts：debug-apk 23.7MB、release-apk 7.6MB（均已下载验证）
- 模拟器 emulator-5554：debug APK 安装运行验证如上
