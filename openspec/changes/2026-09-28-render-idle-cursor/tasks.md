# 任务

- [x] 建 change（proposal）
- [x] `renderWithNewOutput` 空闲帧同样采样光标行（`ffi.rs` 门由 `count > 0` 改为
      `count >= 0`，光标采样移出渲染计数门）
- [x] 空闲帧光标行已上报：原诊断 `ImeDiagTest#dumpPanInputs` 已按第 5 项删除，
      改以行为取证确认——`computeTerminalPanPx` 在 `cursorRow < 0` 时返回 null
      （保持旧平移），只有拿到真实光标行才可能算出正平移；release APK 在 90 行
      内容、光标位于末行时点按终端弹出输入法，实测内容上移且键栏同步抬到键盘上方，
      以测试同款差分口径量得 29942 个差异像素（阈值 20）。
- [x] `contentFewImePopupTerminalStaysPutAndVisible` —— **本轮已修并通过**：
      三处测量/前提问题逐条修掉（系统状态栏带计入比对 → 比对区改用窗口可见显示区
      上沿；「内容较少」前提不成立 → 用例先清屏再在首行重打标记；闪烁比对从 y=0
      起算 → 同样避开状态栏）。逐像素实测：终端内容在弹出前后位移 0、行墨量残差 0.0。
- [ ] `contentManyImePopupMovesUpBottomIdentical` —— **仍受阻，非产品缺陷**：
      本机 AVD 稳定 `位移=0 差异=0`。本地具备 AVD 后复跑确认：输入法时序正确
      （`mIsInputViewShown=true`、高度稳定），120 行内容下位移量在本 AVD 上算不出，
      与 CI（该用例通过）的差异来自 AVD/输入法形态。需在启用中文输入法的 AVD 上复跑。
- [ ] `imeCommitChineseTextGridded` —— **受阻，非产品缺陷**：本机 AVD 只装了
      `LatinIME` 且未启用中文输入语言，无法提交中文（CI 的 AVD 装有 Gboard 中文，
      该用例在 CI 通过）。
- [x] 删临时诊断（`ImeDiagTest` 与 `dumpPanInputs` 引用均已不存在），
      `check-rust.nu` 零错误零警告，小步提交推送
