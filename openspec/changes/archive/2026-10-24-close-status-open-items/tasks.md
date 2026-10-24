# Tasks

## 1. 选择手柄交叉（`text-selection`）

- [x] 1.1 抽出 `dragTargetColumn` / `dragTargetRow` 纯函数（锚点 + 手指整格位移 + 网格钳位），
      验证：`DragTargetCellTest` 覆盖参考点解析、单格位移、钳位、交叉翻转后归属、行折算，全绿
- [x] 1.2 抓柄时锁定 `dragAnchorIsStart`，快速路径 `dragTargetFromTouch` 与边缘滚动路径
      `updateDragHandleForCell` 共用同一折算，验证：两路径产出的目标列在同锚点下相等
      （`DragTargetCellTest`）
- [x] 1.3 补齐边界不变式：被拖边界停在手指所在格、未拖动侧钉在交叉前位置，
      验证：仪器化用例 `dragging_end_past_start_keeps_finger_on_the_dragged_boundary` 通过；
      临时把参考点改回「随归属翻转」后该用例转红，恢复后转绿
- [x] 1.4 规范增量落 `specs/text-selection/spec.md`（参考点锁定、被拖边界跟手、
      未拖动侧钉住、手柄与边界对齐），验证：`openspec validate --strict` 通过
- [x] 1.5 把 `控制柄拖动` 拆成三条「一条行为一条要求」的需求
      （`控制柄拖动` / `手柄交叉时的触点折算` / `抬手帧以最终触点重算`），
      验证：`openspec validate --all --strict` 无「需求正文过长」告警

## 2. 搜索高亮判别力（`text-search`，主 spec 不动）

- [x] 2.1 把 `applySearchHighlights` 的视口裁剪与配色折算抽成纯函数
      `build_search_highlights`，验证：JNI 入口改为调用纯函数，行为不变
- [x] 2.2 补齐三个 Rust 测试（当前命中 alpha 255 + 前景色、其余命中 alpha 160 + 选区色、
      视口外命中两侧裁剪、无选中时全部按其余命中着色），
      验证：`cargo test --lib search_highlight` 全绿
- [x] 2.3 质量闸门，验证：`cargo fmt --check`、`cargo clippy`、`cargo test --lib` 全通过

## 3. kitty 清屏即撤下放置（`kitty-graphics`）

- [x] 3.1 ED2 在同一次清除内撤下全部图像放置（直接放置通道与 Unicode 占位符通道共用语义），
      验证：Rust 用例 `ed2_removes_kitty_placements_on_first_clear` 通过
- [x] 3.2 补齐像素级判据，验证：仪器化用例 `clear_erases_image_on_first_ed2` 通过，
      一次 ED2 后图像像素不再出现在屏幕上
- [x] 3.3 规范增量落 `specs/kitty-graphics/spec.md`（清屏即撤下放置并清像素），
      验证：`openspec validate --strict` 通过

## 4. 占位符扫描按绝对行复用（`kitty-graphics`）

- [x] 4.1 逐行签名只含行内布局、视口签名带绝对行号，缓存基准取 VT 滚动条给出的绝对行号，
      验证：`placeholder_layout_signature` 的签名变化用例通过
- [x] 4.2 用扫描统计作判别力，补齐「同布局重查不重扫」与「回看历史命中缓存」两个用例，
      验证：`kitty_placement_query_reuses_unchanged_rows`、
      `kitty_placement_row_cache_survives_scrollback_viewing` 通过
- [x] 4.3 规范增量落 `specs/kitty-graphics/spec.md`（占位符扫描按绝对行复用），
      验证：`openspec validate --strict` 通过

## 5. 证据留档

- [x] 5.1 写 `status-disposition.md`：四项逐条写状态、自动化判据（用例名 + 计量）与剩余条件，
      验证：文件内无原始日志行、时间戳、PID、设备标识（逐条核对）
- [x] 5.2 记明第三份 logcat 的基线归属（采自不含占位符缓存修复的分支，属修复前基线），
      验证：留档只写事实与计量，不引用原始日志行

## 6. 集成验证

- [x] 6.1 Rust 全量，验证：`cargo test --lib` 全绿
- [x] 6.2 Kotlin JVM 单测，验证：`:app:testDebugUnitTest --tests DragTargetCellTest
      --tests SelectionStateTest` 全绿
- [x] 6.3 仪器化，验证：`:app:connectedDebugAndroidTest` 跑
      `SelectionDragQuantifiedTest`、`KittyGraphicsPlacementTest`、`ScrollBehaviorQuantifiedTest`
      全绿
- [x] 6.4 代码风格与静态检查，验证：`:app:detekt`、`:app:spotlessCheck` 通过

## Workflow follow-up

- `openspec validate 2026-10-24-close-status-open-items --strict` 通过后，
  执行 `openspec archive 2026-10-24-close-status-open-items`，
  把 `text-selection` 与 `kitty-graphics` 的 delta 同步进主 spec。
- 归档后执行 `openspec validate --all --strict`，确认全量规范一致。
- 归档结果连同本 change 的全部文档作为一个小步提交并推送到 `origin/main`。
