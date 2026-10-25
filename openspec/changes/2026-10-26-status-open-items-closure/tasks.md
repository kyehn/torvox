# Tasks

## 1. 选区端点按行主序线性下发（`text-selection`）

- [x] 1.1 `publishNormalizedSelectionBounds` 改用行主序归一（`selectionEndpoints`），
      不再用行列 min/max 的包围盒角点，验证：`SelectionStateTest` 的
      `endpoints stay linear when the start column is to the right` 全绿
- [x] 1.2 `syncDragBoundsToNativeThrottled` 下发有序端点对（拖动途中的高亮与最终提交同源），
      验证：`drag bounds are the native endpoints verbatim` 全绿
- [x] 1.3 补 JVM 用例：跨行列倒置时的下发端点、行内倒置时的下发端点、
      前向端点保持顺序，验证：新增 4 例全绿
- [x] 1.4 规范增量落 `specs/text-selection/spec.md`，验证：`openspec validate --strict` 通过

## 2. 搜索居中按可见带（`text-search`）

- [x] 2.1 新增纯函数 `centerSearchViewportRow(visibleContentPx, shiftPx, cellHeightPx)`，
      由可见带正中反解命中行在视口中的目标行，验证：纯函数用例全绿
- [x] 2.2 `TerminalSurface.searchViewportTop` 暴露按可见带折算的视口顶行，
      `TerminalScreen.centerSelectedRow` 改用它；删除只被测试引用的
      `visibleSearchRows` / `centerSearchRow`（含基线上唯一失败的单测），
      验证：仓内无残留引用（`grep` 无命中）
- [x] 2.3 补用例：输入法遮挡超过总行数三分之一时命中行仍落在可见带内，
      验证：`SearchCenterViewportRowTest` 全绿
- [x] 2.4 规范增量落 `specs/text-search/spec.md`，验证：`openspec validate --strict` 通过

## 3. kitty 撤下放置的代数不变式（`kitty-graphics`）

- [x] 3.1 订正 `tests.rs` 中与实测相反的用例注释（上游撤下虚拟放置**不**推进存储生成戳），
      验证：注释与 `types.rs` 字段注释一致
- [x] 3.2 在虚拟放置用例里断言「存储生成戳不变、放置生成戳变化」，
      验证：`ed2_removes_virtual_kitty_placement_on_first_clear` 全绿
- [x] 3.3 记录直接放置通道的差异（图像条目随放置删除，存储生成戳亦变化），
      验证：`ed2_removes_kitty_placements_on_first_clear` 全绿，两条通道的共同信号
      只有放置生成戳
- [x] 3.4 规范增量落 `specs/kitty-graphics/spec.md`，验证：`openspec validate --strict` 通过

## 4. 留档

- [x] 4.1 写 `status-disposition.md`：四项逐条写根因、修复、自动化判据（用例名）与剩余条件，
      验证：文件内无原始日志行、时间戳、PID、设备标识
- [x] 4.2 记明本轮复核基线为 canonical `main`（与 `kyehn-patch-1` 无共同祖先），
      验证：留档只写事实与计量

## 5. 集成验证

- [x] 5.1 Rust：`cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
      `cargo test --lib` 全绿
- [x] 5.2 JVM：`:app:testDebugUnitTest`、`:app:detekt`、`:app:spotlessCheck` 通过
- [x] 5.3 规范：`openspec validate --all --strict` 全绿
