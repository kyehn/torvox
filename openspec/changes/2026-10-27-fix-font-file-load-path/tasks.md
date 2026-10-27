# Tasks

## 1. 字体文件装载路径

- [x] 1.1 `FontPipeline::load_font_file` 取文件中首个**有族名**的面并返回族名；
      记录该面到「族名（小写）→ 面 id」。
      验证：`cargo test --lib render::font::tests::loading` 全绿。
- [x] 1.2 `FontPipeline::set_font_family` 优先取记录的面（取用前确认面仍在库里），
      无记录时才走「按需装入」。
      验证：`cargo test --lib render::font::tests::loading` 全绿。
- [x] 1.3 `loadFontFile` 改为直接装进当前渲染字体库，不再另起字体库探测、不再重建
      管线；字体源变化时失效实例缓存并请求重绘。
      验证：`cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings` 通过。

## 2. 额外字体路径登记

- [x] 2.1 `setExtraFontPaths` 不再重建字体管线。
      验证：`grep -rn 'rebuild_font_pipeline' native/src` 无命中。
- [x] 2.2 删除无调用方的 `rebuild_font_pipeline`。
      验证：`cargo clippy --all-targets -- --deny warnings` 无告警。

## 3. 回归用例

- [x] 3.1 装入字体文件后按族名选中必须用记录的那份面。
- [x] 3.2 同一路径重新装入后按族名选中必须改用新面。
      验证：`cargo test --lib render::font::tests::loading` 全绿。

## 4. 全量验证

- [x] 4.1 `cargo fmt --all -- --check`、`cargo clippy --all-targets -- --deny warnings`、
      `cargo ndk -t arm64-v8a clippy --all-targets -- --deny warnings`、
      `cargo test --no-fail-fast` 全绿。
      验证：四条命令退出码均为 0，`cargo test` 736 passed / 0 failed。
- [x] 4.2 `openspec validate 2026-10-27-fix-font-file-load-path --strict` 通过。
      验证：命令退出码为 0。

## Workflow follow-up

- 归档本变更（`openspec archive 2026-10-27-fix-font-file-load-path -y`），把
  `font-selection` 的新增要求并入 `openspec/specs/font-selection/spec.md`。
