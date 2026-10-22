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
      - 根因（实测）：`windowSplashScreenAnimatedIcon` 曾直接指 `@mipmap/ic_launcher`（自适应
        图标本身）——平台的 `SplashscreenContentDrawer` 只在拿到自适应图标时才沿用自己的蒙版，
        于是启动屏图案与桌面图标一样被裁成图标形状；此外曾把 `launcher_foreground` 整体缩到
        0.62 去适配，结果是同一图案有了两份缩放版本，桌面图标跟着一起被改样。
      - 落盘：`windowSplashScreenAnimatedIcon` 改指**普通图形** `@drawable/splash_icon`
        （`layer-list` 只引用桌面图标同一份的 `launcher_foreground`）——平台改走「缩进内圈
        72/108 再裁」的路径，图案完整；父主题 `Theme.SplashScreen` 不声明
        `windowSplashScreenIconBackgroundColor`（不垫圆形底板，并因此按
        `splash_icon_no_background_scale_factor` 放大 1.5 倍，观感与桌面图标等大）；
        `launcher_foreground` 保持原始尺寸。桌面图标仍是 `@mipmap/ic_launcher`，两者互不影响。
      - 可运行检查：`SplashIconRobolectricTest#splashIconIsAPlainDrawableWhoseArtworkFitsTheVisibleCircle`
        ——解析主题属性、图标**不得是** `AdaptiveIconDrawable`、底色 alpha 必须为 0、按内圈缩放
        与 1.5 倍放大解析后图案落在图标区可见圆内且不小于桌面图标可见内容、不透明像素非空。
      - 设备证据：该用例以 `--rerun` 强制重跑实测 1 test / 0 failures / 0 skipped（增量缓存会假绿，
        故必须重跑）；`themes.xml`、`AndroidManifest.xml`、`res/drawable/splash_icon.xml` 三处一致。
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
      - **补记（用户报「所有菜单按钮失效、点击菜单消失」的第二层根因）**：上面修的是
        「点击到不了菜单」，用户现象还有一半来自「菜单在点击前一刻自己挪走」。真机日志实证：
        长按选出 `world` 输出 `setSelection (1,6)-(1,10)` 后，**紧跟**一条
        `setSelection (1,6)-(1,7)` ——注入 1px MOVE 即把整词塌缩成被按单元格，菜单随选区
        几何重锚走位，手指点下去的一瞬正好点空。根因：`longPressStartX/Y` 早已记录起点却
        **从未被读取**，长按后的移动阈值判定当初漏实现（缺陷引入于 2026-07-30，早于 CI
        run 39112133317 两个半月，也是该 run 唯一失败步骤的红项）。
      - 落盘（`22625806`）：越过 `ViewConfiguration.scaledTouchSlop` 前 ACTION_MOVE 一律
        不动选区，越过后才扩选。
      - 可运行检查：`VisualInlineVerificationTest#verifyWordSelectionPositions`（修复前
        连续两次独立运行稳定失败 `diff=160px`/`107px`，修复后 0 failed）；
        `SelectionDragQuantifiedTest` 与 `SelectionEspressoTest` 两条回归线全绿。

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
      - **补记（用户报「路径已存在」的根因，此前几轮修的是别的层）**：前几轮修的是提供者
        **自己**的重名策略（`uniqueChild` 改名而非硬失败、拒绝改到自身）。但用户看到
        「路径已存在」的判定根本不在提供者里——AOSP 取证：该提示（`name_conflict`）在整个
        `packages/apps/DocumentsUI` 全树只有一处使用（`RenameDocumentFragment` 的本地预检），
        数据源是目录列表游标的**一次快照**（`DirectoryLoader` 把内容观察者注册在
        **游标**上，`cursor.registerContentObserver`）。因此「提供者写盘成功但客户端仍拒」
        的充要条件就是游标没刷新，而游标刷新只取决于 `setNotificationUri` 与 `notifyChange`
        是否落在同一个 URI 上：此前查询端注册客户端原样传来的 docId（真机为带树根前缀的
        `terminal_home/…`），广播端发行行内无前缀 docId，两者是**兄弟** URI，平台按 path
        段下降匹配永不命中。客户端于是拿终端侧已删条目的幽灵行当已占用名字——这也解释了
        「选择覆盖就能成功」：绕过本地预检即可。
      - 落盘（`a1b85692`）：`setNotificationUri` 与全部 `notifyChange` 调用点的 docId 一律
        由 File 反推为行内无前缀形式；`bf8f7ebb` 另补监视事件集缺 `MODIFY`（终端侧 `>>`、
        `sed -i` 只改内容时元数据列停在旧值）与按查询刷新的 LRU 访问序。
      - 可运行检查：`DocumentsProviderInstrumentedTest` 新增「前缀寻址的列表游标收得到终端侧
        广播」（走真实 `ContentResolver` + inotify，断言 `terminal_home/sub` 寻址的游标在
        终端侧落盘后收到通知）与「追加内容触发通知」两例。
      - 设备证据：修复后单跑两例各 `tests="1" failures="0"`；全量仪器套件 200 例 0 failed；
        全量单元测试 579 例 0 failed。真机日志显示一次前缀寻址删除成功，实证该路径真实存在（故当时窗口内无失败现场）。
      - 剩余两条仍未定案（**不假装已解决**）：①「保存修改文件超时」按既有台账落点在客户端
        超时等待侧，本轮该日志窗口内无 create/rename/copy 现场；②「重命名文件卡死」与本次
        修的本地预检同源（预检拒改后提交无响应），但同样缺真机样本。两条都需一份含
        create/rename 操作的新日志（`trace()` 打点已在位）才能归因。
      - **用户第二次上传的日志（`运行日志`）**：位于
        `origin/user-branch` 分支，**未合入 main**；采集时刻**早于** URI 归一修复，故它记录的是
        修复前的行为。该版本含 `trace()` 打点而无归一修复，正好是一份「有观测、无修复」的
        对照样本：该窗口内提供者侧**只有一条删除请求**，**没有任何 create/rename/copy 请求
        到达提供者**——与「『路径已存在』的判定
          在客户端本地预检、操作根本没走到提供者」这一根因判断一致；也说明上一轮台账里
          「该次删除成功，故窗口内无失败现场」的措辞不够准：真正的原因是这个窗口只做了删除。
        · 前缀寻址（`terminal_home/…`）在真机路径上确认存在，且删除 3ms 完成、无阻塞。
        · **不能**用来验证本轮修复：采集时刻早于全部提供者修复。
      - **新发现（未定案，留给下轮）**：同一份日志里，多数渲染循环窗口出现长帧尖峰，
        而渲染阶段本身很快——停顿不在渲染路径，而在循环的其他环节（`waitOutput`/`pollAll`/事件派发/
        统计上报）。STATUS 当天保留的「文本选择性能需要提升」可能与此有关，但该日志窗口
        内没有选择操作，**不足以归因**；需要一份含滚动与选词的日志，或按
        `SLOW_FRAME`（阈值 30ms，本窗口只出现 1 次，即启动首帧）补齐循环内各段计时。
- [x] 启动屏图案与应用图标
      - 与上面零覆盖项的「启动动画图案模糊（含「应用图标被意外更改」）」是同一条，判据不重复：
        本轮在今天的代码上强制重跑 `SplashIconRobolectricTest`，1 test / 0 失败 / 0 跳过；
        `themes.xml`、自适应图标定义与前景图形在该修复后均未再改动。

## 三、shell 启动缓慢

- [x] 子进程环境缺 `PATH=$PREFIX/bin`（与 Termux-app 正常会话的环境分歧）
      - 现象与日志：用户提供的日志 `运行日志`（**该文件未入库**：
        仓库对象库里查无此名，提交历史里也没有任何 logcat 文件；本文结论取自其内容，
        文件名在同批日志的删除现场里出现过）里 `Session::spawn` 后应用侧无输出；同一窗口内
        只有一连串外部工具链的 SELinux execute 授权——慢的是引导链自身在连续执行，
        不是应用阻塞。
      - 根因（上游源码比对）：`$PREFIX/bin/login` 是引导会话的默认入口，其 termux-exec 探针
        `$SHELL -c "coreutils --coreutils-prog=true"` 依赖 `PATH` 解析 `coreutils`，失败即
        `unset LD_PRELOAD`（termux-exec 关闭）；termux-app 的 `TermuxShellEnvironment.getEnvironment`
        在正常会话（`!isFailSafe` 分支）设置 `PATH=$PREFIX/bin`，我们白名单逐项比对后唯一缺的
        就是它。缺 `$PREFIX/bin` 时由 shell 自带的默认 PATH 兜底，用户包装脚本里
        `command -v nix` 一类按名探测全部落空，启动链走「找不到 → 兜底」分支。
      - 证据（设备，本机模拟器）：改造前会话子进程 envp（`/proc/<pid>/environ`，19 项）无
        `PATH`；会话内探针脚本输出 `$PATH` 只有 mksh 自带的系统默认，`$PREFIX/bin` 不在其中。
      - 落盘：`pty.rs::build_env` 在 `ShellEnv.prefix` 为 `Some` 时补 `PATH={prefix}/bin`；
        `prefix` 为 `None`（无引导 / failsafe）时不设置，保留宿主默认——与 Termux-app 的
        正常 / failsafe 两分支语义对齐。`DESIGN.md` 白名单与 `shell-env` spec 同步更新。
      - 可运行检查：`build_env_sets_path_to_prefix_bin`（有 prefix 时取值恰为 `{prefix}/bin`）、
        `build_env_without_prefix_omits_prefix_vars`（无 prefix 时 `PATH` 必须缺席）、
        `build_env_rejects_unlisted_variables`（`PATH` 之外未声明变量仍被拒）。
      - 设备证据（本机模拟器，重建 `libnative.so` 后）：
        ① 存在 `files/usr/bin/{bash,login}` 时会话子进程 envp 出现
        `PATH=/data/user/0/com.termux/files/usr/bin`（随 PREFIX/TMPDIR 同现）；会话内探针
        `command -v coreutils` / `command -v bash` 均解析到 `$PREFIX/bin`（rc=0）——
        正是 `login` 的 termux-exec 探针所做的查询；
        ② 移走 `files/usr` 重启后 envp 无 `PATH`（19 项），会话内 `$PATH` 回落到宿主默认
        （`cut` 可用、`coreutils` 不可用），与 Termux-app 的 failsafe 分支一致。
      - 未覆盖：该分歧是已证实的环境不一致，但不是 22 秒的全部成因（窗口内 27 次执行说明
        引导链在跑用户工具）；加速与否需要在用户设备上复验。

## 四、触摸操作无效（yazi 等 TUI 应用）

- [x] 鼠标上报开启时轻点与拖动没有转发给会话
      - 现象与日志：yazi 等 TUI 开启 SGR 鼠标上报（`ESC[?1000h` + `ESC[?1006h`）后，
        手指操作完全无效；应用侧收不到任何鼠标序列。
      - 根因：手势层没有区分「应用在收鼠标事件」与「应用在收本地选区/滚回」——
        轻点只走本地点按路径（转成选区手势），拖动只移动本地 scrollback，
        两条都不产生会话输入。
      - 落盘（提交「鼠标上报中轻点转发左键点击、拖动转发滚轮」）：会话处于鼠标上报状态时，轻点转发为同单元格的 SGR
        左键按下+抬起（`ESC[<0;col;rowM` / `m`），单指拖动按累计位移转发滚轮
        （`ESC[<64/65;col;rowM`，每个单元格高度一次），坐标一律取手势落点
        或当前指针所在单元格；未开启上报时保持原有本地选择与滚动行为。
      - 可运行检查：`MouseReportingInstrumentedTest` 两个用例——
        `fingerTapInMouseTuiSendsLeftClickAtSameCell`、
        `fingerDragInMouseTuiSendsWheelInsteadOfScrollingScrollback`；
        spec 侧 `key-input-encoding` 已补该契约。
      - 设备证据：定稿版本连跑 5 轮（`connectedDebugAndroidTest` 单类）全部 0 失败；
        中间两轮红是脚手架自身的时序问题（arming 与断言窗口不匹配），随测试版本
        迭代消除，与产品代码无关。
      - 未覆盖：双指、长按拖拽等其余手势未定义新语义，保持原行为。

## 五、主字体已覆盖 CJK 时的「未找到后备字体」误报

- [x] 非 CJK locale 下主字体自覆盖却报错
      - 现象与日志：系统语言非 CJK（如 en-US）时，主字体本身覆盖 CJK 仍被报
        「未检测到 CJK 备用字体」。
      - 根因：`cjk_probe_char(locale)` 返回 `None` 的语义是「本 locale 没有分片代表
        字符」，实现把它当成了「主字体不支持 CJK」，于是无需回退的场景也走了
        缺字体告警。
      - 落盘（提交「主字体已覆盖 CJK 时不再误报未找到后备字体」）：无探针字符时回退层状态为 `Skipped`（无需回退）；
        覆盖判定改由字体面自身的码点覆盖（`face_covers`）驱动，
        `pipeline` 只在有探针字符且主字体确实未覆盖时才报缺字体。
      - 可运行检查：`cjk_primary_under_non_cjk_locale_reports_skipped_not_none` 等；
        今日重跑 `cargo test render::font` = **132 passed / 0 failed**。
      - 未覆盖：设置页上「不提示」的展示效果没有独立像素级用例，由 pipeline
        状态断言（`CjkFallbackState::Skipped`）覆盖。

## 六、CI build 稳定性（run 39112133317）

- [x] 仪器测试步骤失败（`reactivecircus/android-emulator-runner@v2`）
      - 现象：该 run（main，`e3554083`，08:37 创建）唯一失败步骤是第 12 步
        `reactivecircus/android-emulator-runner@v2`（（时间已移除） → （时间已移除），11m14s），
        其后 `git tag` 与 release 两步被跳过；此前七步（含 `build-android-libs.nu`、
        `build-apk.nu`）全绿。该步 script 为 `nu scripts/test-emulator.nu`，含两条命令：
        `:app:connectedDebugAndroidTest` 与 `:benchmark:connectedBenchmarkReleaseAndroidTest`。
        CI 原始日志需要仓库 admin 权限（API 返回 403），故步骤粒度是可得的最细证据。
      - 根因：`:app:connectedDebugAndroidTest` 里的
        `VisualInlineVerificationTest#verifyWordSelectionPositions` 稳定失败——
        长按选词后 `longPressStartX/Y` 被记录却从未被读取（阈值判定漏实现），
        手指微抖（注入 1px MOVE）即把整词选区 `(1,6)-(1,10)` 塌缩成 `(1,6)-(1,7)`。
        该缺陷引入于 `36043254`（2026-07-30 Kotlin 桥重构），**早于**本 run 的提交，
        故不是偶发；上一轮全量套件（200 例）恰有且仅有这 1 例红，且单独重跑连续两次
        都红，与 CI 步骤失败吻合。
      - 落盘：`22625806` 补 `ViewConfiguration.scaledTouchSlop` 守卫（见本文档「文本选择」
        与 `text-selection` 规范「长按后微动不改写词选区」）。
      - 可运行检查：`22625806` 后单跑该用例 `connected-failures: 0 failed`；
        `SelectionDragQuantifiedTest` 与 `SelectionEspressoTest` 两条回归线亦全绿。
      - 设备证据（本机模拟器，复现 CI 第 12 步的两条命令）：
        ① `./gradlew :app:connectedDebugAndroidTest` = **0 failed**（16m28s）；
        ② `./gradlew :benchmark:connectedBenchmarkReleaseAndroidTest` = **BUILD SUCCESSFUL**（4m45s）。
        两者合计量级与 CI 该步的 11m14s 同阶。
      - 未覆盖：本机模拟器与 CI 的 `android-emulator-runner` 参数不同（CI 为 api 35 /
        google_apis / x86_64 / 1536M / 2 cores / swiftshader_indirect / disable-animations），
        且 CI 上的失败仍是从「本地同款套件有且仅有该例红」推定而来——CI 日志不可得。
        下轮 CI 若再红，第 12 步的步骤粒度不足以定位到具体用例，需要在脚本里细分输出
        （`scripts/` 为禁改区，此处只记观察）。
