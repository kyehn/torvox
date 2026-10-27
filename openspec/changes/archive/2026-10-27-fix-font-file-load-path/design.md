# Design

## Context

真机 logcat（`logcat_2026-10-27_15-34-48.txt`）里字体相关的时间线：

```text
15:25:42.152  FONT_LOAD: set 1 extra font paths      ← setExtraFontPaths
15:25:42.166  FONT_RASTERIZE_ASCII: before=0 after=94 ← 重建管线（约 15ms）
15:25:42.167  setExtraFontPaths: registered extra font paths
15:25:42.188  FONT_LOAD: 2 extra font paths          ← loadFontFile 追加路径
15:25:42.209  FONT_RASTERIZE_ASCII: before=0 after=94 ← 又一次重建管线（约 21ms）
15:25:42.336  FONT_INDEX: 187 families indexed       ← 全量族枚举（约 127ms）
15:25:42.380  loadFontFile: … -> family Maple Mono Normal NF CN
```

三处耗时各有成因：

1. **族索引枚举（127ms）**：`font-selection` 既有要求「完整字体族索引 MUST 只在显示
   字体列表时构建，渲染路径 MUST NOT 触发该枚举」。但 `loadFontFile` 的实现是：另起
   一个 `fontdb::Database` 探测文件取族名 → 追加额外路径 → 重建渲染管线 → 按族名
   `set_font_family`。新管线里的库是 `load_font_database()` 的缓存副本，只含
   fonts.xml 等宽主字体、符号族与区域族，不含刚追加的用户字体文件，于是
   `set_font_family` 走「按需装入」分支 → `load_family` → `family_files` →
   `family_index` → 解析 `fonts.xml` 声明的每一个字体文件。真机上正是这 127ms。
2. **两次管线重建（约 36ms）**：`setExtraFontPaths` 与 `loadFontFile` 各重建一次。
   重建即 `FontPipeline::new`：重分配图集、重光栅 94 个 ASCII 字形、重探 CJK 回退层、
   重装 45 个区域回退面。渲染常驻字体库不读额外字体路径，重建得到的库与重建前同一
   份——重建对这些路径毫无作用。
3. **重建的副作用**：`FontPipeline::new` 把 `font_id` 复位成 fonts.xml 的 monospace 族、
   把 `raster_scale` 复位成 1.0。于是「登记字体目录」与「装载字体文件」都会把用户选的
   主字体与光栅缩放一并抹掉：用户换字体后字号突然改变，主字体退回系统默认。

另外，fontdb 的字体库只增不删：同一路径的字体文件被替换后，旧面仍在库里，按族名
扫库取首个会选回旧面。现状靠「重建管线」把旧面清掉；一旦不重建，就必须另想办法
保证按族名选中的是最新装入的那份。

## Goals / Non-Goals

- Goal：`loadFontFile` 与 `setExtraFontPaths` 不再重建字体管线。
- Goal：渲染路径不再触发全量族索引枚举。
- Goal：字体文件被替换后，按族名选中必须用新装入的那份面。
- Goal：字体源确实变化时，实例缓存与重绘请求与 `setFontFamily` 同一口径。
- Non-goal：不改字体列表的内容与顺序（`family_index` 的枚举范围与去重规则不动）。
- Non-goal：不改「库内没有该族时按需装入」的既有链路——设置页从列表选中的族仍走
  `load_family`，那里需要族名到文件的映射，本就不该由渲染常驻路径承担。

## Decisions

### 决策一：`loadFontFile` 直接装进当前渲染字体库

文件路径已知，族名就装在文件里。直接 `FontPipeline::load_font_file` 一次即可同时
得到族名与库内的面：省掉「另起一个库探测」的重复解析，也让族在按族名选中时已在库内，
不再回落 `load_family` → `family_index`。

- 备选「探测与装入分两步」：同一文件被解析两遍，且探测用的库与渲染用的库不是同一个，
  族名与面 id 对不上号。
- 备选「探测后把路径登记成族名 → 文件的映射，让 `load_family` 直接命中」：多一份
  全局映射要维护（族名可被替换，映射会过期），而直接装库已经让按需装入这条路径在本
  场景下不再被触发。

### 决策二：`setExtraFontPaths` 不重建管线

额外字体路径只服务两处：字体列表枚举（`family_index` → `user_font_files`）与选中时
按需装入（`load_family`）。渲染常驻字体库不读它们（`load_font_database` 的注释与
实现都是如此），重建出来的库与重建前同一份，重建既无收益，又复位主字体族与光栅
缩放。删除重建后这条路径变成纯粹的「登记」。

### 决策三：管线记住用户选文件装入的面

`FontPipeline` 新增「族名（小写）→ 面 id」的记录：`load_font_file` 装入时记下本次
选中的面（文件中首个有族名的面），`set_font_family` 优先取该记录。

fontdb 的字体库只增不删，同一路径的文件被替换后旧面仍在库里，按族名扫库取首个会
选回旧面——用户看到「换了字体却不生效」。记录面 id 就绕开了这个坑：重新装入会写入
新的面 id，按族名选中自然改用新面。

- 备选「按族名扫库取最后一个匹配面」：TTC 里同族的多字重面（Regular/Bold）会因此
  选中粗体面，主字体不再是常规字重。
- 备选「字体文件被替换时重建管线」：把要删掉的重建又请回来，且重建的代价正是本次
  要去掉的。
- 备选「把族名 → 文件路径登记成全局映射」：多一份要维护的过期状态。

面 id 是 fontdb 的 slotmap 键，库被整体替换后旧 id 指向别的面或失效，故取用前确认
它仍在库里（`Database::face` 对外来 id 返回空）。

### 决策四：字体源变化时失效实例缓存

`loadFontFile` 换字体后必须让渲染器丢弃按 UV 缓存的实例：图集 UV 与行高全部过期，
同尺寸下旧判定仍会判兼容。判据与 `setFontFamily` 同一口径——比较 `font_id` 前后是否
变化，变化则清实例缓存并置脏。

### 决策五：删除 `rebuild_font_pipeline`

两处调用点移除后该函数无调用方。它同时承载「替换管线」与「失效缓存与请求重绘」两
件事，后者已由决策四在 `loadFontFile` 里按变化判定完成，前者已无存在理由，故整体
删除，不留死代码。

## Risks / Trade-offs

- [字体文件被替换后旧面仍留在库里] → 旧面只是元数据（`Source::File` 不驻留字体
  数据），代价可忽略；且记录机制保证它永不被选中。
- [管线被整体替换后记录的面 id 失效] → 取用前用 `Database::face` 确认；且本变更
  之后已无替换管线的路径。
- [族索引仍在设置页路径上构建] → 这是既有要求：族名只能从字体文件的 name 表得到，
  列表显示时构建一次并缓存到进程结束。
- [`load_font_file` 取族名的面从「首个面」改为「首个有族名的面」] → TTC 首面可能是
  无名子集，按首个面取会得到空串，用户选字体时无从匹配。
- [Android 专属代码无法在宿主跑单测] → 行为由管线层用例钉住（装入后按族名选中、
  重新装入后改选新面），JNI 层只做转发与缓存失效。

## Migration Plan

无数据迁移。改动是纯代码内的调用顺序与状态记录调整，回滚即还原本次提交。

## Open Questions

无。
