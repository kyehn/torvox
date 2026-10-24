# Design

## 一、两条判据（本轮实现）

### 增量 == 全量

termlib 的 `TerminalRedrawTest` 断言「增量更新后的位图与全新全量渲染逐像素相同」。
本仓的脏带路径只决定「哪些行重画」，行内容必须与全量相同，因此等效判据落在实例
输出上：同一份 `CellData` 与光标，全量构建与「先全脏填充缓存、再只标一行脏」的
增量构建必须逐字节相同。CPU 层逐字节一致后，GPU 侧只多出「画哪些行」，而画哪些行
的像素正确性由 `gpu_scroll_offset_full_redraw_has_no_stale_pixels`（滚动残留）与
`band_clear_instances_cover_band_rect`（脏带平铺范围）分别覆盖。

判据：`cached_build_matches_full_build_byte_for_byte`。

### resize 循环不累积

termlib 的 `AndroidMemoryTest` 在旋转 resize 循环里断言 scratch 复用。本仓对应物
是行缓存与实例缓冲：resize 使行布局失效后 MUST 整缓存重建，输出实例数等于当前
网格单元数。缓冲容量按需增长且不复原（`ensure_instance_capacity` 只在不足时重建），
因此「不累积」要盯的是**旧网格的行是否残留在输出里**，而不是容量数字。

判据：`resize_cycles_do_not_accumulate_stale_rows`（五种网格来回切换，逐次断言
实例数等于 `rows * cols`）。

## 二、三条口径（本轮成文）

### 真机测量操作规程

原为「后续所有真机性能工作的前提」，本轮直接写成规范要求，不再留作文档待办：
判据用 p95 / p99 / missed-deadline 百分比而非中位数；两个 revision 对比必须同设备、
同刷新率、同字体、同尺寸、交替顺序；环境条件（设备、构建、热状态）随结果记录；
模拟器数据 MUST NOT 用于建立物理设备性能结论。CI 侧维持现状：`check.nu` 只编译
benchmark 模块，不上性能门禁。

### 不采纳：真实 PTY 捕获重放

把 `vt_typing` 基准升级为「捕获真机 PTY 流并按 60Hz 重放」需要一份可复现的语料；
语料若进仓库即违反 `BUILD.md` 的「不得 vendor 可自动获取的测试数据」，若现场采集
则每次对比的设备与时机都不同，反而丧失可比性。键入路径已有 JVM/Rust 用例与
`bulk_output_is_processed_without_loss` 突发无损用例覆盖，合成负载保留。

### 不采纳：帧循环 atrace counter 与 FrameTimeline 关联

input-to-present 关联诊断需要「输入侧计数器 + 帧侧计数器 + Perfetto 关联脚本」三件
套，且只能在真机上跑；本仓已有等价排障能力——帧与循环计时报告（avg/p95/max +
帧内分段 `SLOW_RENDER_INNER` / `SLOW_RENDER_PHASE`）足以定位帧内慢段。该项作为真机
排障的可选项记录，不入规。

## 三、`docs/research/` 的处置

已解决条目从文档移除，而不是改写为「已解决」——否则下次阅读仍要逐条判断。六份文档
保留研究结论（架构要点、对照表、权威常量出处、不适用项与理由），只删「可参考点 /
行动建议 / 观察项」中已闭环的部分，并各加一句指向 `performance-measurement` 等能力的
归属说明。`reference-verification.md` 的「风险」节保留（那是静态核验的固有边界说明，
不是待办）。
