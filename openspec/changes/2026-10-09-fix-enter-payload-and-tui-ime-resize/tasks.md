## 上下文

三个缺陷，全部有代码级与真机实证，且都曾被「声称已解决」过：

- 回车在 raw 模式应用（helix）里变成 `j`：`TerminalInputEncoder` 发 LF(0x0A)，
  而 crossterm 在 raw 模式下把 0x0A 解成 `Char('j') + CONTROL`
  （`crossterm/src/event/sys/unix/parse.rs` L92-107）。真机 `od -An -tx1` 读到 `0a`。
  2026-10-08 归档变更只补了 `KEYCODE_DPAD_CENTER`，键码集合对、载荷错。
- 全屏 TUI 输入法弹出时下部永久不可见：网格不减输入法 inset（`adjustNothing` 下
  Surface 尺寸不变），备用屏位移又被强制为 0，于是 helix 仍按整屏行数布局、
  收不到 SIGWINCH。真机日志：弹出前 `45x48`，应变为 `26x48`。
- 字号调节条范围与实际可设置范围不一致：Kotlin 的 `NATIVE_FONT_SIZE_MAX_SP = 100f`
  与原生 `if !(4.0..=100.0).contains(&size) { return }` 两份魔数漂移时，用户拖到的
  字号被原生静默丢弃。

## 任务

- [x] `TerminalInputEncoder` 无修饰回车改发 CR(0x0D)，`Alt+回车` 改 `ESC CR`。
- [x] 输入法唯一换行提交 `"\n"` 归一为 CR；多字符提交内的换行逐字保留。
- [x] 删除恒为 false 的 bracketed paste 分支（生产调用点只有一个，实参恒 `false`）。
- [x] `applyTerminalEditorInfo` 补 `TYPE_TEXT_FLAG_MULTI_LINE`，
      `IME_ACTION_NONE` 换 termux 同款的 `IME_FLAG_NO_FULLSCREEN`。
- [x] 备用屏把输入法遮挡计入网格高度并防抖重排：遮挡高度由平台 insets 派发维护
      （`TerminalSurface.installImeInsetListener`），备用屏状态取运行期逐帧发布的流值，
      键盘已展开时启动 TUI 与离开备用屏各由备用屏状态翻转补一次触发；
      主屏不扣遮挡、仍走纯平移，且不因重排暂停渲染。
- [x] 删除 `NATIVE_FONT_SIZE_MAX_SP` 与被替代的 `effectiveFontSizeMaxSp`/
      `effectiveFontSizeRangeSteps`，`fontSizeRangeMaxSp` 更名 `fontSizeMaxSp`。
- [x] 原生 `setFontSizeInPlace` 守卫上界改由图集边长推导，越界记错误日志；
      `setRasterScale` 越界同样记错误日志，不再静默丢弃。
- [x] 测试改造：`TerminalInputEncoderTest` 断言 CR 与 IME 换行归一；
      `FontSizeRangeTest` 改为逐档对照 Termux 取值（不再复述实现公式）；
      `TerminalSurfaceLogicTest` 低密度上界改为 256sp；
      `CoerceSpToPxScaleTest` 跟随改名。
- [x] `FontSizeReflowInstrumentedTest` 新增端点验收：调节条两端字号都必须被原生接受
      并生效（此前只测中间档，对上端被原生拒收的缺陷完全无感）。
- [x] 新增 `AltScreenImeReflowInstrumentedTest`：备用屏弹输入法后网格行数必须收缩到
      可见高度且列数不变，收起后恢复。
- [x] 真机验证：模拟器上以 `stty raw -echo; dd bs=1 count=1 | od -An -tx1` 读到回车
      下发字节由 `0a` 变为 `0d`；`applyGridResize` 日志显示 45x48 → 26x48 → 45x48。
- [x] `cargo clippy --workspace --all-targets -- --deny warnings` 零警告。
- [x] 同步 `openspec/specs` 的 `font-selection` 与 `render-stability`。
