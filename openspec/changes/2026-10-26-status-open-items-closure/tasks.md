# Tasks

## 1. 选区端点按行主序线性下发（`text-selection`）

- [ ] 1.1 `publishNormalizedSelectionBounds` 改用 `normalizeSelectionAnchors`（行主序），
      不再用行列 min/max 的包围盒角点，验证：跨行且起始列更靠右时下发的是
      `(r1, c1) → (r2, c2)`
- [ ] 1.2 `syncDragBoundsToNativeThrottled` 下发有序端点对（拖动途中的高亮与最终提交同源），
      验证：与 1.1 同一条用例覆盖
- [ ] 1.3 补 JVM 用例：跨行列倒置时的下发端点、行内倒置时的下发端点、
      `extractSelectedText` 与高亮取同一对端点，验证：新增用例全绿
- [ ] 1.4 规范增量落 `specs/text-selection/spec.md`，验证：`openspec validate --strict` 通过

## 2. 搜索居中按可见带（`text-search`）

- [ ] 2.1 新增纯函数 `centerSearchViewportRow(visibleContentPx, shiftPx, cellHeightPx)`，
      由可见带正中反解命中行在视口中的目标行，验证：纯函数用例全绿
- [ ] 2.2 `TerminalSurface` 暴露按可见带折算的视口顶行，`TerminalScreen.centerSelectedRow`
      改用它；删除只被测试引用的 `visibleSearchRows` / `centerSearchRow`，
      验证：仓内无残留引用
- [ ] 2.3 补用例：输入法遮挡超过总行数三分之一时命中行仍落在可见带内，
      验证：新增用例全绿
- [ ] 2.4 规范增量落 `specs/text-search/spec.md`，验证：`openspec validate --strict` 通过

## 3. kitty 撤下放置的代数不变式（`kitty-graphics`）

- [ ] 3.1 订正 `tests.rs` 中与实测相反的用例注释（上游撤下虚拟放置**不**推进存储生成戳），
      验证：注释与 `types.rs` 字段注释一致
- [ ] 3.2 在两个 ED2 用例里断言「存储生成戳不变、放置生成戳变化」，
      验证：`ed2_removes_kitty_placements_on_first_clear`、
      `ed2_removes_virtual_kitty_placement_on_first_clear` 全绿
- [ ] 3.3 规范增量落 `specs/kitty-graphics/spec.md`，验证：`openspec validate --strict` 通过

## 4. 留档

- [ ] 4.1 写 `status-disposition.md`：四项逐条写根因、修复、自动化判据（用例名）与剩余条件，
      验证：文件内无原始日志行、时间戳、PID、设备标识
- [ ] 4.2 记明本轮复核基线为 canonical `main`（与 `kyehn-patch-1` 无共同祖先），
      验证：留档只写事实与计量

## 5. 集成验证

- [ ] 5.1 Rust：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib` 全绿
- [ ] 5.2 JVM：`:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessCheck` 通过
- [ ] 5.3 规范：`openspec validate --all --strict` 全绿
