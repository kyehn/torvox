# 设计：surface 失效检测与自愈

## 1. 机制分析

### 1.1 一次「可用 → 永久不可用」的单向门

`Renderer` 持有 `surface: Option<wgpu::Surface>` 与 `surface_config`。两条路径会用到它：

- `attach_surface`（`native/src/render/context.rs:355`）
  - 快路径：`self.surface.is_some() && self.surface_config.is_some()` →
    `reconfigure_swapchain(w, h)`（`:368-372`），日志 `RECONFIGURE_SWAPCHAIN`。
  - 慢路径：丢弃旧 `surface` / `surface_config`，`create_surface_unsafe` 重建，
    日志 `attach_surface: configured`。
- 帧内 acquire（`native/src/render/pass.rs:153-160`）：`Lost | Outdated` →
  `reconfigure_surface(...)` 后丢弃本帧。

快路径的存在有实测依据（见 `:365-367` 注释）：重建会与渲染线程竞争，SwiftShader 上
报 `ERROR_NATIVE_WINDOW_IN_USE_KHR`，reconfigure 是零拷贝的。但它成立的前提是
**window 仍然可用**。

Android 侧被遗弃的是 BufferQueue（`BufferQueue has been abandoned` +
`NATIVE_WINDOW_MIN_UNDEQUEUED_BUFFERS … No such device (-19)`）：producer 还活着，
consumer 已经没了。此时：

- `reconfigure` / `get_physical_device_surface_capabilities` 必然失败
  （`ERROR_SURFACE_LOST_KHR`）；
- wgpu 的 `Surface` 对象本身不会自愈，它缓存的 `AndroidNdkWindowHandle` 指向死窗口；
- `SurfaceHolder.Callback` 不会再回调（同一个 SurfaceView 没有新 surface），
  所以 Kotlin 侧也不会自然地重新 `attachWindow`。

于是形成单向门：**唯一 surface 一旦绑定到死窗口，该进程内渲染永久失效**。
`render_inner` 在 `renderer.surface.is_none()` 时返回 `0`、在 `begin_frame` 失败时返回
`-1`（`ffi.rs:1458` / `pass.rs:347-349`），Kotlin 侧只看到 `count=-1`，
没有任何「需要换 surface」的信号。

### 1.2 失效被静默吞掉的位置

现状把「surface 级失败」与「本帧没画东西」混为一谈：

| 位置 | 现状 | 后果 |
| --- | --- | --- |
| `pass.rs` acquire | `Lost`/`Outdated` 只 reconfigure，丢弃本帧 | 死窗口下每帧都走这条路 |
| `context.rs` attach | 有 surface 就快路径 | 拿到新 ANativeWindow 也不重建 |
| `ffi.rs` render | 返回 `count=-1` / `0` | 宿主无法区分「暂停」「无内容」「surface 死了」 |
| Kotlin 渲染循环 | 只看 `count` | 无恢复触发点 |

### 1.3 为什么仪器化下必现、非仪器化下少见

非仪器化运行（本会话多次手动验证）终端持续正常渲染、无 `abandoned` 日志；
in instrumentation 两次运行（pid 4626：`abandon=70 configured=0`；
pid 5303 全量：`abandon=6148 configured=1`）必现，且一旦发生后续 132 次
`attach` 全走快路径。触发条件是模拟器 SurfaceFlinger/BufferQueue 侧的回收
（`-no-window -gpu swiftshader_indirect` + UiAutomator 反复抓屏/查询），
应用侧无法预防，但**必须能自愈**。

## 2. 方案对比

### 方案 A：Rust 侧在失败时重建 wgpu Surface

在 `begin_frame` 失败后直接 drop 并用当前 `ANativeWindow` 重建。

- 否决理由：死的是 window 本身，重建只会再次 `Surface::configure` 失败；
  而且与渲染线程竞争会触发 `ERROR_NATIVE_WINDOW_IN_USE_KHR`（已有注释记录）。
  它只解决「缓存了不该缓存的 surface」，不解决「没有可用 window」。

### 方案 B：Rust 失效缓存 + 宿主重建 surface（采纳）

1. **失效（原生）**：`begin_frame`/acquire 判定 surface 级失败时，把
   `surface` / `surface_config` 置失效，并把失效计数/首次时间记录到会话 RenderState。
   失效是幂等的、每帧最多一次（首个失败帧做，之后不重复记账）。
2. **上报（原生 → 宿主）**：`renderWithNewOutput` 的打包返回里增加一个状态位
   （surfaceUnavailable）。与既有 `newOutput` / 光标行 / 内容下沿同一条返回通道，
   **零额外 JNI 调用**、零分配。置位后不再随每帧变化（`distinctUntilChanged` 天然去重）。
3. **换 surface（宿主）**：渲染循环观察到该位由 0 变 1 时，摘下再挂回同一个
   `SurfaceView`（或切换一次 visibility），强制 `SurfaceHolder` 交付新的
   `ANativeWindow` → `surfaceCreated` → `attachWindow` 走**重建慢路径** → 渲染恢复。
4. **限流与退避**：每次「失效 → 请求重建」之间要求
   `surfaceUnavailable` 先回落为 0（即重建成功、开始出帧），或距上次请求 ≥ N ms；
   单会话连续重建次数超过上限后停止请求并打一条 error（避免抖动/耗电）。

### 方案 C：改用应用自建 `SurfaceTexture`

彻底摆脱 SurfaceView 的 BufferQueue 归属问题，但涉及渲染架构与 FFI 形态变更，
风险与工作量远大于本缺陷。列为未来项。

## 3. 采纳方案的关键约束

- **不破坏快路径**：快路径条件收紧为「surface 已配置 **且** 未被标记失效」。
  IME 收起/尺寸不变的常见路径仍是零拷贝 reconfigure，不引入重建开销。
- **不在帧内重建**：失效标记只改状态位；重建仍由 `attach_surface` 在
  「调用方保证无渲染线程处于帧中」的既有契约下执行（与慢路径现状一致）。
- **失败原因可区分**：暂停（`render_paused`）与「本帧无内容」不置失效位——
  否则 IME 收起过渡会误触发重建风暴。
- **回收语义**：`detachWindow` 现有行为（按属主 CAS 释放 surface、拆除 RenderState）
  保持不变；新增的失效位随 RenderState 一并清除。

## 4. 27 个失败的原因分类与后续动作

| 类别 | 数量 | 判定依据 | 动作 |
| --- | --- | --- | --- |
| A 画面未呈现 | 8 | 断言全为「零像素 / 零手柄 / 位移=0」，且同一次运行 logcat 有 `abandoned` → `begin_frame failed` 全链（3074 帧） | 本 change 修复后复测 |
| B 输入未送达 | 9 | `MultiTapSelectionInstrumentedTest#doubleTap…` 失败消息即 `IME 必须弹起（20s 未可见）`；同族 `标记必须落格` / `中文提交必须落格` / `burst` / `UXMARK1` | 本 change 后复测；若仍失败，取 input_method 日志单独立项 |
| C Shell / VT / 剪贴板 | 6 | 断言与渲染无关（`rc=130`、`BEL 振铃事件必须上报`、`viewport never snapped…`、OSC52 marker 未达）；`StickyCtrlInterrupt…` 消息尾部确有网格内容（`IME_MANY_85_44795`…`$`），说明网格在更新 | 与本 change 解耦后单独立项（含 OSC52 族） |
| D 预置资产缺失 | 2 | `installer.*` 自述需 `adb push termux-bootstrap-x86_64.zip` | 测试前置条件，另行处理 |
| E 其他 | 2 | `字号增大后列数必须收缩 (前=33 后=33)`、`必须切回首个会话` | 单独诊断 |

合计 8+9+6+2+2 = 27，与 XML 报告一致。A 类已有 logcat 直接证据；B/C/E 为**工作假设**，
需在渲染恢复后复测确认（避免把「渲染没了」误判成输入/Shell 缺陷）。复测结论回写本表。

## 5. 已落地的相关修复（不属于本 change）

`:app:connectedDebugAndroidTest` 之前跑的是 dev profile 的 native 库
（`build-android-libs.nu` 每 ABI 只落地一个 `.so`，`build.yml` 后建 dev 覆盖了
release），在 2 核 SwiftShader runner 上会把 AVD 拖死。已在 emulator 步骤内先执行
`scripts/build-android-libs.nu --profile release`，使仪器化/benchmark 以 release
profile 库运行（实测：dev 库 → 复现 CI 失败并掉设备；release 库 →
`BUILD SUCCESSFUL in 28s`、全量套件跑完 22m12s、`BehaviorInstrumentedTest` 10/10
通过）。该修复**不能**替代本 change：它只是让模拟器活下来，渲染失效依然存在。
本地手动跑套件时同样需要先部署 release profile 库
（`docs/specification/TESTING.md` 已要求模拟器调试使用 release apk）。

## 6. 验证方法

1. **单测**：Rust 侧对「失效标记 → 下次 attach 走重建路径」「失效位不因暂停置位」
   写宿主单测；Kotlin 侧对「状态位 0→1 触发一次重建请求、随后回落不再重复」
   写单测。
2. **仪器化**：新增用例「渲染中强制遗弃 surface 后自愈」——需要可控的注入点
   （见 tasks 中对注入方式的要求），断言自愈后像素断言重新有墨迹。
3. **像素族复测**：`diag/*PixelAcceptanceTest`、`VisualInlineVerificationTest`、
   `ImePopupPixelInstrumentedTest` 从「在一片空白上做像素断言」变为真实判据
   （先断言有墨迹/有手柄，再断言行为）。
4. **回归**：全量 `:app:connectedDebugAndroidTest`（161 例）失败数从 27 下降到
   A/B/C/E 类的真实剩余数，且不再出现 `abandoned` 之后的持续 `count=-1`。
