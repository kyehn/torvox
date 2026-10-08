## Why

重复块扫描（归一化后连续 8 行相同）在 `native/src/android/ffi.rs` 找到一处**跨约 100 行的同文件重复**：字体管线重建序列在 `loadFontFile` 与 `setExtraFontPaths` 各写了一遍。

两处已经分叉，且分叉出的正是缺陷：

- `loadFontFile` 重建后执行 `cell_cache = None` 与 `dirty = true`，并就地注明
  「管线整体替换：旧实例 UV 全部失效。这两步必须无条件执行——提前返回会留下
  『新管线已装、旧实例缓存仍在、且没请求新帧』的三重不一致。」
- `setExtraFontPaths` 只重建，**没有**清缓存、**没有**请求新帧。

该路径可达且承载已声明功能：`DESIGN.md` 要求 Termux 字体目录存在时加入扫描路径，
`TerminalRuntime` 启动时即调用 `bridge.setExtraFontPaths(...)`。一旦该目录存在，
新管线已装载而旧实例的图集 UV 仍被复用，渲染结果与新字体不一致，且不会主动请求重绘。

## What Changes

- 抽出 `rebuild_font_pipeline(render_state)`，按 `RenderState` 所在层（`ffi.rs`）放置，
  统一「按当前图集尺寸与字号重建管线 + 失效实例缓存 + 请求新帧」三步，两处调用点共用。
- 语义差异保留在调用点：`loadFontFile` 重建后再设字体族，`setExtraFontPaths` 不设。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `cjk-rendering`：字体管线重建 MUST 同时失效实例缓存并请求新帧，所有重建入口一致。

## Impact

- `native/src/android/ffi.rs`：新增共用重建函数，两处调用点改用。
