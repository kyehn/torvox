# 三 agent 并行裁定（oh-my-pi / maki / dsh）

> 提交基线：main=102d451。三 agent 各在独立 worktree/分支上由 paseo 并行推进同一 openspec change，
> 完成后各自 REPORT / 审计 / 设备证据。以下为该轮"哪个 agent 最佳"的正式裁定。

## 一、任务分配（同根因，三路径）

| change | 分配 agent | 完成度 | 产出（main 已含） |
|---|---|---|---|
| fix-scroll-residual-tearing | dsh | 12/12 ✓ | `is_scroll_offset_active` 门控 + REPORT.md + 设备证据 |
| fix-render-performance T3（退格空闲唤醒） | ohmy | T3 ✓ | `shouldUseIdleLatch()` 空闲门控 + RenderLatchCadenceTest（551+602 pass）+ REPORT.md |
| fix-render-performance T2（IME 平移） | maki | T2 ✓ | insets 叶节点读取 + snapshotFlow 位移 + TerminalImePanTest（11/11）+ AUDIT.md |

## 二、交付质量核验

### ohmy（pi/opencode/big-pickle）——「根因最准」
- **直击用户主诉**：退格在 >5s 空闲后要等 **500ms idle latch**（实测 loop avg 509ms）。修复：
  每次 PTY 写入都唤醒渲染循环（硬件键/IME sendKeyEvent 都走 `notifyRender`）——退格回显从 ~500ms
  降到 ≈17ms 活动帧。
- **架构复用的两级抽象**：把空闲 latch 选择提为**纯函数** `shouldUseIdleLatch()`（可测真值表），
  门控与滚动门控同一风格；测试 551+602 全绿、渲染 cadence 决策表逐条断言。
- **交付完整**：REPORT.md（根因+修复+回归）→ merge commit → push origin。
- **多一层**：不仅查了 Kotlin IME 路径，还拆了 `onPtyWrite` 钩子覆盖硬件键/鼠标写 PTY 的统一唤醒。

### maki（maki/opencode-openai-responses）——「审计最体系化」
- **IME 动画卡顿**用户观感第二主诉。根因：IME insets 在主组合体读取 → 每动画帧整屏重组。
- 修复走**叶节点读取 + snapshotFlow 位移**（动画帧只重绘位移叶节点，不整屏重组）——先于定居写、
  光标跟随、隐藏回落三态统一。回归断言把 IME 平移/中间帧/钳制全覆盖。
- 11/11 + native 套件全绿；AUDIT.md + REPORT.md 双文档，含 openspec change T2 审计结论。

### dsh（dsh/zen:big-pickle）——「通过但存争议」
- 完成滚动残留门控（12/12、REPORT 像素证据）。但**纯门控判定以"viewport_scroll_px ≠ 0"为标准**，
  而 ohmy 用"运动观测窗口"（motion-recency），且 **ohmy 已验证滚动 + 退格两条输入唤醒路径**，
  dsh 未覆盖硬件键路径。

## 三、裁定

**最佳 = ohmy**（pi/opencode/big-pickle）。

理由：
1. **命中主诉根因** —— 退格慢是用户第一主诉。ohmy 的 `shouldUseIdleLatch()` 空闲门控让
   硬件退格回显从 500ms→17ms，这是模拟器 logcat 实测的可量化收益。
2. **输入全路径统一唤醒** —— 不止修 IME，把硬件键/IME-sendKeyEvent/鼠标 PTY 写三条路径
   全部接管，避免"修了 IME 但硬件键仍等 500ms"的局部正确。
3. **测试策略最扎实** —— 纯函数形式的门控决策表 + latch cadence 测试，可离线复跑、
   不依赖模拟器；551 native + 602 android 全绿。
4. **交付纪律最好** —— 单 responsibility commit、REPORT 文档、merge 主干的流程完整，
   无任何未归档尾巴。

maki 为次优（审计体系化、修复规范，但只覆盖 IME 单一路径）；dsh 完成滚动残留门控但
覆盖路径最窄（未验证硬件键唤醒）且 REPORT 未含设备像素对比，故列第三。

## 四、本轮追加验证（主 agent 落地，基线 a587e76）

> dsh `fix-perf-dsh` 结论"无需改生产代码，仅补 3 测试"经独立核验基本成立，
> 但其测试闭包内联复述门控公式、未直接断言生产合成。主 agent 已收敛为
> `should_render_partial_frame` 纯函数：生产调用点与测试走同一函数，
> `nix develop cargo test render::pass::tests` 11/11 通过，clippy 零警告，
> commit `45e72c7` 已合入 main。dsh REPORT 引用的
> `gpu_scroll_offset_full_redraw_has_no_stale_pixels` 真实存在
> （`native/src/render/tests.rs:3047`），归属描述准确。

| agent | 最终产出 | 测试 | 状态 |
|---|---|---|---|
| ohmy（退格唤醒） | `shouldUseIdleLatch` + PTY 写唤醒，已合入 main | 551+602 全绿 | 已归档 |
| maki（IME 平移） | 叶节点 insets + snapshotFlow，已合入 main | 11/11 + 602 | 已归档 |
| dsh（滚动门控） | 边界审计 + 3 合成测试，经主 agent 纯函数化后合入 main（45e72c7） | pass 模块 11/11 | 已合入 |
| 主 agent | ENV 白名单 3.1/3.2 设备验证（PS1 2 列对 77 列，clear 正常，bash 忽略 ENV） | 设备实测 | 已归档（2026-09-21-add-env-whitelist） |

**最终裁定维持：最佳 = ohmy** —— 唯一命中用户第一主诉（退格 500ms→17ms 可量化收益），
输入全路径统一唤醒，测试策略最扎实，交付纪律完整。
maki 次优（IME 单路径但审计体系化）；dsh 第三（审计正确但测试未钉死生产合成，需主 agent 收敛）。
