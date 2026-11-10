package terminal.emulator.ui

import android.annotation.SuppressLint
import android.graphics.Matrix
import android.os.SystemClock
import android.view.KeyEvent
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.CorrectionInfo
import android.view.inputmethod.CursorAnchorInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.view.inputmethod.SurroundingText
import android.view.inputmethod.TextAttribute
import terminal.emulator.input.BatchEditDepth
import terminal.emulator.input.ComposingBaseline
import terminal.emulator.input.CursorUpdateMonitor
import terminal.emulator.input.SelectionMover
import terminal.emulator.input.admitCursorAnchor
import terminal.emulator.input.applyTerminalEditorInfo
import terminal.emulator.input.isActive
import terminal.emulator.runtime.LogUtil

/** 组字锚点查询限频：与回滚长度同款，锚点位置滞后一窗不影响候选窗口落点。 */
private const val CURSOR_ANCHOR_QUERY_THROTTLE_NANOS = 100_000_000L // 10 Hz

private const val BACKSPACE_BYTE = 0x08.toByte()

// deleteSurroundingText 参数的天花板（输入法输入不可信）：它只为防崩溃
// 并把单次 PTY 写入的字节数钉在有界范围内，**不是**「允许删这么多」——
// 实际删多少仍由输入法请求的数值决定，一次普通退格要的是 1。
private const val MAX_SURROUNDING_DELETES = 4096

/**
 * 取文类接口的查询长度上界（与 [MAX_SURROUNDING_DELETES] 同量级）。
 *
 * 输入法给出的 length 不可信；组字基线本身只是一条候选区，
 * 超出的部分按截断返回而非构造超长字符串。
 */
private const val MAX_SURROUNDING_QUERY = 4096

/** forward-delete 每次都要向原生编码一次，不可信长度须有远低于通用删除的成本上界。 */
private const val MAX_FORWARD_DELETES = 64

/**
 * `setSelection` 单次折算的最大位移（码点）。
 *
 * 输入法给的是它自己整段文本里的绝对下标，与终端内容毫无关系；
 * 不钳位会按一个荒谬的距离向 PTY 灌入海量方向键。
 */
private const val MAX_SELECTION_MOVE = 4096

/**
 * ACTION_MULTIPLE 的字符载荷。
 *
 * `KeyEvent.getCharacters()` 自 API 29 弃用（官方指向 `commitText`），但硬件键盘的
 * 多字符事件**只**经这条通道送达，而 `InputConnection` 没有接收多字符的非弃用面；
 * 保留并在此定点抑制，处置理由见 `sendKeyEvent` 的 ACTION_MULTIPLE 分支。
 */
@SuppressLint("DeprecatedCall") // 无替代面：多字符事件只经 characters 送达
@Suppress("DEPRECATION") // 同上；Kotlin 编译器告警在 -Werror 下同样需要定点抑制
private fun KeyEvent.multipleCharacters(): String = characters

/**
 * 拥有输入法 InputConnection：组字跟踪、提交/删除处理
 * 以及 EditorInfo 属性。外层以轻量转发暴露 finishComposing。
 */
internal class ImeConnection(private val surface: TerminalSurface) {
    var currentInputConnection: InputConnection? = null

    fun createInputConnection(outAttrs: EditorInfo): InputConnection {
        applyTerminalEditorInfo(outAttrs)
        val connection =
            object : BaseInputConnection(surface, true) {
                // 进行中的输入法组字：使增量得以校对而非被丢弃。
                // 基线下沉为可单测的持有者：取文接口与组字光标必须同源，
                // 否则输入法读到的与自己写下的不一致。
                private val composing = ComposingBaseline()

                // 批量编辑嵌套与选区折算都是与 Android 生命周期无关的纯状态，
                // 故下沉为可单测类型（ImeCursorContract）。
                private val batchEdits = BatchEditDepth()
                private val selectionMover = SelectionMover(maxMagnitude = MAX_SELECTION_MOVE)
                private val cursorMonitor = CursorUpdateMonitor()

                private fun encodeAndSend(text: String, ctrlActive: Boolean, altActive: Boolean) {
                    surface.inputBatchBuffer.write(
                        TerminalInputEncoder.encodeCommittedText(
                            text = text,
                            ctrlActive = ctrlActive,
                            altActive = altActive,
                        ),
                    )
                }

                /**
                 * 当前粘滞修饰键（ctrl/alt）是否生效。
                 *
                 * 输入法提交与硬键盘多码点提交都要这组状态：判定只此一处，
                 * 两条路径对「一次性修饰键是否算按住」的理解才不会分叉。
                 */
                private fun activeStickyModifiers(): Pair<Boolean, Boolean> {
                    val state = surface.viewModel?.state?.value
                    return (state?.ctrlState?.isActive() == true) to (state?.altState?.isActive() == true)
                }

                // 同一事务的退格与追加走同一出口（N1-27）：退格同步直写而追加经批缓冲
                // 异步发送时，超量追加被推迟到下一帧，退格先到即顺序反转。
                private fun sendBackspaces(count: Int) {
                    if (count > 0) surface.inputBatchBuffer.write(ByteArray(count) { BACKSPACE_BYTE })
                }

                /** 光标后删除必须走原生 forward-delete 编码，不能复用退格字节。 */
                private fun sendForwardDeletes(count: Int): Boolean {
                    if (count <= 0) return true
                    val bridge = surface.viewModel?.runtime?.bridge() ?: return false
                    repeat(count) {
                        if (!bridge.processKeyEvent(KeyEvent.KEYCODE_FORWARD_DEL, 0, 0, 0, 0)) {
                            LogUtil.warn(TerminalSurface.TAG, "forward-delete 编码失败")
                            return false
                        }
                    }
                    return true
                }

                /**
                 * 受理输入法的光标位置监视请求。
                 *
                 * 返回 true 是因为本仓**确实**会回报（见 [reportComposingCursorAnchor]）；
                 * 返回 false 会让输入法以为锚点永不更新，从此不再请求，候选窗口
                 * 也就永远贴不到真实光标。
                 */
                override fun requestCursorUpdates(cursorUpdateMode: Int): Boolean {
                    cursorMonitor.update(cursorUpdateMode)
                    reportComposingCursorAnchor(immediate = true)
                    return true
                }

                /**
                 * 把组字光标锚点回报给输入法。
                 *
                 * 准入与落格见 [composingCursorMarker]；坐标取最近一帧的终端光标格——
                 * 组字字节正写进那一格。无组字或输入法未请求监视时不回报，
                 * 否则候选窗口会被贴到不属于它的格子上。
                 *
                 * @param immediate 跳过限频。输入法刚改监视模式时要立刻拿到位置，
                 *   等下一次组字变化会让候选窗口先落在旧格上。
                 */
                private fun reportComposingCursorAnchor(immediate: Boolean = false) {
                    val inputMethodManager =
                        surface.context.getSystemService(InputMethodManager::class.java) ?: return
                    // 准入判据先于取坐标：未请求监视或无组字时连帧内光标都不必取。
                    //
                    // 末项是「输入法连接失活」：连接失活时平台丢弃这次更新，并在
                    // 输入法侧记一条 "requestCursorAnchorInfo on inactive
                    // InputConnection" 的告警——应用侧不得再主动触发这次回报。
                    // 判据取公开的 `InputMethodManager.isActive(view)`，其条件与
                    // 平台自身 `updateCursorAnchorInfo` 的前置条件一致。
                    if (!admitCursorAnchor(
                            monitoring = cursorMonitor.isMonitoring,
                            composing = composing.isComposing,
                            viewFocused = surface.hasWindowFocus() && surface.isFocused,
                            imeConnected = {
                                inputMethodManager.isActive(surface)
                            },
                        )
                    ) {
                        return
                    }
                    if (!immediate) {
                        val now = System.nanoTime()
                        if (now - surface.lastCursorAnchorQueryNanos < CURSOR_ANCHOR_QUERY_THROTTLE_NANOS) return
                    }
                    val runtime = surface.viewModel?.runtime ?: return
                    // 取**最近一帧**的光标：渲染线程已把它发布在会话条目上，这里只做一次
                    // 原子读。旧路每次组字都发一次同步 VT 查询，实测回退一键要等
                    // 18ms(p50)/38ms(p95)，全部落在主线程上。
                    val packed = runtime.bridge()?.renderedCursorPacked() ?: return
                    if (packed < 0) return
                    val cursorRow = (packed shr 32).toInt()
                    val cursorCol = (packed and 0xFFFFFFFFL).toInt()
                    val marker =
                        composingCursorMarker(
                            cursorRow = cursorRow,
                            cursorCol = cursorCol,
                            rows = surface.rows,
                            cellWidth = surface.cellWidth,
                            cellHeight = surface.cellHeight,
                        ) ?: return
                    // 时间戳只在**真的产出了锚点**之后推进（同回滚长度查询）：
                    // `packed < 0` 是取不到光标（首帧未到或光标不可见），标记为 null 则是
                    // 光标落在视口外，两者都不该吃掉限频窗口。
                    surface.lastCursorAnchorQueryNanos = System.nanoTime()
                    val info =
                        CursorAnchorInfo.Builder()
                            // 局部→屏幕的折算矩阵：只给定位参数而不给矩阵时
                            // `CursorAnchorInfo.Builder.build()` 直接抛
                            // （坐标是视图局部的，平台不会替调用方补这一步）。
                            .setMatrix(cursorAnchorMatrix())
                            .setComposingText(0, composing.text)
                            .setInsertionMarkerLocation(
                                marker.horizontal,
                                marker.top,
                                marker.baseline,
                                marker.bottom,
                                0,
                            )
                            .build()
                    inputMethodManager.updateCursorAnchorInfo(surface, info)
                }

                /** [getLocationOnScreen] 的落点，复用以免逐键分配。仅主线程使用。 */
                private val anchorLocation = IntArray(2)

                /** 局部→屏幕的换算矩阵，复用同上。 */
                private val anchorMatrix = Matrix()

                /**
                 * 锚点坐标（视图局部）→ 屏幕坐标的矩阵。
                 *
                 * 本视图不在屏幕原点，且输入法弹出时**自身**就被 `translationY` 上移
                 * （见 [applyImeShift]）——`getLocationOnScreen` 每次都把当前位移算进去，
                 * 故候选窗口跟着光标走，不会停在未上移的旧位置。
                 */
                private fun cursorAnchorMatrix(): Matrix {
                    surface.getLocationOnScreen(anchorLocation)
                    anchorMatrix.setTranslate(anchorLocation[0].toFloat(), anchorLocation[1].toFloat())
                    return anchorMatrix
                }

                /**
                 * 把组字基线改写为目标基线：差分重放 + 光标对齐。
                 *
                 * 组字区在终端里没有可寻址的文本模型，只能靠「方向键 + 退格 + 重打」
                 * 复现。退格删的是光标前的字符，故重放前 MUST 先把光标右移到旧组字区
                 * 末尾，重放后再挪到目标组字光标。
                 *
                 * @param nextCursor 目标组字光标在 [nextText] 中的绝对下标。
                 */
                private fun rewriteComposingBaseline(nextText: String, nextCursor: Int): Boolean {
                    // 纯校对逻辑（ComposingDiff），已单元测试
                    // ——增长/回退/全量重写三种情况集中在一处。
                    val replay =
                        ComposingDiff.planReplay(
                            composing.text,
                            composing.cursor,
                            nextText,
                            nextCursor,
                        )
                    // 供自动化输入法验证的锚点——日志序列必须与注入的组字文本一一对应。
                    // 门控在调用处而非只靠 LogUtil.debug：release 下可省掉整条消息串的拼接。
                    if (terminal.emulator.BuildConfig.DEBUG) {
                        LogUtil.debug(
                            "ComposingDiff",
                            "reconcile prev=${composing.text.length}ch next=$nextText " +
                                "bs=${replay.backspaces} app=${replay.append.length}ch " +
                                "right=${replay.rightMovesBeforeEdit} " +
                                "left=${replay.leftMovesAfterEdit}",
                        )
                    }
                    // 退格删的是光标前的字符：组字光标在组字区中间时必须先挪到末尾，
                    // 否则退格删掉的是组字区之前已提交的内容。
                    repeat(replay.rightMovesBeforeEdit) {
                        sendCursorMove(KeyEvent.KEYCODE_DPAD_RIGHT)
                    }
                    if (replay.backspaces > 0) {
                        sendBackspaces(replay.backspaces)
                    }
                    if (replay.append.isNotEmpty()) {
                        encodeAndSend(
                            replay.append,
                            ctrlActive = false,
                            altActive = false,
                        )
                    }
                    composing.composeAt(nextText, nextCursor)
                    // 差分重放后 PTY 光标位于文本末尾；组字光标在中间时必须同步左移，
                    // 否则输入法按基线在中间改写，字节却落在末尾。
                    repeat(replay.leftMovesAfterEdit) {
                        sendCursorMove(KeyEvent.KEYCODE_DPAD_LEFT)
                    }
                    reportComposingCursorAnchor()
                    return true
                }

                override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean {
                    // 交付不按焦点状态整段拒收（见 onWindowFocusChanged 注释）：
                    // 只有切换瞬间的有界抑制窗口会让输入法重试——返回 false 时
                    // 组字基线不得清、字节不得吞，才能靠重发恢复（N1-24）。
                    if (System.nanoTime() < surface.suppressUntilNanos) return false
                    val nextText = text?.toString() ?: ""
                    return rewriteComposingBaseline(
                        nextText,
                        composing.resolveCursor(nextText, newCursorPosition),
                    )
                }

                override fun finishComposingText(): Boolean {
                    // 暂停时不得清基线：PTY 已收到组字增量，失焦清空后输入法
                    // 重新提交会按空基线算出整段追加，同一段文字被写入两次。
                    // 这是基线不变量，与是否接受交付无关——故此处**不**因
                    // 焦点丢失而阻塞交付，只拒绝「清空基线」这一动作。
                    if (surface.isPaused) return false
                    composing.clear()
                    return true
                }

                override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean {
                    // 抑制窗口内返回 false 让输入法重发该提交，
                    // 返回 true 等于声称已处理，字符永久丢失（N1-24）。
                    if (System.nanoTime() < surface.suppressUntilNanos) {
                        if (terminal.emulator.BuildConfig.DEBUG) {
                            LogUtil.debug(
                                TerminalSurface.TAG,
                                "commitText 在抑制窗口内被拒，" +
                                    "剩余 ${(surface.suppressUntilNanos - System.nanoTime()) / 1_000_000}ms",
                            )
                        }
                        return false
                    }
                    val committedText = text?.toString() ?: return false
                    val (ctrlActive, altActive) = activeStickyModifiers()

                    if (composing.isComposing) {
                        if (committedText == composing.text) {
                            // 已经组字增量转发；不再重发。
                        } else {
                            // 退格删的是光标前的字符：组字光标在组字区中间时必须先挪到
                            // 末尾，否则删掉的是组字区之前已提交的内容。
                            val clear = ComposingDiff.planClear(composing.text, composing.cursor)
                            repeat(clear.rightMovesBeforeEdit) {
                                sendCursorMove(KeyEvent.KEYCODE_DPAD_RIGHT)
                            }
                            sendBackspaces(clear.backspaces)
                            encodeAndSend(committedText, ctrlActive, altActive)
                        }
                        composing.clear()
                    } else {
                        encodeAndSend(committedText, ctrlActive, altActive)
                    }
                    // One-shot 修饰键随本次提交一起消费：编码时已按住
                    // ctrl/alt，本次提交后 Once 必须回到 Off，否则粘滞键
                    // 永远不消失。Locked 不受影响（consume 只清 Once）。
                    // 注意：必须在编码之后消费，且只消费一次——
                    // 无修饰提交不清，避免偷走点亮后尚未使用的 Once。
                    if (ctrlActive || altActive) {
                        surface.viewModel?.consumeOneShotModifiers()
                    }
                    // 不在此回报锚点：组字刚被清空，锚点的准入条件已不满足，
                    // 任何一次回报都必然被丢弃。
                    return true
                }

                /**
                 * 平台已弃用 `ACTION_MULTIPLE` 与 `KeyEvent.characters`，但
                 * compileSdk 37 仍无公开替代面（`InputConnection` 只有
                 * `commitText`，没有 `commitMultipleText`），而硬键盘的多字符
                 * 输入（如蓝牙键盘的粘贴键）**只**经这条通道送达——丢弃它
                 * 等于砍掉该硬件输入。故在此处最小范围抑制并按码点直接提交。
                 */
                @Suppress("DEPRECATION")
                override fun sendKeyEvent(event: KeyEvent): Boolean {
                    // 抑制窗口内**拒绝**：返回 true 等于谎报已处理，而事件并未
                    // 送达——硬键盘多字符没有第二条通道，丢字不可恢复，故据实拒绝。
                    // 口径与其余写入路径一致（皆返回 false），且只限这一个有界窗口：
                    // 焦点丢失本身不拒收（见 onWindowFocusChanged 注释）。
                    if (System.nanoTime() < surface.suppressUntilNanos) {
                        return false
                    }
                    return when (event.action) {
                        // 多字符事件只给 characters，没有可映射的键码：
                        // 按码点直接提交，绕开键码映射。
                        KeyEvent.ACTION_MULTIPLE ->
                            commitEventCharacters(event.multipleCharacters())

                        KeyEvent.ACTION_DOWN -> surface.handleKeyEvent(event)

                        else -> true
                    }
                }

                override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
                    // 抑制窗口内返回 false 让输入法重试（与组字/提交同形，N1-24）。
                    if (System.nanoTime() < surface.suppressUntilNanos) return false
                    // beforeLength/afterLength 来自输入法（不可信）：
                    // 负值或巨大值会在主线程上以 NegativeArraySizeException /
                    // OutOfMemoryError 崩溃，故两侧都先钳到上界。
                    //
                    // 上界取「组字长度与单行上限中的**较大**者」（coerceAtLeast，
                    // 不是 coerceAtMost）：组字本身长于单行上限时（粘贴进来的一大
                    // 段组字）仍要能整段删掉，而组字为空时也不该把上界钳成 0。
                    // 它只是防崩溃的天花板——实际删多少仍由输入法请求的数值决定。
                    // 注意：beforeLength 按码点计数，退格按码点 1:1 发送
                    // （shell 行编辑按字符删除，一个 0x08 删掉整个汉字，
                    // 真机实测锁定：见 CjkBackspaceSemanticsTest）。
                    // 不得按终端列宽加倍（CJK 发 2 个只会多删一个字符）。
                    val maxDeletes = composing.text.length.coerceAtLeast(MAX_SURROUNDING_DELETES)
                    val safeBefore = beforeLength.coerceIn(0, maxDeletes)
                    val maxAfter = if (composing.isComposing) maxDeletes else MAX_FORWARD_DELETES
                    val safeAfter = afterLength.coerceIn(0, maxAfter)
                    if (composing.isComposing) {
                        // 组字光标可能在候选区中间。按目标内容重放差分，
                        // 同时让基线、PTY 与输入法的删除结果保持一致。
                        val target = composing.deletionTarget(safeBefore, safeAfter)
                        return if (target.beforeCount + target.afterCount == 0) {
                            true
                        } else {
                            rewriteComposingBaseline(target.text, target.cursor)
                        }
                    }
                    if (safeBefore > 0) {
                        val directBs = ByteArray(safeBefore) { BACKSPACE_BYTE }
                        surface.inputBatchBuffer.write(directBs)
                    }
                    return sendForwardDeletes(safeAfter)
                }

                // ── 取文接口：只反映组字基线 ──────────────────────
                //
                // `BaseInputConnection` 的默认取自持 editable，而本仓从不读它。
                // 不覆写这些接口，输入法读到的是空 editable：据此判定字段没有
                // 内容，于是提交后看不到自己刚写的字，随即整词重发——终端里
                // 出现重复文本。这是输入法能力「静默缺失」的主因。
                //
                // 一律只回答组字基线，绝不掺入屏幕或回滚文本：否则输入法
                // 会把终端任意位置的字符当成自己的候选区去改写。
                override fun getTextBeforeCursor(length: Int, flags: Int): CharSequence =
                    composing.textBefore(length.coerceAtMost(MAX_SURROUNDING_QUERY))

                override fun getTextAfterCursor(length: Int, flags: Int): CharSequence =
                    composing.textAfter(length.coerceAtMost(MAX_SURROUNDING_QUERY))

                /**
                 * 提取的文本同样只反映组字基线；选区按当前组字光标给出，
                 * 输入法据此显示候选窗口与光标位置。
                 *
                 * `partialEndOffset = -1` 是平台「未选择」的同口径值；填 0 会
                 * 声称选区在文本开头，使输入法把后续编辑当作整段替换。
                 */
                override fun getExtractedText(request: ExtractedTextRequest?, flags: Int): ExtractedText {
                    val extracted = ExtractedText()
                    extracted.text = composing.text
                    extracted.partialStartOffset = 0
                    extracted.partialEndOffset = if (composing.isComposing) -1 else 0
                    extracted.selectionStart = composing.cursor
                    extracted.selectionEnd = composing.cursor
                    extracted.flags = flags
                    return extracted
                }

                /**
                 * 周边文本：`SurroundingText` 是「一段文本 + 选区」的结构，
                 * 故返回整条组字基线并把当前组字光标标为选区。
                 *
                 * 只报基线本身（不掺屏幕文本）：输入法据此在自己写下的范围内
                 * 做上下文判断，而不会把终端任意位置的字符当作自己的内容。
                 */
                override fun getSurroundingText(beforeLength: Int, afterLength: Int, flags: Int): SurroundingText =
                    SurroundingText(composing.text, composing.cursor, composing.cursor, 0)

                // ── 替换与改写区域 ──────────────────────────────
                //
                // 两者都只允许改写**输入法自己写下的组字基线**：
                // 输入法可能给出越界下标，而终端里没有可寻址的文本编辑模型，
                // 越界猜测会让已提交内容被错误删改。故范围不落在基线内即
                // 返回 false（拒绝）且**发出零字节**，让输入法重试。

                /**
                 * 把 `[start, end)` 替换为 [text] 并重设光标（API 33+）。
                 *
                 * 目标组字串按「头 + 新文本 + 尾」拼出，再交既有差分出口送达，
                 * 与组字增量同一条路径：光标先对齐到旧组字区末尾，重放差分后
                 * 再落到目标组字光标。
                 */
                override fun replaceText(
                    start: Int,
                    end: Int,
                    text: CharSequence,
                    newCursorPosition: Int,
                    attrs: TextAttribute?,
                ): Boolean {
                    // 抑制窗口内返回 false 让输入法重试（与组字/提交同形，N1-24）。
                    if (System.nanoTime() < surface.suppressUntilNanos) return false
                    val replacement = text.toString()
                    val target =
                        composing.replaceRange(start, end, replacement)
                            ?: return false
                    // 光标按平台口径落在被插入文本处：正数相对其末尾减一，
                    // 非正数相对其开头（`replaceText` 的 `newCursorPosition` 语义）。
                    val targetCursor =
                        composing.replacedCursor(
                            start = start,
                            end = end,
                            insertedLength = replacement.length,
                            newCursorPosition = newCursorPosition,
                        )
                    return rewriteComposingBaseline(target, targetCursor)
                }

                /**
                 * 把组字区域重锚到 `[start, end)`：只重标定「哪一段正在组字」，
                 * 不引入新文本，区间外的已提交内容必须原样保留。
                 *
                 * 平台语义如此：重锚**不改文本**。本仓的组字基线整体即组字区，
                 * 故只受理覆盖整段基线的重锚（零字节、零光标移动）；子区间重锚
                 * 要求后续组字只替换该子区间，而本仓的差分出口以整段基线为单位，
                 * 无从处置区间外的已提交内容——如实拒绝，不发字节。
                 */
                override fun setComposingRegion(start: Int, end: Int): Boolean {
                    if (System.nanoTime() < surface.suppressUntilNanos) return false
                    if (!composing.acceptsRegionReanchor(start, end)) return false
                    // 文本与光标均未改变：锚点无变化，不回报。
                    return true
                }

                /**
                 * 按 [KeyEvent.characters] 的码点直接提交，不经键码映射。
                 *
                 * 与 `commitText` 的区别只在于修饰键来源：`ACTION_MULTIPLE`
                 * 是硬键盘发来的，输入法无法给出粘滞修饰键状态，故按裸文本提交。
                 */
                private fun commitEventCharacters(characters: CharSequence?): Boolean {
                    val text = characters?.toString().orEmpty()
                    if (text.isEmpty()) return true
                    val (ctrlActive, altActive) = activeStickyModifiers()
                    encodeAndSend(text, ctrlActive, altActive)
                    if (ctrlActive || altActive) {
                        surface.viewModel?.consumeOneShotModifiers()
                    }
                    return true
                }

                // ── 批量编辑与选区折算 ──────────────────────

                override fun beginBatchEdit(): Boolean {
                    batchEdits.begin()
                    return true
                }

                override fun endBatchEdit(): Boolean {
                    batchEdits.end()
                    return true
                }

                /**
                 * 把输入法给出的**绝对**选区折成终端可发送的左右移动。
                 *
                 * 只移动光标而不改写组字文本：读回的文本必须逐字符不变，
                 * 否则输入法会把自己的移动误认为内容变更而重发整段。
                 */
                override fun setSelection(start: Int, end: Int): Boolean {
                    if (System.nanoTime() < surface.suppressUntilNanos) return false
                    val delta = selectionMover.moveTo(end)
                    if (delta.isEmpty) return true
                    repeat(delta.rightward()) { sendCursorMove(KeyEvent.KEYCODE_DPAD_RIGHT) }
                    repeat(delta.leftward()) { sendCursorMove(KeyEvent.KEYCODE_DPAD_LEFT) }
                    return true
                }

                /**
                 * 校正请求一律拒绝：6.1 已证默认路径不分发校正，校正文本不可能
                 * 经提交路径送达，受理即谎称生效；拒绝让输入法停止调用更安全。
                 */
                override fun commitCorrection(info: CorrectionInfo?): Boolean = false

                /** 合成一次方向键按下并走既有键事件路径（`setSelection` 折算用）。 */
                private fun sendCursorMove(keyCode: Int) {
                    val now = SystemClock.uptimeMillis()
                    surface.handleKeyEvent(KeyEvent(now, now, KeyEvent.ACTION_DOWN, keyCode, 0))
                }
            }
        currentInputConnection = connection
        return connection
    }
}
