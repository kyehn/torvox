# 任务

- [x] 建 change（proposal）
- [x] `renderWithNewOutput` 空闲帧同样采样光标行（`ffi.rs` 门由 `count > 0` 改为
      `count >= 0`，光标采样移出渲染计数门）
- [x] 空闲帧光标行已上报：原诊断 `ImeDiagTest#dumpPanInputs` 已按第 5 项删除，
      改以行为取证确认——`computeTerminalPanPx` 在 `cursorRow < 0` 时返回 null
      （保持旧平移），只有拿到真实光标行才可能算出正平移；release APK 在 90 行
      内容、光标位于末行时点按终端弹出输入法，实测内容上移且键栏同步抬到键盘上方，
      以测试同款差分口径量得 29942 个差异像素（阈值 20）。
- [ ] `ImePopupPixelInstrumentedTest` 全过 —— **受阻，非产品缺陷**：
      新建 AVD 只装了 `LatinIME`，Gboard 未启用中文输入语言，
      `imeCommitChineseTextGridded` 因此无法提交中文（属环境前置缺失）。
      `contentManyImePopupMovesUpBottomIdentical` 在该 AVD 上稳定 `差分=0`：
      IME 时序已确认正确（logcat `mInputShown` t=2..4s 亮 → t=6s 灭 →
      t=14s 亮），但该环境下 120 行内容在弹出前已随网格增长重新落位，
      光标始终在末行，弹出时无平移量可算。需在启用中文输入法的 AVD 上复跑确认。
- [x] 删临时诊断（`ImeDiagTest` 与 `dumpPanInputs` 引用均已不存在），
      `check-rust.nu` 零错误零警告，小步提交推送
