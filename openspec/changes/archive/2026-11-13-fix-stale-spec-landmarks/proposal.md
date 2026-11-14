# Proposal

## Why

规范正文里仍有一批实现落点与当前工作树不一致，违反 `upstream-alignment` 的
「规范内的路径指针必须指向现存文件」：

- `cjk-rendering` 的 Purpose 仍写「重建逻辑为 `native/src/android/ffi.rs` 的
  `rebuild_font_pipeline`」，该函数已随 `702e5b6e` 的 JNI 模块拆分删除。现行实现让
  字体面、字号、光栅缩放与 CJK 回退顺序就地作用于活动字体管线，不重建管线。
- 同一能力里「字体管线重建统一失效实例缓存并请求新帧」的载体（重建入口）也已不存在，
  要求本身落空；而它当初要防的「清缓存 + 置脏各自复制」仍然成立——六处入口现已收敛到
  `RenderState::invalidate_glyph_instances`，规约应改述为这条现存实现。
- `pty-lifecycle` 两处把 `read_error_action` 与 `exit_signal_target` 指到 `session.rs`，
  实际分别在 `native/src/terminal/session/spawn.rs` 与 `.../lifecycle.rs`。
- `upstream-alignment` 五处 `internal.rs` 指针未跟上 `internal/` 目录拆分；研究台账行
  的 `session.rs` 按帧上限实际在 `session/output.rs`；`installPtyWriteQueue` 已改名为
  `Bridge.ptyWriteQueue` + `TerminalSurface.bindActivePtyWriteQueue`。
- `render-loop-scroll-cadence` 用 `all_text` 指代现行 `read_all_text` 与其缓存
  `cached_all_text`。

这些都不是行为变更，而是让规约重新可解析：读者按图索骥必须落得到现行代码。

## What Changes

- `cjk-rendering`：Purpose 改写为现行「就地修改活动字体管线」的实现；原
  「字体管线重建统一失效实例缓存并请求新帧」RENAMED 并 MODIFIED 为
  「字面与度量变更统一失效实例缓存并请求新帧」，载体写明
  `RenderState::invalidate_glyph_instances`，同时保留「判据留在调用点」的口径。
- `pty-lifecycle`：两处实现落点订正到 `session/spawn.rs` 与 `session/lifecycle.rs`。
- `upstream-alignment`：五处 `internal.rs` 指针订正到 `internal/` 下的具体文件；
  研究台账行里的 `session.rs` 与 `installPtyWriteQueue` 订正为现行实现名。
- `render-loop-scroll-cadence`：`all_text` 订正为 `read_all_text` 与
  `cached_all_text`。
- 同步订正 Android `Bridge.kt` 两处「原生重建字体管线」的过时注释：`setExtraFontPaths`
  现行实现只登记路径列表，不重建管线。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `cjk-rendering`：字形缓存失效要求改述为现存单一实现，Purpose 去掉已删除的重建入口。
- `pty-lifecycle`：函数落点订正。
- `upstream-alignment`：台账行里的实现落点订正（不改变任何判定）。

## Impact

- `openspec/specs/{cjk-rendering,pty-lifecycle,upstream-alignment,render-loop-scroll-cadence}/spec.md`
- `openspec/changes/2026-11-13-fix-stale-spec-landmarks/`
- `android/app/src/main/java/terminal/emulator/bridge/Bridge.kt`（仅注释）
- 源码行为：无变化。
