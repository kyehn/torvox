# mtty-term 研究留档

## 定位

mtty 的终端核心，作为独立 Gradle 模块 `:term`（包名 `com.spizganed.mtty.term`）存在，不引用任何应用代码。唯一 VT 核心是 libghostty-vt，经 mtty 主仓的 JNI shim 访问；视图与按键编码由本模块自己写，只依据 xterm/vt100/Kitty 规范。

## 数据流

`TermCore` 定义视图与按键所需的最小接口：屏幕行、历史、光标、各种模式、字节入口。`GhosttyTerminal` 是唯一实现，`GhosttyNative` 暴露 `nFeed`、`nResize`、`nReadRows`、`nInfo`、`nPalette`、`nScrolled`、`nTitle`、`nPwd`、`nSetCell`、`nLink`。行 tag 0 表示当前屏，3 表示历史。

字节进来只调 `nFeed` 并置 `dirty`；行在视图读取时才拷进 `Row`，不是每次 feed 后读，所以多次 feed 只对应一次读取。`sync()` 一次取 `nInfo`、`nPalette` 与整屏行；未被修改的 cell 直接跳过，`Row.version` 只在字段变化时递增，供视图判断是否需要重录 display list。历史行用 `LinkedHashMap` 缓存 256 条，键是 `scrolledRows + i` 这个单调逻辑行号；resize 会清空缓存，因为 reflow 改变了历史行。

一个 cell 是 `Int` 码点加一个 `Long` 样式，宽字符占两个 cell（第二格记为 `Row.WIDE_TAIL`），组合字符进入 `extra`。raw 行缓冲每 cell 24 字节，整屏缓冲按 `rows * (4 + columns * CELL) + 4 + EXTRA_ROOM` 分配。模式位来自 `info[9]`：方向键应用 1、小键盘应用 2、bracketed paste 4、反显 8、焦点事件 16、同步输出 32、鼠标 SGR 64、换行模式 1024。`keyboardFlags` 只暴露 `Keys.KITTY_SUPPORTED` 子集，避免答应程序发送本端无法编码的 Kitty 事件。

## 视图与输入

`TermView` 在 `onDraw` 中按 `row.version` 复用硬件 Canvas 的 RenderNode；同步输出（DECSET 2026）开启时 `freeze()` 会等待结束标记或超时再呈现。`onScreenUpdated` 用 `scrolledRows` 差值判断新增历史行。选区、长按、捏合、链接点击都在视图里，应用侧只实现 `TermView.Client`。

`Keys` 把按键、文本、鼠标与焦点事件编码成字节。Kitty 键盘协议按 flag 栈处理（事件类型、全部按键转转义等），legacy 模式下释放事件不发送。`pasteText` 去掉 ESC 与 C1 控制字符、把换行换成 CR，程序要求 bracketed paste 时外包 `ESC[200~`/`ESC[201~`。`Find` 逐行大小写无关搜索，命中不跨行；`OneLine` 把程序软换行的行拼回一行，以 `!` 或以 `$` 加空格开头的行保留断点。盒线、块元素与 Powerline 分隔符由 `BoxTable.kt` 的形状表绘制，列宽测量交给 libghostty。

## 已核实局限

- JVM 测试依赖 mtty 主仓构建的 host shim，缺库时跳过；`README` 写明 95 个测试。这种“缺依赖就跳过”的测试形态不适合本仓。
- libghostty 只保存 OSC 8 的 URI，因此同 URI、不同 `id=` 的两个链接会共享同一个 link id。
- `GhosttyTerminal.close()` 之后对象不可再用，但 `finalize()` 兜底释放只是最后防线；`available` 首次读取才加载库，构造器里额外检查了一次，防止首个实例因加载失败而抛原始 `UnsatisfiedLinkError`。

## 与本仓对照

- 本仓同样用 libghostty，但脏帧管理集中在 Rust 侧并向 Kotlin 只送轻量事件；mtty 的“按 version 复用行 display list”思路可用于核对本仓的行级缓存是否真的跳过了未变行。
- 同步输出冻结、`keyboardFlags` 只暴露可发送子集、paste 清洗与 bracketed 包裹，都是本仓已有或计划中的行为，适合作为边界用例来源。
- 盒绘制表从 Unicode 数据生成而非手写常量，与本仓规范“从规范来源取数据、不硬编码列表”一致。
