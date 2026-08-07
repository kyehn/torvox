package terminal.emulator.runtime

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.util.Log

/**
 * Test-backdoor broadcast receivers (R6: round-3 architecture; extracted
 * from MainActivity). Instrumentation and Maestro flows trigger terminal
 * actions via same-process broadcasts; all receivers are registered with
 * RECEIVER_NOT_EXPORTED so third-party apps cannot reach them.
 */
class TestBackdoorReceivers(
    private val context: Context,
    private val onDumpTerminal: (Context) -> Unit,
    private val onInput: (String, Boolean) -> Unit,
    private val onVtWrite: (String) -> Unit,
    private val onSelectAll: () -> Unit,
    private val onPartialSelect: (startRow: Int, startCol: Int, endRow: Int, endCol: Int) -> Unit,
    private val onShowPaste: (row: Int, col: Int) -> Unit,
) {
    private val receivers: List<Pair<BroadcastReceiver, String>> =
        listOf(
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(
                        context: Context,
                        intent: Intent,
                    ) {
                        onDumpTerminal(context)
                    }
                },
                "terminal.emulator.DUMP_TERMINAL",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(
                        context: Context,
                        intent: Intent,
                    ) {
                        val text = intent.getStringExtra("text") ?: return
                        onInput(text, intent.getStringExtra("raw") == "1")
                    }
                },
                "terminal.emulator.INPUT",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(
                        context: Context,
                        intent: Intent,
                    ) {
                        onSelectAll()
                    }
                },
                "terminal.emulator.SELECT_ALL",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(
                        context: Context,
                        intent: Intent,
                    ) {
                        val text = intent.getStringExtra("text") ?: return
                        onVtWrite(text)
                    }
                },
                "terminal.emulator.VT_WRITE",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(
                        context: Context,
                        intent: Intent,
                    ) {
                        // Clamp: the receiver is NOT_EXPORTED, but
                        // instrumentation (same-uid) can still broadcast; a
                        // hostile broadcast could otherwise carry Int.MAX and
                        // trigger a multi-billion-iteration main-thread loop
                        // (ANR). A generous upper bound is enough since
                        // terminal grids are small.
                        val startRow = intent.getIntExtra("startRow", 0).coerceIn(0, 4095)
                        val startCol = intent.getIntExtra("startCol", 0).coerceIn(0, 4095)
                        val endRow = intent.getIntExtra("endRow", 2).coerceIn(0, 4095)
                        val endCol = intent.getIntExtra("endCol", 10).coerceIn(0, 4095)
                        onPartialSelect(startRow, startCol, endRow, endCol)
                    }
                },
                "terminal.emulator.PARTIAL_SELECT",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(
                        context: Context,
                        intent: Intent,
                    ) {
                        // Clamp defensively (see PARTIAL_SELECT).
                        val row = intent.getIntExtra("row", 10).coerceIn(0, 4095)
                        val col = intent.getIntExtra("col", 0).coerceIn(0, 4095)
                        onShowPaste(row, col)
                    }
                },
                "terminal.emulator.SHOW_PASTE",
            ),
        )

    fun register() {
        for ((receiver, action) in receivers) {
            context.registerReceiver(receiver, IntentFilter(action), Context.RECEIVER_NOT_EXPORTED)
        }
    }

    fun unregister() {
        for ((receiver, action) in receivers) {
            try {
                context.unregisterReceiver(receiver)
            } catch (exception: IllegalArgumentException) {
                Log.w("TestBackdoorReceivers", "unregister $action failed", exception)
            }
        }
    }
}
