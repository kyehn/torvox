package terminal.emulator.bridge

import terminal.emulator.runtime.LogUtil

/**
 * 到原生 Rust 终端引擎（`native.so`）的 JNI 桥接，全部为直接 `external fun` 导出，无 JNA、无线路编码。
 *
 * 会话流程：[initSession] 创建 → [feedPty]/[writeKey] 送输入 → 每帧 [pollEvent] 排空事件 → [destroySession] 销毁。
 * 渲染流程：[attachWindow] 传入 Surface 指针，[detachWindow] 在 Surface 销毁时解绑，[resize] 更新尺寸。
 */
object NativeBridge {
    private const val TAG = "NativeBridge"
    private var nativeLoaded = false

    init {
        try {
            System.loadLibrary("native")
            nativeLoaded = true
            LogUtil.i(TAG, "Native library loaded: native")
        } catch (exception: UnsatisfiedLinkError) {
            LogUtil.e(TAG, "Failed to load native library: ${exception.message}")
        }
    }

    fun isNativeLoaded(): Boolean = nativeLoaded

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

    /** 当前活跃会话数。 */
    @JvmStatic external fun getSessionCount(): Int

    /**
     * 会话的回滚行数（会话未知时为 0），作为内存计量的输入：回滚无界增长会表现为行数单调上升。
     */
    @JvmStatic external fun getScrollbackRows(sessionId: Long): Int

    /** 活跃会话 ID 的 JSON 数组，如 "[1, 2, 3]"。 */
    @JvmStatic external fun listSessions(): String?

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
     * 编码并提交按键事件。
     *
     * @param key 按键名，如 "a"、"Enter"、"Escape"、"Space"
     * @param mods 修饰键位掩码（1=shift，2=alt，4=ctrl，8=meta，16=super）
     * @param text 输入法合成的文本，非输入法按键传 null
     */
    @JvmStatic external fun writeKey(sessionId: Long, key: String, mods: Int, text: String?)

    /**
     * 用 Ghostty 鼠标编码器把鼠标事件编码为终端转义序列（按应用的 DECSET 选择 SGR/X10/UTF-8）。
     * 坐标为 Surface 像素，cellWidth/cellHeight 为实时单元格尺寸；关闭鼠标上报或编码失败时返回空数组。
     */
    @JvmStatic
    external fun encodeMouseEvent(
        sessionId: Long,
        xPx: Float,
        yPx: Float,
        action: Int,
        button: Int,
        cellWidth: Float,
        cellHeight: Float,
    ): ByteArray

    /**
     * 远端是否处于备用屏幕缓冲（vim/less/htop）。由 Rust VT 线程维护的无锁镜像，
     * 可在每次触摸滚动时安全调用：备用屏下滚动以滚轮转义转发给远端而非本地回滚。
     */
    @JvmStatic external fun getAltScreenState(sessionId: Long): Boolean

    /** 查询终端模式（ghostty `mode_get`）；`kind` 为 0 是 DEC 私有模式，非 0 是 ANSI 模式。用于查 DECCKM（应用光标键，DEC 私有模式 1）以在 SS3（`ESC OA`）与 CSI（`ESC [ A`）之间切换方向键。 */
    @JvmStatic external fun getMode(sessionId: Long, modeNum: Int, kind: Int): Boolean

    /** 将窗口焦点变化转发给会话，使子进程收到 DECSET 1004 焦点上报（`\x1b[I` / `\x1b[O`）。 */
    @JvmStatic external fun focusEvent(sessionId: Long, focused: Boolean): Boolean

    /** 回复 OSC 52 剪贴板读取请求。每个请求必须且只能回复一次，同一请求 ID 的重复回复是原生空操作。 */
    @JvmStatic external fun clipboardResult(sessionId: Long, requestId: Long, text: String)

    // ── 事件 ──

    /**
     * 轮询事件队列，返回 JSON 编码的事件或 null；每帧在协程中调用一次（约 16ms）。
     *
     * 事件 JSON 格式（serde internal tag，snake_case）：
     * {"event":"clipboard","session_id":1,"text":"copied text"}
     * {"event":"exit","session_id":1,"code":0}
     */
    @JvmStatic external fun pollEvent(): String?

    // ── Surface ──

    /** 绑定 Android Surface 供 GPU 渲染，原生侧据此 ANativeWindow 指针创建 wgpu surface。 */
    @JvmStatic external fun attachWindow(sessionId: Long, surface: Any, width: Int, height: Int)

    /** 解绑当前 Surface。 */
    @JvmStatic external fun detachWindow(sessionId: Long)

    /** 经 CellData 快路径渲染一帧：1 已输出，0 空闲，-1 出错。 */
    @JvmStatic external fun render(sessionId: Long, width: Int, height: Int): Int

    /**
     * 渲染与 new_output 读取合并为单次 JNI 穿越，返回打包的 `Long`：
     * bit 0..31 为渲染计数，bit 32 为 new_output 标志，bit 33..48 为视口光标行（0xFFFF 表示隐藏或在视口外）。
     */
    @JvmStatic external fun renderWithNewOutput(sessionId: Long, width: Int, height: Int): Long

    // ── 用户输入回调 ──

    // ── 日志 ──

    /** 初始化原生日志，启动时调用一次。 */
    @JvmStatic external fun initLogger()

    // ── TerminalQueryPort（原生查询导出） ──

    /** 会话的终端标题（OSC 0/2），会话未知时为 null。 */
    @JvmStatic external fun getTitle(sessionId: Long): String?

    /** 会话的回滚行数。 */
    @JvmStatic external fun scrollbackLength(sessionId: Long): Int

    /** 某一行的文本（已去除尾部空白），空行返回 null。row 为绝对行号。 */
    @JvmStatic external fun scrollbackLine(sessionId: Long, row: Int): String?

    /** 光标视口位置，打包为 `(y << 32) | x`，隐藏时为 -1。 */
    @JvmStatic external fun getCursorViewportPacked(sessionId: Long): Long

    /** 可视区与回滚文本以换行连接。 */
    @JvmStatic external fun getTerminalText(sessionId: Long): String?

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

    /** 上游 select_line：整行派生并安装（语义提示边界关），回传与失败语义同 [selectWordAt]。 */
    @JvmStatic external fun selectLineAt(sessionId: Long, row: Int, col: Int): IntArray?

    /** 上游 select_all：全部内容派生并安装（界限不含尾部空行/空列），回传与失败语义同 [selectWordAt]。 */
    @JvmStatic external fun selectAll(sessionId: Long): IntArray?

    /** 网格单元格处的 OSC 8 超链接 URI（row 0 = 回滚顶部），无则返回 null。 */
    @JvmStatic external fun hyperlinkAt(sessionId: Long, row: Int, col: Int): String?

    /**
     * 搜索整个回滚缓冲，返回 `{"row":int,"start_col":int,"end_col":int}` 的 JSON 数组
     * （列号为字节偏移），超时返回 `[]`。调用方需在 UI 线程做防抖。
     */
    @JvmStatic
    external fun searchAllInScrollback(sessionId: Long, query: String, caseSensitive: Boolean): String?

    /** (row, col) 处单元格是否没有可打印码点。 */
    @JvmStatic external fun isCellEmpty(sessionId: Long, row: Int, col: Int): Boolean

    /** 字体库族名列表（fonts.xml 声明的文件集 + 用户投放目录）。 */
    @JvmStatic external fun listFontFamilies(): Array<String>?

    /** 默认字体家族名。 */
    @JvmStatic external fun getDefaultFontName(): String?

    /** 结构化字体信息 JSON（见 [FontInfoDto]），渲染器未初始化时为 null。 */
    @JvmStatic external fun getFontInfo(): String?

    /** 清除渲染器的搜索高亮。 */
    @JvmStatic external fun clearSearchHighlights(sessionId: Long)

    /** 设置渲染器搜索高亮范围（按字节打包，见 TerminalSurface）。 */
    @JvmStatic external fun setSearchHighlights(sessionId: Long, data: ByteArray)

    /**
     * 设置当前文本选区（可视网格行列）；高亮颜色由终端侧按主题调色板烘焙，本通道不传颜色参数。
     */
    @JvmStatic
    external fun setSelection(
        sessionId: Long,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        hasSelection: Boolean,
    )

    /** RIS 全重置当前会话：恢复终端初始状态并清空回滚（侧边面板“重置终端”按钮）。 */
    @JvmStatic external fun resetTerminal(sessionId: Long)

    external fun setTheme(sessionId: Long, data: ByteArray)

    @JvmStatic
    external fun setRenderPaused(sessionId: Long, paused: Boolean)

    /**
     * 测试钩子：开启/关闭持续的 surface 级取纹理失败（等价于 BufferQueue 被遗弃）。
     * 仅供仪器化用例验证自愈路径，无生产调用方；关闭后立刻恢复真实取纹理。
     */
    @JvmStatic
    external fun setSurfaceLossInjected(sessionId: Long, injected: Boolean): Boolean

    /**
     * 应用层光标颜色覆盖，线性 RGB（每通道 0..1），总是覆盖，无清除路径。
     */
    @JvmStatic
    external fun setCursorColor(sessionId: Long, red: Float, green: Float, blue: Float)

    external fun setFontFamily(sessionId: Long, family: String): Boolean

    @JvmStatic
    external fun setFontSizeInPlace(sessionId: Long, sizeTenths: Int)

    /** 设置字形光栅化缩放（设备像素密度），保证文字清晰。 */
    @JvmStatic
    external fun setRasterScale(sessionId: Long, scale: Float)

    external fun loadFontFile(sessionId: Long, path: String): String?

    @JvmStatic
    external fun setSystemLocale(sessionId: Long, locale: String)

    external fun setExtraFontPaths(sessionId: Long, paths: Array<String>)

    @JvmStatic
    external fun getCellWidth(sessionId: Long): Float

    external fun getCellHeight(sessionId: Long): Float

    @JvmStatic
    external fun getGridRowsColsPacked(sessionId: Long): Long

    external fun setScrollOffset(sessionId: Long, offset: Int)

    @JvmStatic
    external fun setScrollYPx(sessionId: Long, offsetPx: Float)

    /** 后台预热渲染器与字体库（spawn 后、attach 前调用，不阻塞）。 */
    @JvmStatic external fun prefetchRenderState()
}
