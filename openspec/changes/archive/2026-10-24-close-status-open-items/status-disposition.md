# STATUS 四项处置证据

对应 `docs/STATUS.md` 的「未被解决的问题」四项。该文件受保护，本留档是本轮证据的
唯一去处：只写结论、用例名与计量，不写原始日志行、时间戳、PID 与设备标识。

证据采集环境：x86_64 模拟器（软件 Vulkan），应用以 debug 签名安装；仪器化用例经
`:app:connectedDebugAndroidTest` 运行，Rust 用例经 `cargo test --lib` 运行。

## 一览

| 项 | 当前状态 | 自动化判据 | 剩余条件 |
| --- | --- | --- | --- |
| 上下滑动滚动缓慢卡顿闪烁 | 根因已修（占位符扫描缓存基准改为绝对行），应用侧单帧工作已压到噪声级 | Rust `kitty_placement_query_reuses_unchanged_rows`、`kitty_placement_row_cache_survives_scrollback_viewing`；仪器化 `ScrollBehaviorQuantifiedTest.drag_loop_frame_pacing_stays_smooth` | 真机（Mali）复测帧耗时分布；模拟器为软件 Vulkan，帧率数字随宿主负载波动 |
| 文本搜索的匹配与当前匹配高亮未区分 | 行为已符合规范，并已抽成可表驱动验证的纯函数 | Rust `search_highlight_tests` 三例；`cargo test --lib search_highlight` | 无 |
| kitten icat 显示图像后 `clear` 需清两次才撤下图像 | 根因已修（ED2 同一次清除内撤下放置） | Rust `ed2_removes_kitty_placements_on_first_clear`；仪器化 `KittyGraphicsPlacementTest.clear_erases_image_on_first_ed2` | 真机上 kitten icat 客户端行为（协议序列已一致） |
| 长按选择后手柄越过对侧手柄时选区异常变动 | 根因已修（触点折算参考点抓柄时锁定） | JVM `DragTargetCellTest`；仪器化 `SelectionDragQuantifiedTest.dragging_end_past_start_keeps_finger_on_the_dragged_boundary` | 无 |

## 逐项证据

### 一、上下滑动滚动缓慢卡顿闪烁

- 根因：Unicode 占位符扫描缓存的基准错位。逐行签名本可跨帧复用，但查询时用
  「回滚长度 + 视口行」当基准，与 VT 实际给出的绝对行号整体错位，缓存恒失效，
  回看历史时每帧退化为全量重扫。
- 修复：基准改取 VT 滚动条给出的绝对行号；逐行签名只含行内布局（列、字素簇续接记号、
  前景色、下划线色），不含视口行号，纯滚动时签名不变。
- 判别力：扫描路径自记账（扫描行数、命中行数），缓存失效立刻表现为扫描行数不为 0。
  - `kitty_placement_query_reuses_unchanged_rows`：同一签名下第二次查询扫描 0 行。
  - `kitty_placement_row_cache_survives_scrollback_viewing`：视口上滚一屏后按绝对行命中，
    扫描 0 行。
- 帧节奏：仪器化 `ScrollBehaviorQuantifiedTest.drag_loop_frame_pacing_stays_smooth`
  断言「循环自身工作（门控之后、渲染 JNI 之前）单帧 ≤ 8ms」，本轮实测
  `drag_loop_pre_max_ms=5`（窗口 `pre_max=5.26ms`），逐帧打点里 `pre` 约 0.03ms、
  `poll` 约 0.1ms。
- 剩余：慢帧时间几乎全在 GPU 取帧（`SLOW_RENDER_PHASE acquire` 23–88ms），
  `encode` 约 0.2ms、`present` 约 0.5–3.6ms——模拟器是软件 Vulkan，属设备能力而非
  应用侧回归。模拟器上的整窗 p95/fps（本轮 25–41fps）随宿主负载波动，
  真机（Mali）的分布须另行复测。

### 二、文本搜索的匹配与当前匹配高亮未区分

- 状态：行为本就符合 `text-search` 规范，问题在于折算与配色内联在 JNI 入口里，
  无法表驱动验证，只能靠肉眼判断「当前在哪」。
- 修复：折算与配色抽成纯函数 `build_search_highlights`，**行为不变**：
  当前命中 alpha 255（完全不透明）且取主题前景色 `0xEEEEEE`；
  其余命中 alpha 160 且取主题选区色 `0x334455`；视口外的命中在上下两侧裁掉；
  无选中命中时全部按「其余命中」着色。
- 判据（Rust，`cargo test --lib search_highlight` 全绿）：
  - `current_match_is_distinguishable_from_other_matches`：当前命中与其余命中的颜色
    必须不同，alpha 分别为 255 / 160。
  - `matches_outside_viewport_are_clipped_on_both_sides`：视口上方的命中不得写入高亮，
    折算行号是绝对行减视口顶行。
  - `without_selection_all_matches_use_other_color`：无选中命中时不得出现「当前项」着色。
- 剩余：无。该项纯在渲染层，不依赖设备。

### 三、kitten icat 显示图像后 `clear` 需清两次才撤下图像

- 根因：ED2 清屏时 VT 侧未撤下图像放置集合。渲染侧无从知道该擦掉哪块像素，
  只能靠「整帧重绘」兜底，而累加器里的残留像素要等下一次真正的全量重绘才被抹掉，
  表现为「要 clear 两次才消失」。
- 修复：ED2 在同一次清除内撤下全部图像放置（直接放置通道 `a=T` 与 Unicode 占位符通道
  共用撤除语义）；放置被撤下即视为叠加层「本帧发生变化」，渲染走整帧重绘抹掉残留。
- 判据：
  - Rust `ed2_removes_kitty_placements_on_first_clear`：ED2 之后放置集合**立即为空**
    （不是第二次清除之后才空）。
  - 仪器化 `KittyGraphicsPlacementTest.clear_erases_image_on_first_ed2`：喂入一次 ED2 后，
    屏幕上的图像颜色像素计数归零。断言落在真实像素上——只断言内部放置列表为空不够，
    缺陷正是「VT 已删放置但渲染线程没重查、继续画缓存帧」。
  - 同轮 `kitty_placeholder_pixels=4096`：图像确实画出来了（判据非空转）。
- 剩余：真机上 kitten icat 客户端自身的行为。自动化注入的是同一套 kitty 协议序列，
  客户端的绘制时序仍需真机确认。

### 四、长按选择后手柄越过对侧手柄时选区异常变动

- 根因：触点到网格列的折算原先按「当前这根手柄是哪一侧」选参考点
  （首端柄 = 首格左缘，末端柄 = 末格右缘）。手柄越过对侧后归属翻转，参考点跟着换，
  目标列在翻转瞬间跳一格：被拖的边界反向移动一格、交叉被立刻撤销（选区塌成单格）、
  手柄与手指脱开。
- 修复：抓柄瞬间锁定参考点（`dragAnchorIsStart`），整次手势内不变；归属翻转只改
  「这根柄现在扮演首端还是末端」。快速路径与边缘滚动路径共用同一折算纯函数
  （`dragTargetColumn` / `dragTargetRow`），消除口径分裂——原先边缘滚动路径按
  「手指所在格」折算，手指进出边缘区时列会跳一格。
- 判据：
  - JVM `DragTargetCellTest`：参考点解析、单格位移、网格钳位、交叉翻转后边界归属与
    钉住、行折算与视口钳位。
  - 仪器化 `SelectionDragQuantifiedTest.dragging_end_past_start_keeps_finger_on_the_dragged_boundary`：
    被拖的边界停在手指所在格，未拖动的一侧钉在交叉前的位置。本轮实测
    `drag_cross_dragged_col=3`（被拖边界落在手指所在列），同轮
    `drag_start_edge_drift_px=0`（手柄未与边界漂移）。
  - 红绿验证：把参考点临时改回「随归属翻转」（旧行为）后，该仪器化用例转红；
    恢复锁定后转绿——判据确有判别力，不是恒真断言。
- 剩余：无。

## 附：第三份运行日志的基线归属

用户提供的第三份运行日志采自一个不含占位符绝对行缓存修复的分支，其慢帧记录属于
**修复前**基线；不能据此判定当前实现仍然卡顿。修复后的帧节奏以本留档第一项的
计量为准（应用侧单帧工作 ≤ 5ms，剩余时间在软件 Vulkan 的 GPU 取帧）。

## 附：本轮未改动的受保护文件

`docs/STATUS.md` 与 `docs/specification/` 的实际内容均未改动：前者是四项的来源，
后者是规范来源；本轮的规范增量落在 openspec 的 `text-selection` 与 `kitty-graphics`
主 spec（归档时同步），证据落在本留档。
