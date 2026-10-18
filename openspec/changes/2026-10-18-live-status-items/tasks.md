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

- [ ] 重命名文件卡死
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
- [ ] prompt 占满整行时输入内容后全部退格不留下空行
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

- [ ] 简体中文退格延迟
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
- [ ] 文档提供者：创建、复制/移动、保存超时三个子项
- [ ] 启动屏图案与应用图标
