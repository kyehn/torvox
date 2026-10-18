# Tasks

基线：`docs/STATUS.md` 当前的 7 条。每条按「根因 → 落盘 → 可运行检查 → 设备证据」
四段填，缺哪段就写缺哪段，不写「大概修了」。

## 为什么多次解决失败

三次返工都停在现象上，没有动现象背后的机制。可核对的有三处：

- 粘贴丢失曾被观察到，处置是**补日志**：`TerminalSurface.hideSelectionMenu` 的注释至今写着
  「PasteButtonInstrumentedTest 在 CI 同款几何下曾整段丢失粘贴动作而日志里查不到是谁关的」。
  日志补上了，问题「谁关的」没有人回答，同一个 bug 又活过一轮。
- 同一现象的另一半被当成「已修」：菜单和手柄覆盖层都是铺满终端表面的子系统窗口，覆盖层
  消费其边界内的全部触摸。二者重叠时菜单看得见、点不到，用户读作「菜单消失」。没有把
  「点击最终到达哪里」写成断言，测试只在菜单存在时去点它。
- 规范文档被当成不可动的约束：输入法位移的规格写着「MUST NOT 取光标行」，于是「prompt
  满行时光标不可见」被判成规范已定的行为，只改了内容下沿。实际是拿光标行**替换**下沿才会
  退化，取二者的较低者同时满足两条理由——该改规格，而不是绕开症状。

- 设备上验的是旧的原生库。`android/app/src/main/jniLibs/` 是 `.gitignore` 里的本地产物，
  gradle 只打包、不编译它（编译在 `scripts/build-android-libs.nu`，CI 跑 arm64-v8a +
  x86_64 两个 ABI）。改了 `native/` 却没重跑该脚本，`assembleDebug` 打出来的 APK 仍旧带着
  上一次的 `libnative.so`，设备上看到的还是旧行为。方块光标就是这样被误判的：源码早已按
  整格几何，设备上量到的却是旧的 22×27（行高 45），重跑脚本后同一位置变成 22×45。
  凡依赖 native 渲染的设备验证，先重跑脚本并核对 `jniLibs/**/libnative.so` 比改动更新。

共同点：结论来自读代码、读文档，而不是来自一次能证伪的实验。本轮把每条都落到一个会红/
会绿的观察上，并且先做实验再下结论（例如覆盖层假说先由「点击前后只有一次 detach」的日志
排除掉「菜单被谁关掉」，再由覆盖层窗口尺寸等于整个表面的截图坐实）。

## 一、零覆盖项

- [x] 重命名文件卡死
      - 根因：仓内无缺陷。客户端路径（树 URI + 平台 `renameDocument`）在真机上改名落盘、
        旧名消失、新 docId 立刻可寻址。此前标成「零覆盖」是覆盖缺口而非缺陷：仪器套件有
        创建、写回、复制、移动的往返，唯独没有重命名往返，于是改没改都不会红。真机上
        对话框关掉、名字不变、无报错，正是客户端没改名字时框架的静默收场（提供者按
        `documents-provider` 的「重命名到自身是空操作」回 `null`，平台把同一 URI 交回）；
        手工复现时注入的按键始终没进到对话框输入框（IME 日志里该窗口 `inputType=NULL`），
        没走到改名分支，也顺带印证了这条路径：文本没变，界面就只会「什么都没发生」。
      - 落盘：提供者未改（行为已符合契约：同名字返回 `null`、冲突走 `uniqueChild`），
        本轮补的是可运行检查。
      - 可运行检查：`DocumentsProviderInstrumentedTest#tree_uri_rename_round_trip`，
        经真机 `ContentResolver` 走树 URI 断言：① 改名必须返回新 URI、旧名消失、内容不变；
        ② 新 docId 立刻可 `getType`/`openInputStream`；③ 名字没变时平台回同一 URI、
        磁盘不得多出副本。
      - 设备证据：`am instrument -e class terminal.emulator.DocumentsProviderInstrumentedTest`
        真机全绿 `OK (10 tests)`。
      - 未覆盖：DocumentsUI 改名对话框本身的端到端点击（注入按键进不到该输入框，见根因）。
- [x] 输入光标大小形状不一致、移到文本时字体下降溢出
      - 根因：光标几何锚在格内字形位图上。方块光标取「整格宽 × 位图高」，空单元格取
        `'M'` 位图、文本格取该字形位图，两者不等；Bar/Underline 更进一步，起点也按
        位图顶边算，降部字形会把标记带出行界。
      - 落盘：`native/src/render/cell_builder.rs` 的方块分支只换背景色，四边形恒取
        `[单元宽 × 列跨度, 单元高]`、顶边恒为单元顶边；Bar/Underline 一并改锚单元盒
        （竖线纵向铺满整格，下划线贴单元底边），空单元格路径不再查 `'M'` 参考字形，
        整条分支不再读字体。
      - 可运行检查：`render::tests::cursor_geometry_anchored_to_cell_box`，对
        Block/Bar/Underline × 空单元格/`g`/`M` 断言左边界 = 0、方块为整格、竖线纵向
        铺满整格且窄于单元、下划线宽度为整格且底边 = 单元底边。旧断言写的是
        `quad_size[1] <= cell_height`，跟随位图同样能过，所以一直没红。
      - 设备证据：重跑 `scripts/build-android-libs.nu x86_64` 后重装（`jniLibs` 是
        `.gitignore` 里的本地产物，gradle 不编译它）。`$ ab` 上量到方块 22×45 像素、
        `x=[88,109]`，正好一格；`$ gjpqy` 的降部墨迹止于行内第 43 像素；用
        `printf %b '\033[5\040q'`（DECSCUSR）切到竖线后量到 6×44 像素、x=[44,49]，
        即第二列整格高，未越出行界。
      - 未覆盖：光标格的设备像素断言没有进 CI 套件（要抄产品几何常量进测试，且依赖
        模拟器时序），设备量测仍是手工步骤。
- [x] prompt 占满整行时输入法光标不可见（termux 落在下一行）
      - 根因：位移只按最后一个有字形的行的下沿算，而 shell 的块光标落在该行的下一行，
        位移到位后光标仍在键盘之后。
      - 落盘：`computeImeSurfaceShift` 增加必需参数 `cursorBottomPx`，取内容下沿与可见光标格
        下沿的较低者，仍受键盘高度上限约束；`applyImeShift` 用 `renderedCursorPacked()`
        （`< 0` 表示不可用）经 `computeContentBottomPx` 求光标格下沿。
      - 可运行检查：`ComputeImeSurfaceShiftTest` 新增「光标落在最后一个有字行的下一行」，
        既有用例继续约束不得因此过移。
      - 设备证据：`ImePopupPixelInstrumentedTest#contentManyImePopupMovesUpBottomIdentical`
        在改动后通过（内容满屏时终端上移、上移前后底部内容相同、稳定后无闪烁）。
      - 未覆盖：光标格的像素位置没有设备断言——该断言需要把产品几何常量抄进测试，
        会变成易碎测试；差距由单元测试的公式与设备用例的实测位移共同兜住。
- [x] prompt 占满整行时输入内容后全部退格不留下空行
      - 结论：今天的代码上该行为正确，两种形状都实测过，没有残留空行。位移公式在内容退回一行
        时归零（`ComputeImeSurfaceShiftTest#single row prompt does not move`、
        `empty viewport does not move` 已钉住），退格只发退格字节、不重写整段
        （`ComposingDiffTest#contract_to_empty_is_all_backspaces`，用「世界」两个码点断言 2 次退格）。
      - 设备证据：① 先在默认提示符下键入 190 个字符（提示符行被占满、光标停在行尾），再连按
        200 次退格——光标回到 `$` 提示符所在的第一行，其后无残留行、无空白行；② 把 `PS1` 设成 60 个
        `p`（提示符自身占满整行）后键入 `abc` 再退格 3 次——光标回到提示符末尾，下面没有多出空行。
        两次都在键盘可见（即 `computeImeSurfaceShift` 生效）的状态下截图核对。
      - 未覆盖：没有自动化的像素级断言——这条是观感，判据落在位移公式与组字退格的单元用例上。
- [x] 启动动画图案模糊（含「应用图标被意外更改」）
      - 根因：启动屏一度不取自适应前景图形，又把 `launcher_foreground` 整体缩到 0.62 去适配，
        于是同一图案有了两份缩放版本，桌面图标跟着一起被改样。
      - 落盘：`windowSplashScreenAnimatedIcon` 直接指 `@mipmap/ic_launcher`（自适应图标本身，
        平台只取前景层）；父主题 `Theme.SplashScreen` 不声明
        `windowSplashScreenIconBackgroundColor`（不垫圆形底板）；`launcher_foreground` 保持原始尺寸。
      - 可运行检查：`SplashIconRobolectricTest#splashIconIsTheAppIconItselfAndItsArtworkIsNotShrunk`
        ——解析主题属性、必须是 `AdaptiveIconDrawable`、底色 alpha 必须为 0、前景按 4 像素/单位
        渲染后最远半径落在 40~60 单位且覆盖非空。
      - 设备证据：该用例以 `--rerun` 强制重跑实测 1 test / 0 failures（增量缓存会假绿，故必须重跑）；
        `themes.xml`、`AndroidManifest.xml`、`res/mipmap/ic_launcher.xml` 三处复核一致。
        启动屏存活到首帧为止，截图往返抓不到它，像素级判据以该渲染用例为准。

## 二、复验项（上一变更已收敛，需在今天的代码上重测）

- [x] 简体中文退格延迟
      - 根因：组字每次变化都会把光标锚点回报给输入法，而该回报曾走同步 VT 查询——主线程要等
        一整帧（PTY 回显 → 渲染帧 → VT 循环节拍）才拿到光标位置。拼音逐键回退就是逐键
        组字变化，于是「退格明显缓慢」按帧计。
      - 落盘：锚点改读帧内已发布的光标（`reportComposingCursorAnchor`），不再进同步查询；
        组字增量本身是纯前缀 diff（`ComposingDiff.reconcile`），退格只发退格字节、不重写整段。
      - 可运行检查：`CjkComposingLatencyTest#composing_backspace_does_not_wait_on_cursor_anchor_query`
      ——注入逐键 `setComposingText` 回退（`zhongguo` → …），量该回调在**主线程**上的耗时，
      分「请求光标监视」与「不请求」两组各 20 个样本，两组 p50 之差必须低于 8ms（半帧）。
      同组还有 `ComposingDiffTest` 钉住退格语义（「世界」两码点＝2 次退格、emoji 算一次）。
      - 设备证据：今天的代码上单跑该类真机通过 `OK (1 test)`，14.9s。此前两次红都是本机
        UiAutomation 被占用（驱动设备的 mobile-mcp 常驻进程持有），与产品无关；
        释放该进程后同一命令即绿。
      - 未覆盖：模拟器里没有中文输入法，手感本身无法复现；用例注入的是输入法回调，
        量的是回调耗时，不是真实拼音候选窗的行为。
- [x] 文本选择卡顿、不跟手，拖动选区或全选后点「复制」无效
      - 根因（设备日志）：菜单与手柄覆盖层同时存在。覆盖层窗口尺寸等于整个终端表面、
        `isOutsideTouchable=false`，按自身注释消费边界内全部触摸，落在菜单条目上的点击被它
        吃掉；当轮日志里长按到测试收尾之间只有一次 `hideSelectionMenu(detach)`（测试自身的
        收尾），证明菜单一直在，是点击没有到达。
      - 落盘：菜单显示前收起覆盖层，菜单关闭后按当前选区恢复；内容未变的重复显示复用已显示
        的窗口（原先每次都 dismiss + 重建，`hideSelectionMenu(showSelectionMenu)` 出现在
        Espresso 点击前 0.5s，点击落在被丢弃的旧实例上）。复用前以
        `contentView.isAttachedToWindow` 判存活——只看 `isShowing` 会把菜单永久留在已消失的
        窗口上，该回归本轮已实测到并由该判据修掉。
      - 可运行检查：`PasteButtonInstrumentedTest#pasteMenuTypesClipboardIntoShell`、
        `SurfaceLossRecoveryInstrumentedTest#selectionMenuSurvivesSurfaceRebuild`。
      - 设备证据：日志出现 `粘贴入队 11 字节 -> 会话 1`，粘贴用例通过；单独运行
        `SurfaceLossRecoveryInstrumentedTest` 全绿（0 失败）。此前两轮的红是模拟器被并行负载
        拖慢留下的环境噪声（`shell prompt 未出现`，帧耗时 850ms），不是产品行为。
- [x] 文档提供者：创建、复制/移动、保存超时三个子项
      - 结论：三个子项在今天的代码上由真机仪器用例覆盖并通过（同一次 `am instrument` 报
        `OK (10 tests)`），单元层 `DocumentsProviderTest` 61 test / 0 失败。
      - 创建：`child_documents_round_trip_create_delete`、`resolver_create_then_modify_round_trip`、
        `resolver_create_directory_round_trip`、`tree_uri_nested_create_and_write_back_round_trip`
        （嵌套目录里建文件，再按相对根的 docId 回查、写回、删除）。
      - 复制/移动：`tree_uri_modify_copy_move_round_trip`（源保留、副本落盘、移动后旧父目录不再
        有条目，重名冲突按 `uniqueChild` 落成 `source.txt (2)`）。这条往返由设备层用例覆盖，
        单元层不再重复一份同形用例——旧变更记的「覆盖＝先删再拷没有单测」缺口由它兜住。
      - 保存超时：`external_process_write_back_lands_on_disk`（外部进程经提供者写回落到磁盘）、
        `external_process_read_succeeds_without_manage_documents`。旧变更已按设备日志确认报错
        落在客户端超时侧、仓内没有超时或 ANR 可修，本轮在今天的代码上重跑仍无该现象。
      - 未覆盖：由第三方编辑器触发的真实「另存为」交互需要一个可安装的客户端，写回只验到
        提供者边界为止。
- [x] 启动屏图案与应用图标
      - 与上面零覆盖项的「启动动画图案模糊（含「应用图标被意外更改」）」是同一条，判据不重复：
        本轮在今天的代码上强制重跑 `SplashIconRobolectricTest`，1 test / 0 失败 / 0 跳过；
        `themes.xml`、自适应图标定义与前景图形在该修复后均未再改动。
