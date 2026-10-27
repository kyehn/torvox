# Tasks

## 1. 字体库侧拆分

- [x] 1.1 `font_db::load_family` 拆为 `family_files`（解析族文件列表，可能构建族
      索引）与 `load_family_files`（只装入族文件）；补宿主构建的 `family_files` 桩。
- [x] 1.2 新增 `font_db::family_display_names`：族索引的展示名，供列表构造在锁外
      取用。
      验证：`cargo clippy --all-targets -- --deny warnings` 无告警。

## 2. 管线侧接口

- [x] 2.1 `FontPipeline::set_font_family` 接受族文件列表入参，按需装入不再自行触发
      族索引枚举。
- [x] 2.2 新增 `FontPipeline::family_resident`：族是否已在渲染字体库中，判定口径与
      `set_font_family` 的取面顺序一致。
- [x] 2.3 `FontPipeline::list_monospace_fonts` 的设备分支改为委托
      `font_db::family_display_names`，生产路径在锁外取列表。
      验证：`cargo test --lib render::font` 全绿（138 passed）。

## 3. JNI 侧锁区间

- [x] 3.1 `listFontFamilies` 在渲染状态锁外构造族列表（Android）。
- [x] 3.2 `setFontFamily` 改两段式：持锁判定族是否常驻 → 解锁解析族文件列表 →
      重新持锁装入并选中。
- [x] 3.3 `loadFontFile` 跟随新签名（族已在库中，不解析族文件列表）。
      验证：`cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings` 通过。

## 4. 回归用例

- [x] 4.1 `family_resident`：空族名恒为常驻；已装入的族为常驻；未知族非常驻。
- [x] 4.2 既有 `set_font_family` 用例跟随新签名，行为不变。
      验证：`cargo test --no-fail-fast` 全绿（737 passed / 0 failed）。

## 5. 全量验证

- [x] 5.1 `cargo fmt --all -- --check`、`cargo clippy --all-targets -- --deny warnings`、
      `cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings`、
      `cargo test --no-fail-fast` 全绿。
- [x] 5.2 `semgrep scan --error --dataflow-traces --config .semgrep/rules.yml` 0
      findings；`markdownlint-cli2 "openspec/**/*.md"` 0 issues；
      `openspec validate --all --strict` 39 passed / 0 failed。

## Workflow follow-up

- 归档本变更（`openspec archive 2026-10-27-fix-font-index-blocking-render -y`），把
  `font-selection` 的变更并入 `openspec/specs/font-selection/spec.md`。
