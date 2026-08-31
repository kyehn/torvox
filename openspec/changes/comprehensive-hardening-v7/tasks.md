## 1. 基线加固与依赖滚动

- [ ] 1.1 恢复并冻结 `docs/reference` 44 文件，确保 `cargo test --lib` 1016 基线通过，验证 `cargo test --lib 2>&1 | grep passed`
- [ ] 1.2 执行 `cargo update` 与 `flake.lock` 滚动，验证 `cargo check + clippy` 零新增且 so 16M 守卫通过
- [ ] 1.3 配置 `.pi/lens.toml` 超时 120s 或 CI `cargo test` 超时提升，验证暖缓存 76s 不超时

## 2. 鼠标编码硬化 (mouse-encoding)

- [ ] 2.1 审计 `public_api::encode_mouse_event` 的 `ghostty_mouse_encoder` 门控与实时 cellW/H，补充 zelland 引用注释，验证 `cargo test encode_mouse_event`
- [ ] 2.2 补 `encode_mouse_event_bounds_negative_clamp` 与 `oversized_clamp` 单测，验证越界 clamp 正确
- [ ] 2.3 补 `encode_mouse_event_drag_sequence` 单测，验证 press→drag→release SGR 序列
- [ ] 2.4 审计 `ffi.rs:encode_mouse_event_inner` 空 array 与 session 静默丢弃，验证 Kotlin Bridge 透传 live 尺寸
- [ ] 2.5 模拟器 `vim set mouse=a` 后 `adb shell input tap` 验证 SGR 可达

## 3. 无障碍硬化 (a11y-overlay)

- [ ] 3.1 实现/审计 `TerminalAccessibility.visibleLines` 与 `LineNavigator` 包裹，补越界与 MAX 2000 截断单测
- [ ] 3.2 实现 500ms debounce updater 与 `contentDescription` diff 去重，验证 `AccessibilityLineProvider` 单测
- [ ] 3.3 限频 `announceForAccessibility` 500ms 合并 bell/title，验证 Robolectric 不刷屏
- [ ] 3.4 增加 Robolectric 截断/debounce 去重用例，验证 `./gradlew :app:testDebugUnitTest` 通过
- [ ] 3.5 模拟器开启 TalkBack 后 `uiautomator dump` 验证 contentDescription 含可见文本

## 4. OSC133 语义段 (osc133-semantic)

- [ ] 4.1 在 `output_processor.rs` 定义 `SemanticSegment {start_col,end_col,kind,exit_code}` 与 `SemanticKind` 枚举
- [ ] 4.2 实现 `scan_osc133` ST/BEL 双终结、跨 chunk 状态机、A 重置、D;exit_code 解析，验证 `cargo test last_command_output`
- [ ] 4.3 维护 `semantic_segments` Vec 与 `getLastCommandOutput` JNI，验证 `ffi.rs` + `NativeBridge` 透传
- [ ] 4.4 增加 7 项 `osc133_semantic_*` 单测（ST/BEL、跨 chunk、A 重置、exit_code、多行、take 清空），验证 `cargo test osc133`
- [ ] 4.5 Kotlin 端 `printf '\x1b]133;B\x07…'` 序列后验证 `getLastCommandOutput` 端到端

## 5. CellRun 游程 (cell-run-cache)

- [ ] 5.1 定义 `CellRun {start_col,length,fg,bg,flags}` 与 `build_row_runs` 线性扫描（bytemuck 字节等价）
- [ ] 5.2 集成至 `build_row_instances_into` 增量路径，dirty 行按 run 合并后展开，保持 CellData 兼容
- [ ] 5.3 增加 4 项 `cell_run_*` 单测（single/mixed/newline/empty）验证 >50% 合并率
- [ ] 5.4 运行 `cargo bench cell_builder` 验证单测 runs==1 稳定且耗时波动 <10%
- [ ] 5.5 在 `render/tests.rs` 补充 zelland 等价性注释与 `rows_equal` 字节等价实现

## 6. 细节硬化 (terminal-winsize-sync / bootstrap-sha-verification / mcp-argument-tokenization / mcp-peer-credential / render-dirty-cache)

- [ ] 6.1 实现 `terminal-winsize-sync`：spawn 前预计算 pixel→rows/cols 并立即 `setPixelSize`，验证 `stty size` 与渲染网格一致且横竖屏无黑屏
- [ ] 6.2 实现 `bootstrap-sha-verification`：下载后 sidecar `.sha256` best-effort 校验，失败删 staging 重试一次，验证篡改 zip 被拒且 Termux 预设不受影响
- [ ] 6.3 实现 `mcp-argument-tokenization`：引入 `shell-words` crate 拆分 argv，验证 `run_command` 单测覆盖引号/转义/注入
- [ ] 6.4 实现 `mcp-peer-credential`：`getsockopt(SO_PEERCRED)` 校验 uid==app uid，验证异 uid 连接被拒且同 uid 放行
- [ ] 6.5 固化 `render-dirty-cache`：`CachedInstances` + `compute_dirty_bands` 字节等价与 zelland 对照 property test，验证 `cargo test render`

## 7. 构建、测试与模拟器 90fps 验证

- [ ] 7.1 执行 `cargo test --lib` (1016+16) 与 `cargo test -p integration-tests` 47 passed，验证确定性无 flaky
- [ ] 7.2 执行 `./gradlew :app:testDebugUnitTest` 与 `detekt` 零新增，验证 JVM 门控
- [ ] 7.3 执行 `cargo clippy` 与 `cargo machete` 零新增，验证 `cargo-machete` 零未用依赖
- [ ] 7.4 构建 `x86_64` release `.so` 16M 校验 `readelf --dynamic` 无 NEEDED ghostty，`assembleDebug` 后 `adb install -r` 成功
- [ ] 7.5 模拟器 API 35 验证：`am start` + `dumpsys gfxinfo` 解析 90th <11ms 判定 90fps+，验证 `render_frame` 心跳 2112 instances 与 SurfaceFlinger BLAST 可见
- [ ] 7.6 产出 `docs/verification/2026-08-30-comprehensive-hardening-v7-verification.md` 记录 166fps loop、so/APK 尺寸与测试计数

## 8. 文档与发布

- [ ] 8.1 更新 `docs/plans` 详细实施与测试计划，验证 `markdownlint` 新文档 0
- [ ] 8.2 循环审阅三次 `openspec validate` 与 `review/grill` 无阻塞，验证连续三次通过
- [ ] 8.3 git 提交推送（conventional 单行），验证 `git log --format="%an" | sort | uniq -c` 仅 `jane` 且 `git log --merges | wc -l == 0`
