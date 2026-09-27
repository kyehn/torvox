package terminal.emulator.runtime

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.util.Log

/**
 * 测试后门广播接收器。instrumentation 与 Maestro 流程经同进程广播触发终端动作；
 * 所有接收器均以 RECEIVER_NOT_EXPORTED 注册，使第三方应用无法触达。
 */
class TestBackdoorReceivers(
    private val context: Context,
    private val onDumpTerminal: (Context) -> Unit,
    private val onInput: (String, Boolean) -> Unit,
    private val onVtWrite: (String) -> Unit,
    private val onSelectAll: () -> Unit,
    private val onInstallBootstrap: (Context, String) -> Unit,
    private val onPartialSelect: (startRow: Int, startCol: Int, endRow: Int, endCol: Int) -> Unit,
    private val onShowPaste: (row: Int, col: Int) -> Unit,
) {
    private val receivers: List<Pair<BroadcastReceiver, String>> =
        listOf(
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(context: Context, intent: Intent) {
                        onDumpTerminal(context)
                    }
                },
                "terminal.emulator.DUMP_TERMINAL",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(context: Context, intent: Intent) {
                        val text = intent.getStringExtra("text") ?: return
                        onInput(text, intent.getStringExtra("raw") == "1")
                    }
                },
                "terminal.emulator.INPUT",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(context: Context, intent: Intent) {
                        onSelectAll()
                    }
                },
                "terminal.emulator.SELECT_ALL",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(context: Context, intent: Intent) {
                        val text = intent.getStringExtra("text") ?: return
                        onVtWrite(text)
                    }
                },
                "terminal.emulator.VT_WRITE",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(context: Context, intent: Intent) {
                        // 钳位：接收器虽为 NOT_EXPORTED，但 instrumentation
                        // （同 uid）仍可广播；恶意广播否则可携带 Int.MAX
                        // 并触发数十亿次迭代的主线程循环（ANR）。
                        // 终端网格很小，宽裕的上界已经足够。
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
                    override fun onReceive(context: Context, intent: Intent) {
                        // 同样防御性钳位（见 PARTIAL_SELECT）。
                        val row = intent.getIntExtra("row", 10).coerceIn(0, 4095)
                        val col = intent.getIntExtra("col", 0).coerceIn(0, 4095)
                        onShowPaste(row, col)
                    }
                },
                "terminal.emulator.SHOW_PASTE",
            ),
            Pair(
                object : BroadcastReceiver() {
                    override fun onReceive(context: Context, intent: Intent) {
                        val path = intent.getStringExtra("path") ?: return
                        onInstallBootstrap(context, path)
                    }
                },
                "terminal.emulator.INSTALL_BOOTSTRAP",
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
