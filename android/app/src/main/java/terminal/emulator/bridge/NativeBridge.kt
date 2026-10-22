package terminal.emulator.bridge

import terminal.emulator.runtime.LogUtil
import java.util.concurrent.CountDownLatch

/**
 * 原生库的类初始化是否已经开始。
 *
 * 它必须是**文件级**属性：通过 `NativeBridge.xxx` 访问同一个闩会在主线程上触发
 * 类初始化，恰好把装载拉回主线程。`TerminalApp` 等这个闩，保证
 * `native-preload` 线程先进入 [NativeBridge] 的 `init`，主线程随后才可能触碰它。
 */
internal val nativeClassInitStarted = CountDownLatch(1)

/**
 * 到原生 Rust 终端引擎（`native.so`）的 JNI 桥接，全部为直接 `external fun` 导出，无 JNA、无线路编码。
 *
 * 会话流程：[initSession] 创建 → [feedPty]/[encodeKeyText] 送输入 → 每帧 [pollEvent] 排空事件 → [destroySession] 销毁。
 * 渲染流程：[attachWindow] 传入 Surface 指针，[detachWindow] 在 Surface 销毁时解绑，[resize] 更新尺寸。
 */
object NativeBridge {
    private const val TAG = "NativeBridge"
    private var nativeLoaded = false

    /**
     * 触发类初始化（也就是装载原生库）的那条线程的名字。
     *
     * 装载要读整个 `.so`，落在首帧前的主线程上会拖慢冷启动（debug 构建实测
     * 471ms，StrictMode 记为 `DiskReadViolation`），故预装载线程来触碰。
     * 记录它是为了让崩溃日志能回答「库是在哪条线程上装的」，
     * 并由仪器测试钉住「不得落在主线程」。
     */
    @Volatile
    @JvmStatic
    var loaderThreadName: String? = null
        private set

    init {
        loaderThreadName = Thread.currentThread().name
        // 先宣告「装载已在本线程开始」，再读 .so：等待方只等这一次调度，
        // 不替主线程等完整个装载。
        nativeClassInitStarted.countDown()
        try {
            System.loadLibrary("native")
            nativeLoaded = true
            LogUtil.i(TAG, "Native library loaded: native on $loaderThreadName")
        } catch (exception: UnsatisfiedLinkError) {
            LogUtil.e(TAG, "Failed to load native library: ${exception.message}")
        }
    }

    fun isNativeLoaded(): Boolean = nativeLoaded

    /**
     * 冷启动预热：调用方 MUST 在后台线程触碰本对象，使 [System.loadLibrary] 的数百毫秒
     * 磁盘 I/O 与首帧前的启动工作并行（debug 构建实测 471ms，StrictMode 记为
     * `DiskReadViolation`）。装载由本类的类初始化完成，本函数只是那样的触碰点；
     * 主线程之后首次调用任一 native 方法时只等剩余装载时间。
     *
     * 在主线程调用等于把装载挪到更早，仍串行阻塞首帧，故不提供主线程入口。
     */
    fun warmUp() {
        // 触碰即触发类初始化（`init` 块里的 loadLibrary）；无需其他动作。
    }

    // ── 会话生命周期 ──

    /** 新建终端会话，返回会话 ID（0 表示失败）。 */
    @JvmStatic
    // JNI 签名对齐原生 init_session，参数不可合并
    external fun initSession(
        rows: Int,
        cols: Int,
        shell: String,
        home: String,
        workingDirectory: String,
        prefix: String,
        mkshrcPath: String,
    ): Long

    /** 按 ID 销毁会话，成功返回 true。 */
    @JvmStatic external fun destroySession(sessionId: Long): Boolean

    /** 切换当前会话，会话存在返回 true。 */
    @JvmStatic external fun switchSession(sessionId: Long): Boolean

    /**
     * 按传入顺序返回各会话的终端标题；未知会话对应元素为 null。
     *
     * 会话列表刷新原本逐会话各发一次 [getTitle]：N 个会话就是 N 次同步往返。
     */
    @JvmStatic external fun getSessionTitles(sessionIds: LongArray): Array<String?>?

    /** 活跃会话 ID；数组为空即无会话。 */
    @JvmStatic external fun listSessions(): LongArray

    // ── 终端输入输出 ──

    /** 调整指定会话尺寸。 */
    @JvmStatic external fun resize(sessionId: Long, rows: Int, cols: Int)

    /**
     * 更新 PTY winsize 的像素字段（ws_xpixel/ws_ypixel）而保持 rows/cols 不变。
     * 感知像素的程序（icat、全屏 TUI）经 TIOCGWINSZ 读取该尺寸。
     */
    @JvmStatic external fun setPixelSize(sessionId: Long, widthPx: Int, heightPx: Int)

    /** 向 PTY 写入原始字节（二进制安全，不做 UTF-8 转码）。 */
    @JvmStatic external fun feedPty(sessionId: Long, data: ByteArray)

    /** 字节直接送入 VT 解析器而非 PTY，仅测试用：注入需由终端解析、而非被 shell 回显的转义序列（OSC 8 链接、DECSET）。 */
    @JvmStatic external fun feedTerminal(sessionId: Long, data: ByteArray)

    /**
     * 编码器未覆盖的键码所对应码点的终端字节，`metaPrefix` 为真时带 ESC 前缀（Alt/Super）。
     *
     * 只在 [terminal.emulator.input.KeyModifiers] 之外的回退分支调用（上游
     * `key::Key` 没有对应键，如 `KEYCODE_AT` 的 `@` 实为 Shift+2）。纯编码，不碰会话
     * 也不写 PTY——写入由 [Bridge] 交给既有的按键写入队列，与其余按键同一条有序出口。
     */
    @JvmStatic external fun encodeKeyText(key: String, metaPrefix: Boolean): ByteArray

    /**
     * 用 Ghostty 鼠标编码器把鼠标事件编码为终端转义序列（按应用的 DECSET 选择 SGR/X10/UTF-8）。
     * 坐标为 Surface 像素，cellWidth/cellHeight 为实时单元格尺寸；关闭鼠标上报或编码失败时返回空数组。
     * `modifiers` 取上游 `key.Mods` 原始位（见 [terminal.emulator.input.KeyModifiers.ghosttyMods]），
     * 原生以 `Mods::from_bits_retain` 直接消费：Shift/Ctrl 点击必须与普通左键可区分。
     */
    @JvmStatic
    external fun encodeMouseEvent(
        sessionId: Long,
        xPx: Float,
        yPx: Float,
        action: Int,
        button: Int,
        modifiers: Int,
        cellWidth: Float,
        cellHeight: Float,
    ): ByteArray

    /**
     * 把 `repeat` 档滚轮一次编码成 press+release 串（每档一对）。
     *
     * 触摸拖动一个事件可能滚过数行，逐档调用 [encodeMouseEvent] 会把 UI 线程按在
     * 每档一次 VT 应答上；合并后每次事件只有一次往返，PTY 也只写一次。
     * `button` 3 为向上（更旧）、4 为向下（更新）；上报未启用时返回空数组。
     */
    @JvmStatic
    external fun encodeMouseWheel(
        sessionId: Long,
        xPx: Float,
        yPx: Float,
        button: Int,
        modifiers: Int,
        cellWidth: Float,
        cellHeight: Float,
        repeat: Int,
    ): ByteArray

    /**
     * 远端是否启用鼠标上报（DECSET 1000/1002/1003）。
     *
     * 读 VT 线程每帧维护的镜像，没有查询往返：触摸拖动的每个事件都要判一次，
     * 走查询通道会把 UI 线程按满一个查询超时。
     */
    @JvmStatic
    external fun mouseTrackingActive(sessionId: Long): Boolean

    /**
     * 用 Ghostty 键编码器把按键事件编码为终端转义序列。
     *
     * DECCKM、modifyOtherKeys、kitty 键盘协议、backarrow key mode 等模式全部由编码器
     * 从终端状态读取，调用方不再查询任何模式。`keyCode` 是 Android 键码，
     * `unicodeChar`/`unshiftedChar` 是 KeyEvent 的码点（0 = 无字符）；
     * `modifiers` 取上游 `key.Mods` 原始位（见 [terminal.emulator.input.KeyModifiers]）。
     *
     * 未收录的键码、不产生序列的键（单独的修饰键）与编码失败都返回空数组；
     * 调用方据此回退到按键文本，不静默丢弃按键。
     */
    @JvmStatic
    external fun encodeKeyEvent(
        sessionId: Long,
        keyCode: Int,
        unicodeChar: Int,
        unshiftedChar: Int,
        modifiers: Int,
        action: Int,
    ): ByteArray

    /**
     * 远端是否处于备用屏幕缓冲（vim/less/htop）。由 Rust VT 线程维护的无锁镜像，
     * 可在每次触摸滚动时安全调用：备用屏下滚动以滚轮转义转发给远端而非本地回滚。
     */
    @JvmStatic external fun getAltScreenState(sessionId: Long): Boolean

    /** 将窗口焦点变化转发给会话，使子进程收到 DECSET 1004 焦点上报（`\x1b[I` / `\x1b[O`）。 */
    @JvmStatic external fun focusEvent(sessionId: Long, focused: Boolean): Boolean

    /**
     * 回复 OSC 52 剪贴板读取请求。每个请求必须且只能回复一次，同一请求 ID 的重复回复是原生空操作。
     * `text` 为 null 表示读取失败：原生据此不写回应答（空串会被远端当成用户清空了剪贴板）。
     */
    @JvmStatic external fun clipboardResult(sessionId: Long, requestId: Long, text: String?)

    // ── 事件 ──

    /**
     * 轮询事件队列，返回 JSON 编码的事件或 null；每帧在协程中调用一次（约 16ms）。
     *
     * 事件 JSON 格式（serde internal tag，snake_case）：
     * {"event":"clipboard","session_id":1,"text":"copied text"}
     * {"event":"exit","session_id":1,"code":0}
     */
    @JvmStatic external fun pollEvent(): String?

    // ── 渲染唤醒 ──

    /**
     * 渲染循环的驻留点：等新输出或任意唤醒信号，返回新的唤醒代数。
     *
     * 传入上一次拿到的代数；期间若已发生通知则立即返回，故「查完信号再进场」
     * 与通知竞争时也不会丢唤醒。[timeoutMillis] 为 0 表示只取当前代数、不等待。
     */
    @JvmStatic external fun waitForOutput(sessionId: Long, lastGeneration: Long, timeoutMillis: Long): Long

    /**
     * 唤醒指定会话的渲染循环（vsync、输入、滚动、选区等所有宿主侧信号）。
     *
     * 只是把原生唤醒代数 +1 并 `notify_all`，不触碰会话锁。
     */
    @JvmStatic external fun wakeRender(sessionId: Long)

    // ── Surface ──

    /** 绑定 Android Surface 供 GPU 渲染，原生侧据此 ANativeWindow 指针创建 wgpu surface。 */
    @JvmStatic external fun attachWindow(sessionId: Long, surface: Any, width: Int, height: Int)

    /** 解绑当前 Surface。 */
    @JvmStatic external fun detachWindow(sessionId: Long)

    /** 经 CellData 快路径渲染一帧：1 已输出，0 空闲，-1 出错。 */
    @JvmStatic external fun render(sessionId: Long, width: Int, height: Int): Int

    /**
     * 渲染与 new_output 读取合并为单次 JNI 穿越，返回打包的 `Long`：
     * bit 0..31 为渲染计数，bit 32 为 new_output 标志，bit 33..42 保留（恒 0，
     * 历史光标行位形），bit 43..52 为视口最后一个有内容的行（0x3FF 表示视口全空），
     * bit 53 为 surface 已判死，bit 54 为备用屏激活。
     * 解码见 `Bridge.renderWithNewOutput`。
     */
    @JvmStatic external fun renderWithNewOutput(sessionId: Long, width: Int, height: Int): Long

    // ── 用户输入回调 ──

    // ── TerminalQueryPort（原生查询导出） ──

    /** 会话的终端标题（OSC 0/2），会话未知时为 null。 */
    @JvmStatic external fun getTitle(sessionId: Long): String?

    /** 会话的回滚行数。 */
    @JvmStatic external fun scrollbackLength(sessionId: Long): Int

    /**
     * 最近一帧的回滚长度：与屏幕上这一帧同源，只读一次原子量。
     * 首帧尚未取走时为 -1；未知会话抛异常。
     */
    @JvmStatic external fun getRenderedScrollbackLength(sessionId: Long): Int

    /** 会话已应用的滚动偏移（0 = 视口贴底）；未知会话返回 0。 */
    @JvmStatic external fun getSessionScrollOffset(sessionId: Long): Int

    /** 某一行的文本（已去除尾部空白），空行返回 null。row 为绝对行号。 */
    @JvmStatic external fun scrollbackLine(sessionId: Long, row: Int): String?

    /** 光标视口位置，打包为 `(y << 32) | x`，隐藏时为 -1。**当前真值**，代价是一次同步 VT 查询。 */
    @JvmStatic external fun getCursorViewportPacked(sessionId: Long): Long

    /**
     * **最近一帧**渲染取走的光标视口位置，打包同 [getCursorViewportPacked]；首帧取走之前为 -1。
     * 一次原子读，不等待 VT——逐键的热路径（输入法锚点上报）用它。
     */
    @JvmStatic external fun getRenderedCursorPacked(sessionId: Long): Long

    /** 可视区与回滚文本以换行连接。 */
    @JvmStatic external fun getTerminalText(sessionId: Long): String?

    /** 仅可见视口的文本，每行以换行连接；代价随网格面积走，不随回滚深度走。 */
    @JvmStatic external fun getVisibleText(sessionId: Long): String?

    /**
     * 用 Ghostty 原生格式化器提取选中文本：软换行处不插入 '\n' 并去除尾部空白，
     * 与 termux 的 TerminalBuffer.getSelectedText 同为换行感知语义。坐标为绝对网格行列（0 = 回滚顶部），出错返回 ""。
     */
    @JvmStatic
    external fun selectionText(sessionId: Long, startRow: Int, startCol: Int, endRow: Int, endCol: Int): String?

    /**
     * 上游 select_word：派生并安装落点词选区，回传有序界限
     * `[startRow, startCol, endRow, endCol]`（绝对网格坐标，0 = 回滚顶部）；落点无可选词或查询失败返回
     * null。
     */
    @JvmStatic external fun selectWordAt(sessionId: Long, row: Int, col: Int): IntArray?

    /** 上游 select_all：全部内容派生并安装（界限不含尾部空行/空列），回传与失败语义同 [selectWordAt]。 */
    @JvmStatic external fun selectAll(sessionId: Long): IntArray?

    /** 网格单元格处的 OSC 8 超链接 URI（row 0 = 回滚顶部），无则返回 null。 */
    @JvmStatic external fun hyperlinkAt(sessionId: Long, row: Int, col: Int): String?

    /**
     * 该行中作为宽字符后半格（`SpacerTail`）的列号，升序；无宽字符或查询失败时为空数组。
     */
    @JvmStatic external fun wideCharTailCols(sessionId: Long, row: Int): IntArray

    /**
     * 设置搜索 needle 并同步跑完搜索，返回命中总数。
     * 上游 `search::Search` 承担匹配、软换行跨行与增量同步；ASCII 字母不区分大小写，
     * 无区分大小写开关。超时、断连或会话不存在返回 0。
     */
    @JvmStatic
    external fun searchSetNeedle(sessionId: Long, needle: String): Int

    /**
     * 选中下一个（`forward = true`，朝更旧内容）或上一个命中，返回
     * `[序号, 总数, 行, 起始列, 结束列]`；无命中、超时、断连或会话不存在返回 null。
     * 行号是绝对网格行（0 = 回滚顶部），选中时视口已滚到该命中。
     */
    @JvmStatic
    external fun searchSelect(sessionId: Long, forward: Boolean): IntArray?

    /** 字体库族名列表（fonts.xml 声明的文件集 + 用户投放目录）。 */
    @JvmStatic external fun listFontFamilies(): Array<String>?

    /** 默认字体家族名。 */
    @JvmStatic external fun getDefaultFontName(): String?

    /** 搜索查询串长度上限：原生为唯一真源，UI 据此截断输入。 */
    @JvmStatic external fun searchQueryMaxChars(): Int

    /**
     * 原生接受的光栅缩放（sp→px 系数）区间 `[min, max]`：原生为唯一真源。
     *
     * Kotlin MUST NOT 自带一份副本——它与原生 `setRasterScale` 的守卫一旦漂移，
     * 字号上界（`SettingsRepository.fontSizeMaxSp` 以该系数为分母）就会用被原生
     * 拒收的系数算出，重新出现「设置条范围与实际可设置范围不一致」。
     */
    @JvmStatic external fun getRasterScaleRange(): FloatArray?

    /** 字体信息打包字节（布局见 `FontInfo.fromPacked`），渲染器未初始化时为 null。 */
    @JvmStatic external fun getFontInfo(): ByteArray?

    /**
     * 推进全部会话的终端状态（输出灌入 VT、收割 VT 副作用、事件入队），但不消费事件。
     *
     * 渲染暂停（切后台、Surface 销毁）时渲染线程停止，[pollEvent] 无人调用；若此时
     * 也无人推进会话，有界输出通道会填满 → 读线程阻塞 → PTY 内核缓冲填满 → 后台
     * 作业停摆。本调用维持推进而不取走事件，故后台期间的剪贴板、退出等事件仍留在
     * 队列中，渲染恢复后由 [pollEvent] 正常派发。
     */
    @JvmStatic external fun pumpSessions()

    /** 清除渲染器的搜索高亮。 */
    @JvmStatic external fun clearSearchHighlights(sessionId: Long)

    /**
     * 把当前搜索的视口命中写入渲染态（下一帧绘制）。
     *
     * 命中区间、选中态与高亮色（当前命中用主题前景色、其余用主题选区色）都由原生
     * 按当前主题给出；本调用只提供视口滚动偏移（0 表示贴底）。
     */
    @JvmStatic
    external fun applySearchHighlights(sessionId: Long, scrollOffset: Int)

    /**
     * 设置当前文本选区（绝对网格行列，0 = 回滚顶部）；高亮颜色由终端侧按主题调色板烘焙，本通道不传颜色参数。
     */
    @JvmStatic
    external fun setSelection(
        sessionId: Long,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        hasSelection: Boolean,
    ): Boolean

    /** RIS 全重置当前会话：恢复终端初始状态并清空回滚（侧边面板“重置终端”按钮）。 */
    @JvmStatic external fun resetTerminal(sessionId: Long)

    /** 按标识应用主题：调色板真源在原生，Kotlin 只传设置里存的标识。 */
    @JvmStatic
    external fun setTheme(sessionId: Long, themeName: String)

    /**
     * 用上游 `paste::encode` 把剪贴板字节编码为 PTY 写入字节（bracketed 包裹与否
     * 取自终端模式 2004 的活体状态）。会话未知或编码失败返回空数组（本次粘贴
     * 被丢弃）；JVM 拷贝失败抛异常并返回 null。
     */
    @JvmStatic
    external fun encodePaste(sessionId: Long, data: ByteArray): ByteArray?

    /**
     * 主题目录与全部调色板的一次性打包：`u16` 主题数（小端）后跟逐条
     * 「标志 + 名称长度 + 名称 + 54 字节颜色」，顺序即设置界面展示顺序
     * （深底在前、日间在后）。标志 bit0 默认深色、bit1 默认浅色。
     *
     * 单源在 Rust 侧（构建期嵌入的 alacritty 主题）；分配失败返回 null。
     * 取代逐个标识取调色板：一次同步调用即取回全部主题。
     */
    @JvmStatic external fun getThemeCatalog(): ByteArray?

    /**
     * 暂停/恢复渲染器。标志挂在唯一的全局渲染器上，与会话无关。
     */
    @JvmStatic
    external fun setRenderPaused(paused: Boolean)

    /**
     * 测试钩子：开启/关闭持续的 surface 级取纹理失败（等价于 BufferQueue 被遗弃）。
     * 仅供仪器化用例验证自愈路径，无生产调用方；关闭后立刻恢复真实取纹理。
     */
    @JvmStatic
    external fun setSurfaceLossInjected(sessionId: Long, injected: Boolean): Boolean

    @JvmStatic
    external fun setFontFamily(sessionId: Long, family: String): Boolean

    @JvmStatic
    external fun setFontSizeInPlace(sessionId: Long, sizeTenths: Int)

    /** 设置字形光栅化缩放（设备像素密度），保证文字清晰。 */
    @JvmStatic
    external fun setRasterScale(sessionId: Long, scale: Float)

    @JvmStatic
    external fun loadFontFile(sessionId: Long, path: String): String?

    @JvmStatic
    external fun setSystemLocale(sessionId: Long, locale: String)

    @JvmStatic
    external fun setExtraFontPaths(sessionId: Long, paths: Array<String>)

    /**
     * 单元格宽高，打包为一个 `Long`：低 32 位宽度、高 32 位高度的 `f32` 位。
     * 宽高一并取回——两者只可能被同一处消费，分两次查询只是多一次往返。
     */
    @JvmStatic
    external fun getCellSizePacked(): Long

    @JvmStatic
    external fun getGridRowsColsPacked(sessionId: Long): Long

    @JvmStatic
    external fun setScrollOffset(sessionId: Long, offset: Int): Boolean

    @JvmStatic
    external fun setScrollYPx(sessionId: Long, offsetPx: Float)

    /** 后台预热渲染器与字体库（spawn 后调用，不阻塞）；正常路径上 spawn 前的网格计算已完成初始化，此处是幂等重试。 */
    @JvmStatic external fun prefetchRenderState()
}
