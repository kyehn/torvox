# 任务

- [x] 建 change（proposal/design/tasks，skip_specs）
- [x] `GlyphInfo::atlas_uv` 收敛图集 UV 换算（`render/font/mod.rs` + `cell_builder.rs` + `font/pipeline.rs`）
- [x] `read_byte_array` 收敛 JNI 字节数组读取（`android/ffi/input.rs`）
- [x] `read_terminal_flag_jboolean` 收敛原子标志到 JNI 布尔的映射（`android/ffi/state.rs` + `input.rs`）
- [x] `rgb_to_rgba` 收敛 RGB 到 RGBA 展开（`terminal/ghostty_terminal/internal/kitty.rs`）
- [x] `ReadScan::pass_through` 收敛 ESC 透传（`terminal/output_processor.rs`）
- [ ] `with_render_state` 收敛渲染状态守卫与解锁时机（`android/ffi/font.rs`）
- [ ] `select_bounds_export` 收敛选区查询导出（`android/ffi/query.rs`）
- [ ] `render_state_guard` 与 `draw_cells` 收敛渲染帧样板（`android/ffi/render/frame.rs`）
- [ ] 图集像素拷贝统一为一条路径（`render/font/atlas.rs`）
- [ ] `create_atlas_bind_group` 收敛 wgpu 绑定组创建（`render/context/atlas.rs` + `render/pipeline.rs`）
- [ ] 查询资源改传 `QueryContext`（`terminal/ghostty_terminal/internal/dispatch.rs`）
- [ ] 空闲节拍与批尾合并为 `respond_queries_and_publish`（`terminal/ghostty_terminal/internal/run.rs` + `commands.rs`）
- [ ] 单发查询包装内联（`terminal/ghostty_terminal/public_api.rs`）
- [ ] 修饰键状态读取收敛为单一读取点（`android/.../ui/TerminalSurface.kt`）
- [ ] 测试内推进宽度求和 helper（`render/font/shaping.rs`）
- [ ] 跑完整 CI 门禁（`nix develop -c nu scripts/check.nu`）
- [ ] `openspec validate --strict` 通过后归档
