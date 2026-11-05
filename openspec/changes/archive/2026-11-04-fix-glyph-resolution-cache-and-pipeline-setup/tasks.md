# 任务

## 一、解析结论缓存

- [x] `FontPipeline::glyph_information_with_synthesis`：`cjk_glyph_cache` 的读取
      不再挂在 `has_cjk_fallback` 与 `!synthesized` 上；光栅化按当前样式现做。
- [x] 写入侧放开样式门：粗体/斜体文本同样记入结论缓存（字形号相同，粗斜是
      光栅化后的遮罩后处理）。
- [x] 伪字形拒绝：字形号 0（`.notdef`）与私用区码位不记入结论缓存，也不从缓存
      命中返回。
- [x] `find_glyph_anywhere` 暴露仅测试用的扫描计数；回归钉以**计数**判定「没重扫」
      （断言「缓存里有某个键」只证明「记了」，证不了「没重扫」）。
- [x] 回归用例：同一码位在主字体不覆盖、库内其他面覆盖时，第二次解析不再触发
      全库扫描。

## 二、管线建立与图集重传

- [x] `ensure_cell_pipeline`：着色器编译与图集分配从渲染循环的惰性执行中抽出。
- [x] `attach_window_inner` 成功分支主动建管线（不占 UI 线程）；尺寸未知与渲染
      状态缺失各记一条可见诊断。
- [x] `initialize_pipeline_and_bind_group` 计时并把建立耗时写进日志。
- [x] `reset_atlas` 改为逐字形登记脏区；`reset_dirty_rect_to_cached_glyphs` 只取
      缓存字形的包围盒，不再整幅重传 16MB。
- [x] 回归用例：管线重建后只重传缓存字形包围盒，脏区不含图集外的空白。

## 三、基准重校

- [x] `cell_builder` 上限按修复后实测重校（每格重扫全库会被直接判红）。
- [x] `cjk_resolve` 上限按修复后实测重校（稳态解析不得回到毫秒级）。

## 四、验证

- [x] `cargo clippy --offline --all-targets -- --deny warnings` 通过。
- [x] `cargo ndk -t arm64-v8a clippy --offline --all-targets -- --deny warnings` 通过。
- [x] `cargo test --offline` 全过（原生 801 个用例 + 集成测试）。
- [x] `cargo bench --offline --bench cell_builder --bench vt_typing --bench cjk_resolve -- --quick`
      全过。
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --offline --no-deps` 通过。
