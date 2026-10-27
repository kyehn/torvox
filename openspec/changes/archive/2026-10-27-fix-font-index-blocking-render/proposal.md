# Proposal

## Why

真机 logcat 显示 `FONT_INDEX: 187 families indexed` 一次要 127ms（读完全部
`fonts.xml` 声明字体文件的 name 表）。该枚举按 `font-selection` 的既有要求只在显示
字体列表时构建，但**构建的位置**没有被约束：两条路径都在持有全局 `RENDER_STATE`
互斥锁时构建它。

- `listFontFamilies`（设置页打开字体选择器）：持锁调用
  `FontPipeline::list_monospace_fonts` → `font_db::family_index` → 127ms 全量枚举。
- `setFontFamily`（会话启动应用已存字体族、从列表选中一个族）：持锁调用
  `FontPipeline::set_font_family` → 族不在渲染常驻三族里 → `font_db::load_family`
  → `family_files` → `family_index` → 同一个 127ms 枚举。

渲染线程每帧都要取 `RENDER_STATE`（阶段 1 与阶段 3），故这 127ms 里终端完全冻结：
用户看到的是「打开字体选择器时终端卡住」「会话起来后终端先卡一下」。

## What Changes

- `listFontFamilies` 在渲染状态锁外构造族列表：族索引是进程级缓存，与渲染状态无关。
- `setFontFamily` 改为两段式：持锁判定族是否已在渲染字体库中 → 缺族时**解锁**解析
  族文件列表（可能首次构建族索引）→ 重新持锁装入族文件并选中。
- `FontPipeline::set_font_family` 接受调用方给出的族文件列表，按需装入不再自行触发
  族索引枚举；`FontPipeline::family_resident` 给出「族是否已在库中」的判定。
- `font_db::load_family` 拆成「解析族文件列表」（`family_files`，锁外）与「装入族
  文件」（`load_family_files`，锁内）两半。
- 族已在库中时（常驻三族、用户选文件装入的面）不解析族文件列表——「装载字体文件
  不触发族枚举」的要求由这条快径守住。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `font-selection`：「字体列表仅设置页按需构建」补充「族索引 MUST NOT 在渲染状态锁内
  构建」；「字体列表取自渲染侧字体库」补充列表构造与渲染状态锁的关系；新增「按需装入
  族时族文件列表必须在渲染状态锁外解析」一条要求。

## Impact

- `native/src/render/font/font_db.rs`：`load_family` 拆为 `family_files` 与
  `load_family_files`；新增 `family_display_names`；补宿主构建的 `family_files` 桩。
- `native/src/render/font/pipeline.rs`：`set_font_family` 增加族文件列表入参；新增
  `family_resident`；`list_monospace_fonts` 收窄为宿主构建专用。
- `native/src/android/ffi/font.rs`：`listFontFamilies` 与 `setFontFamily` 调整持锁
  区间；`loadFontFile` 跟随新签名。
- `native/src/render/font/tests/`：既有 `set_font_family` 调用点跟随新签名；新增
  `family_resident` 用例。
