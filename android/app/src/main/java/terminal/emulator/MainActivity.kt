package terminal.emulator

import android.Manifest
import android.annotation.SuppressLint
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.os.SystemClock
import android.view.InputDevice
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
import androidx.core.view.WindowCompat
import androidx.hilt.lifecycle.viewmodel.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import terminal.emulator.input.KeyModifiers
import terminal.emulator.input.afterVirtualCtrlKey
import terminal.emulator.installer.BootstrapInstallService
import terminal.emulator.runtime.LogUtil
import terminal.emulator.runtime.TerminalRuntime
import terminal.emulator.settings.BackKeyBehaviour
import terminal.emulator.settings.SessionShortcut
import terminal.emulator.settings.TermuxProperties
import terminal.emulator.settings.VolumeKeysBehaviour
import terminal.emulator.ui.SettingsScreen
import terminal.emulator.ui.TerminalScreen
import terminal.emulator.ui.theme.resolveMaterialColorScheme
import terminal.emulator.util.TerminalDispatchers
import javax.inject.Inject

/** 冷启动后，设置覆盖层的暂停逻辑等待 bridge 出现的时间预算（毫秒）。 */
private const val BRIDGE_READY_TIMEOUT_MS = 2_500L

/** 等待 bridge 出现期间的轮询间隔（毫秒）。 */
private const val BRIDGE_READY_POLL_INTERVAL_MS = 50L

/** 系统启动屏的最长驻留：终端首帧迟迟不来（引导未装、会话起不来）时不得永久遮挡界面。 */
private const val SPLASH_MAX_WAIT_MS = 5_000L

@AndroidEntryPoint
class MainActivity : ComponentActivity() {
    companion object {
        private const val TAG = "MainActivity"

        /**
         * 兼容 termux 的应急 extra：应用快捷方式「新建会话（应急）」与第三方
         * 启动器/任务器随 ACTION_RUN 发送 `com.termux.app.failsafe_session=true`。保留 termux
         * 原名（applicationId 即 com.termux），以便现有快捷方式与 tasker 任务继续工作。
         */
        const val EXTRA_FAILSAFE_SESSION = "com.termux.app.failsafe_session"

        /** 本应用专有 extra：在启动时打开设置页。 */
        const val EXTRA_OPEN_SETTINGS = "terminal.emulator.open_settings"

        /**
         * 桌面通知点击携带的会话 id：点击后切到该会话。
         *
         * 0 表示无此请求（extra 缺省）。会话 id 从 1 起，故 0 不会与真实 id 撞车。
         */
        const val EXTRA_NOTIFICATION_SESSION_ID = "terminal.emulator.notification_session_id"

        /**
         * 仅测试用的 extra：从本地路径安装引导 zip，供引导程序真实终端测试使用
         * ——instrumentation 进程无法写应用 filesDir（SELinux app_data 类目）。
         * 仅 debug 构建响应，release 构建不携带该后门。
         */
        const val EXTRA_INSTALL_BOOTSTRAP = "terminal.emulator.install_bootstrap"
    }

    @Inject
    @Suppress("LateinitUsage") // Dagger injection
    lateinit var runtime: TerminalRuntime

    /**
     * 通知权限请求：AndroidX 契约替代裸 `requestPermissions` + 魔数请求码。
     * 拒绝不影响终端功能（只是会话通知不再出现），故结果无需处理。
     */
    private val notificationPermissionLauncher =
        registerForActivityResult(
            ActivityResultContracts.RequestPermission(),
        ) { _ -> }

    /**
     * 上一份配置快照：夜间模式、字体缩放与显示密度任一变化都要重推渲染契约。
     *
     * 存副本而非引用：框架会复用并就地改写同一个 `Configuration` 实例，存引用会让
     * 「上一份」被改写成当前值，变化永远检测不到。
     */
    private var previousConfiguration: Configuration? = null

    internal val terminalViewModel: TerminalViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        // 系统启动屏：冷启动首帧前显示主题背景，与 windowBackground 同色，
        // 首帧就绪后自动切回 Theme.Terminal。
        // 撤下时机不交给默认的「内容视图首次绘制」——那一刻原生渲染器还没提交过缓冲，
        // SurfaceView 的开孔区透出黑底（真机 logcat：首帧在进程启动后约 0.85s 才呈现，
        // 两者之间即「Shell 启动前偶发黑屏」）。改为等到终端首帧已呈现，见
        // [shouldKeepSplashOnScreen]。
        val splashScreen = installSplashScreen()
        val splashStartMs = SystemClock.uptimeMillis()
        splashScreen.setKeepOnScreenCondition {
            // 闭包在内容视图首次预绘制时才求值，故此处读 runtime 一定晚于 Hilt 注入。
            shouldKeepSplashOnScreen(
                framePresented = runtime.hasPresentedFrame,
                waitedMs = SystemClock.uptimeMillis() - splashStartMs,
            )
        }
        super.onCreate(savedInstanceState)
        // activity 1.14.0-alpha03 弃用 ComponentActivity.enableEdgeToEdge()，
        // 改用 core 的 WindowCompat.enableEdgeToEdge(window)。
        WindowCompat
            .enableEdgeToEdge(window)
        previousConfiguration = Configuration(resources.configuration)
        // Color.TRANSPARENT 是 ARGB 整数而非资源 id —— lint 建议的 KTX
        // Int.toDrawable() 会把它当作资源 id (0) 并解析到错误的 drawable。
        @SuppressLint("UseKtx")
        window.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
        window.setFormat(PixelFormat.TRANSPARENT)
        // 前台服务不在这里启动：它只为存活会话提供常驻通知与 PARTIAL_WAKE_LOCK，
        // 而首个会话由 `TerminalRuntime.startSession` 创建时经
        // `startForegroundServiceIfNeeded` 启动。冷启动时先起服务要付一次 AMS 往返
        // + 通知渠道创建，且裸 start() 的计数为 0，会被紧接着的会话更新覆盖。
        // Android 13+ 需要 POST_NOTIFICATIONS 运行时权限，未授权时通知被**静默丢弃**
        // （不抛异常），常驻通知与桌面通知都不会出现。启动时申请一次；拒绝不影响终端功能。
        if (
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        handleLaunchIntent(intent, freshIntent = savedInstanceState == null)
        registerBackKeyMapping()
        // 最后一个会话结束后关闭应用（termux 语义：会话表空了就 finish，见 termux-app
        // TermuxTerminalSessionActivityClient#removeFinishedSession）。此前会话被移除后
        // 应用停在无会话的空界面，用户看到的是「exit 后什么都没关」。
        // 生命周期作用域内收集：Activity 销毁即取消，且 finish() 本就在主线程调用。
        lifecycleScope.launch {
            val sessions = SessionTableWatcher()
            runtime.state.collect { state ->
                if (sessions.onSessionCount(state.sessionIds.size)) {
                    LogUtil.debug(TAG, "last session closed; finishing activity")
                    finish()
                }
            }
        }
        setContent {
            TerminalNavHost(openSettingsRequests = openSettingsRequests)
        }
    }

    /**
     * `back-key` 决定返回键的语义。
     *
     * `escape`：返回键改发 ESC 且**不退出应用**——终端里跑 vim/less 时「返回」该怎么走
     * 由远端决定，退出应用反而是意外的副作用。`back`（默认）交回系统：先收键盘，
     * 无键盘时退出。
     *
     * 走 `OnBackPressedCallback` 而不是 `dispatchKeyEvent` 里截 `KEYCODE_BACK`：
     * 手势返回与预测性返回不产生按键事件，只有这条通道覆盖全部返回路径。
     */
    private fun registerBackKeyMapping() {
        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    val properties = terminalViewModel.termuxConfiguration.value.properties
                    if (
                        terminalViewModel.terminalScreenActive &&
                        properties.backKey == BackKeyBehaviour.ESCAPE
                    ) {
                        terminalViewModel.sendKey(KeyEvent.KEYCODE_ESCAPE)
                        return
                    }
                    // 交回系统前先让位：本回调仍是启用状态时转发会再次进到这里，
                    // 形成无限递归而不是退出。
                    isEnabled = false
                    onBackPressedDispatcher.onBackPressed()
                    isEnabled = true
                }
            },
        )
    }

    /**
     * 应用快捷方式/第三方 intent 处理（兼容 termux）：`ACTION_RUN` +
     * EXTRA_FAILSAFE_SESSION 新建一个应急系统 shell 会话；
     * EXTRA_OPEN_SETTINGS 打开设置面板。对应 termux-app TermuxActivity.java:401-425
     * （快捷方式 intent 在重建时会重新投递，故应急请求必须在 onNewIntent 中
     * 重新应用，而不只是 onCreate）。
     *
     * @param freshIntent 本次 Intent 是否为新投递。Activity 重建会再次走 onCreate
     *   并带上同一个启动 Intent；据此再建会话会让每次配置变更（密度、语言）都
     *   多出一个应急会话。onNewIntent 到达的 Intent 恒为新投递。
     *
     * 打开设置请求计数：每次带 `EXTRA_OPEN_SETTINGS` 的 intent 到达即递增。
     * 用计数而不用布尔值——设置关闭后计数保持，再次到达仍能触发重组；
     * 布尔值在首次置 true 后不再变化，二次 intent 无法生效。
     */
    private var openSettingsRequests by mutableIntStateOf(0)

    private fun handleLaunchIntent(intent: Intent?, freshIntent: Boolean) {
        if (intent == null) {
            LogUtil.debug("MainActivity", "handleLaunchIntent: null intent")
            return
        }
        LogUtil.debug(
            "MainActivity",
            "handleLaunchIntent: action=${intent.action} failsafe=${intent.getBooleanExtra(
                EXTRA_FAILSAFE_SESSION,
                false,
            )} fresh=$freshIntent",
        )
        if (
            Intent.ACTION_RUN == intent.action && intent.getBooleanExtra(EXTRA_FAILSAFE_SESSION, false)
        ) {
            // 交给 ViewModel 决定处置：应用未启动时首个会话由 runtime.start() 以
            // /system/bin/sh 建出；应用已启动时必须显式建一个新会话（start() 会早退）。
            terminalViewModel.requestFailsafeSession(freshIntent)
        }
        // 重建时重复投递的启动 Intent 不重放状态改写：否则用户关闭设置页后的一次
        // 配置变更（密度、语言）会把设置页重新打开。
        if (freshIntent && intent.getBooleanExtra(EXTRA_OPEN_SETTINGS, false)) {
            openSettingsRequests++
        }
        // 桌面通知点击：切到通知所属的会话。Surface 未就绪时 ViewModel 记下请求，
        // 待 Surface 交付后再执行（通知点击常把应用从后台拉起，那时还没有 Surface）。
        // 重建时重复投递的同一个 Intent 不重放切换——用户在重建前切走的会话会被拽回去。
        val notificationSessionId = intent.getLongExtra(EXTRA_NOTIFICATION_SESSION_ID, 0L)
        if (notificationSessionId != 0L) {
            LogUtil.info(TAG, "handleLaunchIntent: switching to session $notificationSessionId")
            terminalViewModel.requestSessionSwitch(notificationSessionId, freshIntent)
        }
        // 仅测试用的引导安装入口（TermuxBootstrapRealTerminalTest）。
        // 仅限 debug 构建：release APK 绝不能携带该安装后门 extra
        // （否则任何应用都能把我们指向任意 zip）。
        if (BuildConfig.DEBUG) {
            intent.getStringExtra(EXTRA_INSTALL_BOOTSTRAP)?.let { zipPath ->
                installBootstrapFromPath(zipPath)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        // MUST NOT setIntent(intent)：仪器化的 ActivityScenario 以「Activity 当前 intent
        // 等于启动 intent」筛选生命周期事件（不匹配即整段忽略，见其
        // "intent does not match" 日志）。换掉 intent 会让测试收不到 DESTROYED，
        // 收尾永远等不到销毁态。本仓也没有任何地方读 getIntent()——launch intent 的
        // 处理全部走 onCreate/onNewIntent 的入参。
        handleLaunchIntent(intent, freshIntent = true)
    }

    /**
     * 从本地路径安装引导程序 zip。委托给 [BootstrapInstallService]，
     * 该服务运行在独立的 `:install` 进程：主进程为 instrumentation 目标时携带的是测试包的
     * SELinux 分类，无法写入 filesDir（已在模拟器验证）。安装状态一律从已安装的条目
     * （usr/bin/login）读回，不读标记文件。
     */
    private fun installBootstrapFromPath(zipPath: String) {
        LogUtil.debug("MainActivity", "delegating bootstrap install to :install process")
        startService(
            Intent(this, BootstrapInstallService::class.java)
                .putExtra(BootstrapInstallService.EXTRA_ZIP_PATH, zipPath),
        )
    }

    /**
     * 应用进入前台：桌面通知改用应用内提示（用户在终端前，无需离开当前界面）。
     *
     * 判据取 Activity 生命周期而非「渲染是否暂停」：后者在设置页打开、输入法弹出
     * 时也为真，那时用户仍看着终端。
     */
    override fun onStart() {
        super.onStart()
        runtime.setAppForeground(true)
    }

    /** 应用退到后台：桌面通知改用系统通知（应用内提示在后台不可见）。 */
    override fun onStop() {
        runtime.setAppForeground(false)
        super.onStop()
    }

    override fun onDestroy() {
        LogUtil.debug(TAG, "onDestroy")
        super.onDestroy()
        // 无会话时停止前台服务：否则用户离开应用后服务（及其 PARTIAL_WAKE_LOCK）
        // 永久存活，持续耗电并常驻通知。有活跃会话时必须继续运行。运行时在会话锁内
        // 重新核对：IO 线程上新建的后台会话可能已越过（较旧的）状态快照。
        runtime.stopForegroundServiceIfIdle()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        // 夜间模式、系统「字体大小」与显示密度都改写渲染契约（主题、sp→px 系数、
        // 文字光栅尺度），任一变化都要重推一次。manifest 已声明这些限定符，
        // Activity 不重建，而 TerminalRuntime 作为 @Singleton 仍会活下来——
        // 不在此补推，系数就停在旧值：Kotlin 按新系数算字号与网格、原生按旧值
        // 光栅，字形被拉伸、度量与网格整体偏掉。
        val previous = previousConfiguration
        if (previous != null && shouldApplySettingsOnConfigurationChange(previous, newConfig)) {
            lifecycleScope.launch(TerminalDispatchers.inputOutput) {
                runtime.applySettings()
            }
        }
        previousConfiguration = Configuration(newConfig)
    }

    @SuppressLint("RestrictedApi")
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (handleTermuxPropertyKey(event)) {
            LogUtil.debug(TAG, "dispatchKeyEvent: consumed by termux.properties key mapping")
            return true
        }
        val handled = terminalViewModel.handleLayoutAwareHardwareKey(event)
        if (handled) {
            LogUtil.debug(TAG, "dispatchKeyEvent: consumed physical-key layout-aware char")
            return true
        }
        return super.dispatchKeyEvent(event)
    }

    /**
     * `termux.properties` 里的按键改写，在输入法/编码器之前生效。
     *
     * 规则都返回「已消费」，因为它们改写的是**事件的路由**而不是字符：
     * 放回平台路径就意味着同一个组合键既进终端又触发快捷键。
     *
     * - `volume-keys = virtual`（termux 默认）：音量减键当虚拟 Ctrl（termux 的
     *   `handleVirtualKeys` 把音量减映射成 `mVirtualControlKeyDown`）。音量加在 termux
     *   里是虚拟 FN，而本应用没有 FN 映射表（FN+字母→ESC/方向键那一套），**故意不拦截**：
     *   抢走一个只能什么都不做的键，等于白丢音量加。
     * - `ctrl-space-workaround`：部分输入法/键盘把 Ctrl+空格吞成输入法切换，显式补发 NUL。
     */
    private fun handleTermuxPropertyKey(event: KeyEvent): Boolean {
        if (!terminalViewModel.terminalScreenActive) return false
        val properties = terminalViewModel.termuxConfiguration.value.properties
        val actionDown = event.action == KeyEvent.ACTION_DOWN

        sessionShortcutFor(event, properties)?.let { shortcut ->
            // 只认首次按下：长按会产生一串 repeatCount > 0 的 ACTION_DOWN，
            // 逐条执行会在按住 Ctrl+N 的一秒里建出十几个会话。
            if (actionDown && event.repeatCount == 0) {
                when (shortcut) {
                    SessionShortcut.CREATE -> terminalViewModel.createSession()
                    SessionShortcut.NEXT -> terminalViewModel.switchToAdjacentSession(forward = true)
                    SessionShortcut.PREVIOUS -> terminalViewModel.switchToAdjacentSession(forward = false)
                }
            }
            return true
        }

        if (
            properties.volumeKeys == VolumeKeysBehaviour.VIRTUAL &&
            event.keyCode == KeyEvent.KEYCODE_VOLUME_DOWN &&
            !isExternalAlphabeticKeyEvent(event)
        ) {
            // 按住期间持续生效（Termux 的 `mVirtualControlKeyDown`）：判据见
            // `ModifierState.afterVirtualCtrlKey`（含「不得吞掉手动锁定」与「抬起只回收
            // 自己置的 Held」两条），此处只做接线。
            terminalViewModel.ctrlModifierState
                .afterVirtualCtrlKey(actionDown)
                ?.let { terminalViewModel.setCtrlModifier(it) }
            return true
        }

        if (
            properties.ctrlSpaceWorkaround &&
            event.keyCode == KeyEvent.KEYCODE_SPACE &&
            event.isCtrlPressed &&
            !event.isAltPressed
        ) {
            // Ctrl+空格在终端语义里就是 NUL。只在下按发送一次（长按的重复下按不再补发），
            // 但上下两路都要消费，否则抬起事件仍会漏回输入法/编码器。
            if (actionDown && event.repeatCount == 0) {
                terminalViewModel.sendKey(
                    keyCode = KeyEvent.KEYCODE_SPACE,
                    modifiers = KeyModifiers.CTRL,
                )
            }
            return true
        }

        return false
    }

    /**
     * 该事件是否命中 `shortcut.*` 配置。
     *
     * 上游在 `TerminalView` 清掉 Ctrl 修饰后取码点，再转小写匹配
     * （`TermuxTerminalViewClient.onCodePoint`），因此 Ctrl+Shift+N 与 Ctrl+n 等价。
     * 这里用同一个口径：清掉 Ctrl 后取 Unicode 字符，转小写比较。Alt 组合不参与，
     * 上游也把 Ctrl+Alt 留给硬件快捷键。
     */
    private fun sessionShortcutFor(event: KeyEvent, properties: TermuxProperties): SessionShortcut? {
        if (!event.isCtrlPressed || event.isAltPressed) return null
        val codePoint = event.getUnicodeChar(event.metaState and KeyEvent.META_CTRL_MASK.inv())
        if (codePoint == 0) return null
        val lowerCase = Character.toLowerCase(codePoint)
        return properties.sessionShortcuts.entries.firstOrNull { it.value == lowerCase }?.key
    }

    /**
     * 该按键事件是否来自外接的**完整字母键盘**。
     *
     * 上游 `TermuxTerminalViewClient.handleVirtualKeys` 的判据：外接字母键盘在场时
     * 不抢它的音量键——那些键属于键盘自身的布局（部分蓝牙键盘带音量键），抢过来
     * 当 Ctrl 用会让用户调不了音量。本应用沿用同一判据。
     */
    private fun isExternalAlphabeticKeyEvent(event: KeyEvent): Boolean =
        event.device?.keyboardType == InputDevice.KEYBOARD_TYPE_ALPHABETIC
}

@Composable
private fun TerminalNavHost(openSettingsRequests: Int = 0, viewModel: TerminalViewModel = hiltViewModel()) {
    // 设置覆盖层的开合状态跨 Activity 重建保持：用 `rememberSaveable`（随实例状态
    // 保存），不用 `remember`（随组合丢弃）。重建前打开的仍打开；已关闭的不得因重建
    // 重新打开——后者过去由启动 Intent 决定，于是「关掉设置页后改一次系统语言」会把
    // 设置页弹回来，而手动打开的设置页又会在重建后凭空消失。
    var showSettings by rememberSaveable { mutableStateOf(openSettingsRequests > 0) }
    LaunchedEffect(openSettingsRequests) {
        if (openSettingsRequests > 0) showSettings = true
    }
    // 设置页的渲染暂停走运行期的共享账本而不是直接写全局布尔：关设置页时
    // 无条件 setRenderPaused(false) 会把切后台那条暂停一并顶掉，在已被系统回收的
    // BufferQueue 上出帧即永久黑屏。账本保证只有全部来源都归还后才恢复。
    var settingsPauseToken by remember { mutableStateOf<Long?>(null) }
    LaunchedEffect(showSettings) {
        if (!showSettings) return@LaunchedEffect
        // bridge 由运行期在首个会话 spawn 之后异步创建；原生暂停标志落在全局渲染器
        // 上，渲染器尚未建立时写入即丢失，故打开设置页前先等它出现（有界等待，
        // 只在带 `openSettingsOnLaunch` 的冷启动才真正等待）。
        awaitBridgeReady { viewModel.runtime.bridge() != null }
        settingsPauseToken = viewModel.runtime.acquireRenderPause()
    }
    DisposableEffect(showSettings) {
        onDispose {
            // 打开设置时也走这里（本次持有为空），那一次 MUST NOT 出帧：刚打开就
            // 出一帧，等于在「设置页遮住终端」的第一帧上白白跑一次 GPU。
            val token = settingsPauseToken
            settingsPauseToken = null
            if (token == null) return@onDispose
            viewModel.runtime.releaseRenderPause(token)
            // 关设置页要的是「立刻出一帧」：账本归还不是无条件恢复，Surface 已就绪时
            // 这里补一次，与 TerminalSurface.surfaceCreated 同源。
            viewModel.runtime.forceRender()
        }
    }
    val settings by viewModel.settings.collectAsStateWithLifecycle()
    val appThemeMode = settings.appThemeMode
    val isDarkTheme = isSystemInDarkTheme()

    // 深色与否只由 resolveAppDarkMode 裁决一次：配色方案内部复用它，
    // 调用方不再自己算一份 forceDark 传进去（两份判断迟早会分家）。
    val colorScheme = resolveMaterialColorScheme(appThemeMode, isDarkTheme)

    Box(Modifier.semantics { testTagsAsResourceId = true }) {
        MaterialTheme(colorScheme = colorScheme) {
            TerminalScreen(
                onSettings = { showSettings = true },
                isOverlayVisible = showSettings,
            )
            if (showSettings) {
                SettingsScreen(
                    onBack = { showSettings = false },
                )
            }
        }
    }
}

/**
 * 配置变化是否要求重推渲染契约。
 *
 * 夜间模式改主题；系统「字体大小」与显示密度改写 sp→px 系数，进而改写文字光栅
 * 尺度与网格度量。三者任一变化都要重推一次渲染契约——manifest 已声明这些限定符，
 * Activity 不重建，运行期是 @Singleton 存活下来，不重推就停在旧系数上。
 *
 * 其余限定符（方向、窗口尺寸、键盘）由各自的路径处理，不在此重推。
 */
internal fun shouldApplySettingsOnConfigurationChange(previous: Configuration, current: Configuration): Boolean =
    (previous.uiMode and Configuration.UI_MODE_NIGHT_MASK) !=
        (current.uiMode and Configuration.UI_MODE_NIGHT_MASK) ||
        previous.fontScale != current.fontScale ||
        previous.densityDpi != current.densityDpi

/**
 * 系统启动屏是否继续遮挡。
 *
 * 终端首帧已呈现就立刻撤下；首帧迟迟不来（引导未装、会话起不来、原生库缺失）时以
 * [SPLASH_MAX_WAIT_MS] 兜底，把界面（及其错误提示）还给用户，而不是让启动屏永久占屏。
 *
 * 纯函数、无 Android 依赖，便于单测；判据只有「首帧」与「等了多久」两项，故不引入状态机。
 */
internal fun shouldKeepSplashOnScreen(framePresented: Boolean, waitedMs: Long): Boolean =
    !framePresented && waitedMs < SPLASH_MAX_WAIT_MS

/**
 * 等待 bridge 出现，最多等 [BRIDGE_READY_TIMEOUT_MS] 毫秒。
 *
 * bridge 由运行期在首个会话 spawn 之后异步创建；原生暂停标志落在全局渲染器上，
 * 渲染器尚未建立时写入即丢失，故打开设置页前先等它出现。等待以时间预算表达，不用
 * 「轮询次数」——次数隐含「每次间隔恰好固定时长」的前提，负载高时实际等待会远超标称值。
 *
 * @param isReady bridge 是否已就绪的判据
 * @return 预算内就绪返回 true，超时返回 false
 */
internal suspend fun awaitBridgeReady(isReady: () -> Boolean): Boolean = withTimeoutOrNull(BRIDGE_READY_TIMEOUT_MS) {
    while (!isReady()) {
        delay(BRIDGE_READY_POLL_INTERVAL_MS)
    }
    true
} ?: false

/**
 * 会话表清空 → 关闭应用的判据。
 *
 * 冷启动时 [TerminalRuntime.state] 先以空会话表发布（会话由运行期异步创建），直接判空
 * 会在启动瞬间关掉应用，故必须记住「曾有过会话」；关闭只触发一次，使 finish 之后
 * 残留的状态发射不再重复调用 finish。纯 Kotlin、无 Android 依赖，便于单测。
 */
internal class SessionTableWatcher {
    private var sawSession = false
    private var fired = false

    /** 返回是否应关闭应用。 */
    fun onSessionCount(count: Int): Boolean {
        if (count > 0) {
            sawSession = true
            return false
        }
        if (!sawSession || fired) return false
        fired = true
        return true
    }
}
