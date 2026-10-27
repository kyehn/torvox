# Proposal

## Why

真机 logcat 显示：`loadFontFile` 在启动路径上耗掉约 190ms（15:25:42.188 → 42.380），
其中 127ms 花在枚举全部字体族（`FONT_INDEX: 187 families indexed`）上。该枚举按
`font-selection` 的既有要求只应在设置页显示字体列表时发生，渲染路径不得触发——
现状是 `loadFontFile` 另起一个字体库探测族名、随后重建渲染管线，管线里的族不在库内，
`setFontFamily` 便按族名回落到「按需装入」路径，触发全量枚举。

重建管线本身也是空转：渲染常驻字体库不读额外字体路径（见 `load_font_database`），
重建得到的是同一份库；而重建会把主字体族复位成 `fonts.xml` 的 monospace 族、把光栅
缩放复位成 1.0——用户换字体后字号会突然改变。

## What Changes

- `loadFontFile` 直接把字体文件装进当前渲染字体库并取其族名，不再另起字体库探测、
  不再重建字体管线；族已在库内，按族名选中不再触发族索引枚举。
- `setExtraFontPaths` 不再重建字体管线：额外字体路径只服务字体列表枚举与按需装入，
  渲染常驻字体库不读它们，重建既无收益又复位主字体族与光栅缩放。
- 字体管线记住「用户选文件装入并选中的面」：同一路径的字体文件被替换后重新装入，
  按族名选中改用新装入的那份面（fontdb 的字体库只增不删，旧面仍在库里）。
- `loadFontFile` 在字体源确实变化时失效实例缓存并请求重绘，与 `setFontFamily` 同一口径。
- 删除已无调用方的 `rebuild_font_pipeline`。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `font-selection`：明确「字体列表仅设置页按需构建」这条要求覆盖字体文件装载路径，
  并新增「字体来源未变时不得重建字体管线」「字体文件被替换后按族名选中必须用新装入的
  面」两条要求。

## Impact

- `native/src/render/font/pipeline.rs`：管线新增「族名 → 用户选文件装入的面」记录；
  `set_font_family` 优先取该面；`load_font_file` 取首个有族名的面并记录之。
- `native/src/render/font/tests/loading.rs`：新增两条用例钉住上述行为。
- `native/src/android/ffi/font.rs`：`loadFontFile` 改为直接装入当前字体库；
  `setExtraFontPaths` 不再重建管线。
- `native/src/android/ffi.rs`：删除 `rebuild_font_pipeline`。
