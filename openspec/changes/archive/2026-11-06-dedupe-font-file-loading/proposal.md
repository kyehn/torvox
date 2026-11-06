# Proposal

## Why

`fonts.xml` 的 `lang` 块按字重重复声明**同一个**字体文件：真机 `zh-Hans` 块把
`NotoSansCJK-Regular.ttc` 按 weight 100..900 声明九次（AOSP 的 `fonts.xml` 即此形状，
一字重一个条目、条目指向同一份 TTC 的不同面）。

`font_db::load_font_database` 把该块的文件名逐个解析成路径后交给 `load_files` 装库，
而 `load_files` 不去重——同一份 TTC 于是被装进字体库九次。

后果是资源按倍数膨胀：一份约 19MB 的 CJK TTC 被 mmap 并解析九次，字体库里多出九倍
重复面。`font_db::find_glyph_anywhere` 的全库扫描逐面走 `with_face_data` 重建
charmap，重复面把这条本就以毫秒计的路径再放大九倍。`font-selection` 规格要求
「渲染侧字体库只常驻三族」且「面数等于等宽主字体 + 符号族 + 区域族」，重复装入
直接违反该条。

区域族的**补装**路径（`missing_region_fallback_faces`）已按文件名去重，同一问题
只在建库路径上出现——两处口径分叉。

## What Changes

- `font_db::load_files` 按路径去重：同一字体文件只装入一次，保留首次出现的顺序。
- `font_db::load_font_database` 把主字体、符号族与区域族的文件合成一份清单后统一
  装入，去重因此覆盖跨族重复（同一文件被两个族声明时也只装一次）。
- 规约 `font-selection` 增补一条要求：同一字体文件在渲染字体库中 MUST 只装入一次。

## Capabilities

### Modified Capabilities

- `font-selection`：渲染字体库按文件去重装入。

## Impact

- Rust：`native/src/render/font/font_db.rs`（`load_files`、`load_font_database`）与其
  宿主单测。
- 行为变化仅限建库：CJK 设备的字体库面数从「区域族 × 9」降到「区域族 × 1」，
  渲染字形与回退链不变（同一文件本就只有一份面，重复装入只是多出同族同字形的
  拷贝）。
- 不改依赖、不改协议、不改设置项。
