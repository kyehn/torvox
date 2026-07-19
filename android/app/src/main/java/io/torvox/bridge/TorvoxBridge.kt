@file:Suppress("ktlint:standard:function-naming")

package io.torvox.bridge

import android.util.Log
import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer

private const val LOW_16_MASK = 0xFFFFL
private const val JNA_POINTER_SIZE = 8L
private const val JNA_INT_SIZE = 4L

// ── Wire encoding helpers ──────────────────────────────────────────────

internal class WireWriter {
    private val buffer = mutableListOf<Byte>()

    fun writeByte(value: Byte) {
        buffer.add(value)
    }

    fun writeI32(value: Int) {
        buffer.add((value and 0xFF).toByte())
        buffer.add(((value shr 8) and 0xFF).toByte())
        buffer.add(((value shr 16) and 0xFF).toByte())
        buffer.add(((value shr 24) and 0xFF).toByte())
    }

    fun writeU32(value: UInt) = writeI32(value.toInt())

    fun writeString(value: String) {
        val bytes = value.toByteArray(Charsets.UTF_8)
        writeI32(bytes.size)
        buffer.addAll(bytes.toList())
    }

    fun toByteArray(): ByteArray = buffer.toByteArray()
}

class WireReader(
    data: ByteArray,
) {
    private val data = data
    private var position = 0

    fun readBool(): Boolean {
        val value = data[position]
        position += 1
        return value != 0.toByte()
    }

    fun readByte(): Byte {
        val value = data[position]
        position += 1
        return value
    }

    fun readI32(): Int {
        require(data.size - position >= Int.SIZE_BYTES) {
            "WireReader.readI32: need ${Int.SIZE_BYTES} bytes at offset $position " +
                "but only ${data.size - position} remain in buffer of size ${data.size}"
        }
        val value =
            (data[position].toInt() and 0xFF) or ((data[position + 1].toInt() and 0xFF) shl 8) or
                ((data[position + 2].toInt() and 0xFF) shl 16) or ((data[position + 3].toInt() and 0xFF) shl 24)
        position += 4
        return value
    }

    fun readU32(): UInt = readI32().toUInt()

    fun readString(): String {
        val length = readI32()
        if (length == 0) return ""
        require(length > 0) {
            "WireReader.readString: invalid negative length $length"
        }
        require(data.size - position >= length) {
            "WireReader.readString: need $length bytes at offset $position " +
                "but only ${data.size - position} remain in buffer of size ${data.size}"
        }
        val bytes = data.copyOfRange(position, position + length)
        position += length
        return String(bytes, Charsets.UTF_8)
    }
}

// ── Data types matching Rust #[boltffi::data] ─────────────────────────

sealed class Shell {
    object SystemDefault : Shell()

    data class Custom(
        val path: String,
    ) : Shell()

    internal fun wireEncode(writer: WireWriter) {
        when (this) {
            is SystemDefault -> {
                writer.writeI32(0)
            }

            is Custom -> {
                writer.writeI32(1)
                writer.writeString(path)
            }
        }
    }
}

data class BridgeTheme(
    val name: String = "",
    val bg: Int = 0,
    val fg: Int = 0,
    val cursor: Int = 0,
    val selectionBg: Int = 0,
    val ansi0: Int = 0,
    val ansi1: Int = 0,
    val ansi2: Int = 0,
    val ansi3: Int = 0,
    val ansi4: Int = 0,
    val ansi5: Int = 0,
    val ansi6: Int = 0,
    val ansi7: Int = 0,
    val ansi8: Int = 0,
    val ansi9: Int = 0,
    val ansi10: Int = 0,
    val ansi11: Int = 0,
    val ansi12: Int = 0,
    val ansi13: Int = 0,
    val ansi14: Int = 0,
    val ansi15: Int = 0,
) {
    internal fun wireEncode(writer: WireWriter) {
        writer.writeString(name)
        writer.writeI32(bg)
        writer.writeI32(fg)
        writer.writeI32(cursor)
        writer.writeI32(selectionBg)
        writer.writeI32(ansi0)
        writer.writeI32(ansi1)
        writer.writeI32(ansi2)
        writer.writeI32(ansi3)
        writer.writeI32(ansi4)
        writer.writeI32(ansi5)
        writer.writeI32(ansi6)
        writer.writeI32(ansi7)
        writer.writeI32(ansi8)
        writer.writeI32(ansi9)
        writer.writeI32(ansi10)
        writer.writeI32(ansi11)
        writer.writeI32(ansi12)
        writer.writeI32(ansi13)
        writer.writeI32(ansi14)
        writer.writeI32(ansi15)
    }

    fun wireEncodeBytes(): ByteArray {
        val writer = WireWriter()
        wireEncode(writer)
        return writer.toByteArray()
    }

    companion object {
        @Suppress("FunctionNaming")
        fun wireDecode(reader: WireReader): BridgeTheme =
            BridgeTheme(
                name = reader.readString(),
                bg = reader.readI32(),
                fg = reader.readI32(),
                cursor = reader.readI32(),
                selectionBg = reader.readI32(),
                ansi0 = reader.readI32(),
                ansi1 = reader.readI32(),
                ansi2 = reader.readI32(),
                ansi3 = reader.readI32(),
                ansi4 = reader.readI32(),
                ansi5 = reader.readI32(),
                ansi6 = reader.readI32(),
                ansi7 = reader.readI32(),
                ansi8 = reader.readI32(),
                ansi9 = reader.readI32(),
                ansi10 = reader.readI32(),
                ansi11 = reader.readI32(),
                ansi12 = reader.readI32(),
                ansi13 = reader.readI32(),
                ansi14 = reader.readI32(),
                ansi15 = reader.readI32(),
            )
    }
}

data class TerminalConfig(
    val shell: Shell = Shell.SystemDefault,
    val rows: UInt = DEFAULT_ROWS,
    val cols: UInt = DEFAULT_COLS,
    val scrollbackLines: UInt = DEFAULT_SCROLLBACK,
    val font_size_tenths: UInt = DEFAULT_FONT_SIZE_TENTHS,
    val theme: BridgeTheme = BridgeTheme(),
    val home: String = "",
    val user: String = "",
    val path: String = "",
    val workingDirectory: String = "",
    val prefix: String = "",
) {
    fun wireEncode(): ByteArray {
        val writer = WireWriter()
        shell.wireEncode(writer)
        writer.writeU32(rows)
        writer.writeU32(cols)
        writer.writeU32(scrollbackLines)
        writer.writeU32(font_size_tenths)
        theme.wireEncode(writer)
        writer.writeString(home)
        writer.writeString(user)
        writer.writeString(path)
        writer.writeString(workingDirectory)
        writer.writeString(prefix)
        return writer.toByteArray()
    }

    companion object {
        private const val DEFAULT_ROWS = 24u
        private const val DEFAULT_COLS = 80u
        private const val DEFAULT_SCROLLBACK = 50_000u
        private const val DEFAULT_FONT_SIZE_TENTHS = 140u

        @Suppress("FunctionNaming")
        fun wireDecode(reader: WireReader): TerminalConfig =
            TerminalConfig(
                shell =
                    when (reader.readI32()) {
                        0 -> Shell.SystemDefault
                        1 -> Shell.Custom(reader.readString())
                        else -> Shell.SystemDefault
                    },
                rows = reader.readU32(),
                cols = reader.readU32(),
                scrollbackLines = reader.readU32(),
                font_size_tenths = reader.readU32(),
                theme = BridgeTheme.wireDecode(reader),
                home = reader.readString(),
                user = reader.readString(),
                path = reader.readString(),
                workingDirectory = reader.readString(),
                prefix = reader.readString(),
            )
    }
}

// ── JNA native interface ──────────────────────────────────────────────

@Suppress("FunctionNaming")
private interface TorvoxNative : Library {
    fun boltffi_torvox_bridge_new(
        config_ptr: ByteArray?,
        config_len: Int,
    ): Long

    fun boltffi_torvox_bridge_free(handle: Long)

    fun boltffi_torvox_bridge_ping(handle: Long): Int

    fun torvox_bridge_ping(handle: Long): Int

    // / Global logger: initialise the Rust logging (idempotent).
    // / Called once from TorvoxApp.onCreate.
    fun torvox_init_logger()

    // / Global logger: set the log file path for Rust log output.
    fun torvox_set_log_file_path(
        path: ByteArray,
        path_len: Int,
    )

    // Raw C-ABI wrappers for methods with scalar parameters
    fun torvox_bridge_set_native_window(
        handle: Long,
        window_ptr_low: Int,
        window_ptr_high: Int,
        width: Int,
        height: Int,
    ): Int

    fun boltffi_torvox_bridge_set_native_window(
        handle: Long,
        window_ptr: Long,
        width: Int,
        height: Int,
    ): Int

    fun boltffi_torvox_bridge_resize(
        handle: Long,
        rows: Int,
        cols: Int,
    ): Int

    fun torvox_bridge_resize(
        handle: Long,
        rows: Int,
        cols: Int,
    ): Int

    fun boltffi_torvox_bridge_recompute_grid(
        handle: Long,
        width: Int,
        height: Int,
    ): Int

    fun torvox_bridge_recompute_grid(
        handle: Long,
        width: Int,
        height: Int,
    ): Int

    fun boltffi_torvox_bridge_set_surface_size(
        handle: Long,
        width: Int,
        height: Int,
    )

    fun torvox_bridge_set_surface_size(
        handle: Long,
        width: Int,
        height: Int,
    )

    fun torvox_bridge_update_native_window(
        handle: Long,
        window_ptr_low: Int,
        window_ptr_high: Int,
        width: Int,
        height: Int,
    ): Int

    fun boltffi_torvox_bridge_update_native_window(
        handle: Long,
        window_ptr: Long,
        width: Int,
        height: Int,
    ): Int

    fun boltffi_torvox_bridge_spawn_terminal(
        handle: Long,
        rows: Int,
        cols: Int,
    ): Int

    fun torvox_bridge_spawn_terminal(
        handle: Long,
        rows: Int,
        cols: Int,
    ): Int

    fun boltffi_torvox_bridge_release_surface(handle: Long)

    fun torvox_bridge_release_surface(handle: Long)

    fun boltffi_torvox_bridge_release_gpu_surface(handle: Long)

    fun torvox_bridge_release_gpu_surface(handle: Long)

    fun boltffi_torvox_bridge_get_config(handle: Long): Pointer?

    fun boltffi_torvox_bridge_get_theme_names(handle: Long): Pointer?

    fun boltffi_torvox_bridge_list_fonts(handle: Long): Pointer?

    fun boltffi_torvox_bridge_load_font_file(
        handle: Long,
        path: ByteArray,
        pathLen: Int,
    ): Pointer?

    fun torvox_bridge_load_font_file(
        handle: Long,
        path: ByteArray,
        pathLen: Int,
    ): Long

    fun boltffi_torvox_bridge_get_theme(
        handle: Long,
        name: String?,
    ): Pointer?

    // Raw C-ABI wrappers
    fun boltffi_torvox_bridge_render(handle: Long): Int

    fun torvox_bridge_render(
        handle: Long,
        skipOutput: Byte,
    ): Int

    fun torvox_bridge_get_snapshot(
        handle: Long,
        scroll_offset: Int,
        buf: ByteArray,
        buf_len: Int,
    ): Int

    fun boltffi_torvox_bridge_poll_bel(handle: Long): Int

    fun torvox_bridge_poll_bel(handle: Long): Int

    fun boltffi_torvox_bridge_save_test_frame(
        handle: Long,
        dataDir: Pointer,
    ): Int

    fun torvox_bridge_save_test_frame(
        handle: Long,
        dataDir: Pointer,
    ): Int

    @Suppress("LongParameterList")
    fun boltffi_torvox_bridge_save_test_frame_with_selection(
        handle: Long,
        dataDir: Pointer,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        active: Int,
        mode: Int,
    ): Int

    @Suppress("LongParameterList")
    fun torvox_bridge_save_test_frame_with_selection(
        handle: Long,
        dataDir: Pointer,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        active: Int,
        mode: Int,
    ): Int

    fun boltffi_torvox_bridge_poll_clipboard(handle: Long): Long

    fun torvox_bridge_poll_clipboard(handle: Long): Long

    fun boltffi_torvox_bridge_poll_notification(handle: Long): Long

    fun torvox_bridge_poll_notification(handle: Long): Long

    fun boltffi_torvox_bridge_poll_shell_integration(handle: Long): Int

    fun torvox_bridge_poll_shell_integration(handle: Long): Int

    fun boltffi_torvox_bridge_poll_sync_active(handle: Long): Int

    fun torvox_bridge_poll_sync_active(handle: Long): Int

    fun boltffi_torvox_bridge_cwd(handle: Long): Pointer?

    fun torvox_bridge_cwd(handle: Long): Pointer?

    fun boltffi_torvox_bridge_free_cstring(pointer: Pointer?)

    fun torvox_bridge_free_cstring(pointer: Pointer?)

    fun boltffi_torvox_bridge_focus_event(
        handle: Long,
        focused: Int,
    )

    fun torvox_bridge_focus_event(
        handle: Long,
        focused: Int,
    )

    fun boltffi_torvox_bridge_scrollback_len(handle: Long): Int

    fun torvox_bridge_scrollback_len(handle: Long): Int

    // Raw C-ABI wrappers for string/byte-array/scalar methods

    fun boltffi_torvox_bridge_set_save_path(
        handle: Long,
        path_ptr: ByteArray?,
        path_len: Int,
    ): Int

    // Manual FFI functions for path-based operations (avoid boltffi wire format issues)
    fun torvox_bridge_set_save_path(
        handle: Long,
        path_ptr: ByteArray?,
        path_len: Int,
    ): Int

    fun torvox_bridge_has_saved_session(
        handle: Long,
        path_ptr: ByteArray?,
        path_len: Int,
    ): Boolean

    fun torvox_bridge_save_session(
        handle: Long,
        path_ptr: ByteArray?,
        path_len: Int,
    ): Int

    fun torvox_bridge_restore_session(
        handle: Long,
        path_ptr: ByteArray?,
        path_len: Int,
    ): Int

    fun boltffi_torvox_bridge_has_saved_session(
        handle: Long,
        path_ptr: ByteArray?,
        path_len: Int,
    ): Boolean

    fun boltffi_torvox_bridge_write_to_pty(
        handle: Long,
        data_ptr: ByteArray?,
        data_len: Int,
    ): Int

    fun torvox_bridge_write_to_pty(
        handle: Long,
        data_ptr: ByteArray?,
        data_len: Int,
    ): Int

    fun boltffi_torvox_bridge_process_key_event(
        handle: Long,
        key_code: Int,
        modifiers: Byte,
        action: Byte,
        unicode_char: Int,
        unshifted_char: Int,
    ): Int

    fun torvox_bridge_process_key_event(
        handle: Long,
        key_code: Int,
        modifiers: Byte,
        action: Byte,
        unicode_char: Int,
        unshifted_char: Int,
    ): Int

    fun boltffi_torvox_bridge_set_font_size(
        handle: Long,
        size_tenths: Int,
    ): Int

    fun torvox_bridge_set_font_size(
        handle: Long,
        size_tenths: Int,
    ): Int

    fun boltffi_torvox_bridge_set_font_size_in_place(
        handle: Long,
        size_tenths: Int,
    ): Int

    fun torvox_bridge_set_font_size_in_place(
        handle: Long,
        size_tenths: Int,
    ): Int

    fun boltffi_torvox_bridge_set_extra_font_paths(
        handle: Long,
        paths_ptr: com.sun.jna.Pointer?,
        lens_ptr: com.sun.jna.Pointer?,
        count: Int,
    ): Int

    fun torvox_bridge_set_extra_font_paths(
        handle: Long,
        paths_ptr: com.sun.jna.Pointer?,
        lens_ptr: com.sun.jna.Pointer?,
        count: Int,
    ): Int

    @Suppress("LongParameterList", "FunctionParameterNaming")
    fun boltffi_torvox_bridge_set_selection(
        handle: Long,
        start_row: Int,
        start_col: Int,
        end_row: Int,
        end_col: Int,
        active: Int,
        mode: Int,
    ): Int

    fun torvox_bridge_set_selection(
        handle: Long,
        start_row: Int,
        start_col: Int,
        end_row: Int,
        end_col: Int,
        active: Int,
        mode: Int,
    ): Int

    fun boltffi_torvox_bridge_expand_and_set_selection(
        handle: Long,
        row: Int,
        col: Int,
        mode: Int,
    ): Long

    fun torvox_bridge_expand_and_set_selection(
        handle: Long,
        row: Int,
        col: Int,
        mode: Int,
    ): Long

    fun torvox_bridge_is_cell_empty(
        handle: Long,
        row: Int,
        col: Int,
    ): Int

    fun torvox_bridge_has_text_in_range(
        handle: Long,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
    ): Int

    fun torvox_bridge_set_selection_endpoint(
        handle: Long,
        handleSide: Byte,
        anchorRow: Int,
        anchorCol: Int,
        otherRow: Int,
        otherCol: Int,
        mode: Int,
        originRow: Int,
        originCol: Int,
    ): Long

    fun boltffi_torvox_bridge_set_search_highlights(
        handle: Long,
        data_ptr: ByteArray?,
        data_len: Int,
    ): Int

    fun torvox_bridge_set_search_highlights(
        handle: Long,
        data_ptr: ByteArray?,
        data_len: Int,
    ): Int

    fun boltffi_torvox_bridge_set_font_family(
        handle: Long,
        family_ptr: ByteArray?,
        family_len: Int,
    ): Int

    fun torvox_bridge_set_font_family(
        handle: Long,
        family_ptr: ByteArray?,
        family_len: Int,
    ): Int

    fun boltffi_torvox_bridge_set_theme(
        handle: Long,
        theme_ptr: ByteArray?,
        theme_len: Int,
    ): Int

    fun torvox_bridge_set_theme(
        handle: Long,
        theme_ptr: ByteArray?,
        theme_len: Int,
    ): Int

    fun boltffi_torvox_bridge_scrollback_line(
        handle: Long,
        index: Int,
    ): Long

    fun torvox_bridge_scrollback_line(
        handle: Long,
        index: Int,
    ): Long

    fun boltffi_torvox_bridge_get_terminal_text(handle: Long): Long

    fun torvox_bridge_get_terminal_text(handle: Long): Long

    fun boltffi_torvox_bridge_get_active_session_title(handle: Long): Long

    fun torvox_bridge_get_active_session_title(handle: Long): Long

    fun boltffi_torvox_bridge_get_grid_rows(handle: Long): Int

    fun torvox_bridge_get_grid_rows(handle: Long): Int

    fun boltffi_torvox_bridge_get_grid_cols(handle: Long): Int

    fun torvox_bridge_get_grid_cols(handle: Long): Int

    fun boltffi_torvox_bridge_get_cell_width(handle: Long): Float

    fun torvox_bridge_get_cell_width(handle: Long): Float

    fun boltffi_torvox_bridge_get_cell_height(handle: Long): Float

    fun torvox_bridge_get_cell_height(handle: Long): Float

    fun torvox_bridge_get_grid_rows_cols(handle: Long): Long

    fun boltffi_torvox_bridge_get_default_font_name(handle: Long): Long

    fun torvox_bridge_get_default_font_name(handle: Long): Long

    fun boltffi_torvox_bridge_get_font_info(handle: Long): Long

    fun torvox_bridge_get_font_info(handle: Long): Long

    fun boltffi_torvox_bridge_set_system_locale(
        handle: Long,
        locale: ByteArray,
    )

    fun torvox_bridge_set_system_locale(
        handle: Long,
        locale: ByteArray,
    )

    fun boltffi_torvox_bridge_list_font_families(handle: Long): Long

    fun torvox_bridge_list_font_families(handle: Long): Long

    fun boltffi_torvox_bridge_free_string(handle: Long)

    fun torvox_bridge_free_string(handle: Long)

    fun boltffi_torvox_bridge_free_notification(ptr: Long)

    fun torvox_bridge_free_notification(ptr: Long)

    /** Aggregates all per-frame poll results into a single surface-lock acquisition. */
    fun boltffi_torvox_bridge_poll_all(handle: Long): Long

    fun torvox_bridge_poll_all(handle: Long): Long

    fun boltffi_torvox_bridge_free_poll_all(ptr: Long)

    fun torvox_bridge_free_poll_all(ptr: Long)

    fun torvox_bridge_wait_output(
        handle: Long,
        timeoutMs: Long,
    ): Int

    fun boltffi_torvox_bridge_search_in_scrollback(
        handle: Long,
        query_ptr: ByteArray?,
        query_len: Int,
    ): Long

    fun torvox_bridge_search_in_scrollback(
        handle: Long,
        query_ptr: ByteArray?,
        query_len: Int,
    ): Long

    fun boltffi_torvox_bridge_search_all_in_scrollback(
        handle: Long,
        query_ptr: ByteArray?,
        query_len: Int,
        case_sensitive: Byte,
        fuzzy: Byte,
    ): Long

    fun torvox_bridge_search_all_in_scrollback(
        handle: Long,
        query_ptr: ByteArray?,
        query_len: Int,
        case_sensitive: Byte,
        fuzzy: Byte,
    ): Long

    fun boltffi_torvox_bridge_set_scroll_offset(
        handle: Long,
        offset: Int,
    )

    fun torvox_bridge_set_scroll_offset(
        handle: Long,
        offset: Int,
    )

    fun boltffi_torvox_bridge_wait_until_ready_for_render(handle: Long)

    fun torvox_bridge_wait_until_ready_for_render(handle: Long)

    fun boltffi_torvox_bridge_set_background_image(
        handle: Long,
        data: ByteArray?,
        len: Int,
        width: Int,
        height: Int,
    )

    fun torvox_bridge_set_background_image(
        handle: Long,
        data: ByteArray?,
        len: Int,
        width: Int,
        height: Int,
    )

    fun boltffi_torvox_bridge_set_background_params(
        handle: Long,
        blur_radius: Int,
        alpha_tenths: Int,
    )

    fun torvox_bridge_set_background_params(
        handle: Long,
        blur_radius: Int,
        alpha_tenths: Int,
    )

    fun boltffi_torvox_bridge_clear_background_image(handle: Long)

    fun torvox_bridge_clear_background_image(handle: Long)

    fun torvox_bridge_set_render_paused(
        handle: Long,
        paused: Int,
    )

    fun boltffi_torvox_bridge_set_cursor_blink_enabled(
        handle: Long,
        enabled: Int,
    )

    fun torvox_bridge_set_cursor_blink_enabled(
        handle: Long,
        enabled: Int,
    )

    fun boltffi_torvox_bridge_set_cursor_blink_speed_ms(
        handle: Long,
        speed_ms: Int,
    )

    fun torvox_bridge_set_cursor_blink_speed_ms(
        handle: Long,
        speed_ms: Int,
    )

    fun boltffi_torvox_bridge_reset_cursor_blink(handle: Long)

    fun torvox_bridge_reset_cursor_blink(handle: Long)

    fun boltffi_torvox_bridge_set_cursor_style(
        handle: Long,
        style_ptr: ByteArray?,
        style_len: Int,
    )

    fun torvox_bridge_set_cursor_style(
        handle: Long,
        style_ptr: ByteArray?,
        style_len: Int,
    )

    fun boltffi_last_error_message(): ByteArray?
}

@Volatile
private var nativeLib: TorvoxNative? = null
private val libLock = Any()

/** Force JNA into Android mode so it uses System.loadLibrary instead of dlopen. */
fun initNativeProxy() {
    if (nativeLib != null) return
    if (isRunningRobolectric()) {
        android.util.Log.i("TorvoxBridge", "Robolectric environment detected — skipping native library load")
        return
    }
    // Tell JNA we're on Android so it uses System.loadLibrary (which works with the
    // classloader namespace) instead of dlopen (which fails on Android 15+ linker
    // namespaces). Must be set before any Native.load() call.
    try {
        System.setProperty("jna.platform", "android")
    } catch (e: SecurityException) {
        android.util.Log.w("TorvoxBridge", "Failed to set jna.platform", e)
    }
    System.loadLibrary("torvox_android")
    nativeLib = Native.load("torvox_android", TorvoxNative::class.java)
}

/** Detect whether we are running in a Robolectric unit test environment. */
private fun isRunningRobolectric(): Boolean =
    try {
        Class.forName("org.robolectric.RobolectricTestRunner")
        true
    } catch (_: ClassNotFoundException) {
        false
    }

private fun ensureLib(): TorvoxNative {
    if (isRunningRobolectric()) {
        throw IllegalStateException(
            "TorvoxBridge native library is not available in Robolectric unit tests. " +
                "Do not call TorvoxBridge functions from tests that run on Robolectric.",
        )
    }
    nativeLib?.let { return it }
    synchronized(libLock) {
        nativeLib?.let { return it }
        initNativeProxy()
        val lib = nativeLib
        if (lib == null) {
            Log.w("TorvoxBridge", "initNativeProxy() did not set nativeLib")
        }
        return nativeLib ?: error("nativeLib not initialized after initNativeProxy")
    }
}

// ── FfiBuf reader ─────────────────────────────────────────────────────

private data class FfiBuf(
    val pointer: Long,
    val length: Int,
)

private fun readFfiBuf(pointer: Pointer?): FfiBuf {
    if (pointer == null) return FfiBuf(0, 0)
    return FfiBuf(pointer.getLong(0), pointer.getLong(8).toInt())
}

private fun readWireBytes(buffer: FfiBuf): ByteArray {
    if (buffer.pointer == 0L || buffer.length == 0) return ByteArray(0)
    return Pointer(buffer.pointer).getByteArray(0, buffer.length)
}

/**
 * JNA bridge to the native torvox library — wire encoding and session lifecycle.
 *
 * @see FR-049 Bridge: boltffi ↔ JNA wire format
 * @see FR-050 Bridge: rkyv serialization
 */
class TorvoxBridge(
    private val handle: Long,
) : AutoCloseable {
    private var closed = false

    private fun ensureOpen() {
        check(!closed) { "TorvoxBridge is closed" }
    }

    private fun <T> callOk(
        functionPointer: (TorvoxNative) -> Pointer?,
        decode: (WireReader) -> T,
    ): T {
        val library = ensureLib()
        val pointer = functionPointer(library) ?: throw RuntimeException("Native call returned null")
        val buffer = readFfiBuf(pointer)
        if (buffer.pointer == 0L) throw RuntimeException("Empty response from native call")
        val reader = WireReader(readWireBytes(buffer))
        val tag = reader.readByte()
        return if (tag == 0.toByte()) decode(reader) else throw RuntimeException(reader.readString())
    }

    fun ping(): String {
        val library = ensureLib()
        val result = library.torvox_bridge_ping(handle)
        return if (result == 0) "pong" else "error:$result"
    }

    fun spawnTerminal(
        rows: UInt,
        cols: UInt,
    ): Int {
        val library = ensureLib()
        return library.torvox_bridge_spawn_terminal(handle, rows.toInt(), cols.toInt())
    }

    fun setNativeWindow(
        windowPointer: Long,
        width: Int,
        height: Int,
    ) {
        val library = ensureLib()
        val result =
            library.torvox_bridge_set_native_window(
                handle,
                (windowPointer and 0xFFFFFFFFL).toInt(),
                ((windowPointer shr 32) and 0xFFFFFFFFL).toInt(),
                width,
                height,
            )
        if (result != 0) throw RuntimeException("setNativeWindow failed with code $result")
    }

    fun render(skipOutput: Boolean = false): Int {
        ensureOpen()
        return ensureLib().torvox_bridge_render(handle, if (skipOutput) 1 else 0)
    }

    fun getSnapshot(
        scrollOffset: Int,
        buf: ByteArray,
        bufLen: Int,
    ): Int {
        ensureOpen()
        return ensureLib().torvox_bridge_get_snapshot(handle, scrollOffset, buf, bufLen)
    }

    fun saveTestFrame(dataDir: String): Int {
        com.sun.jna.Native.toByteArray(dataDir).let { dataBytes ->
            com.sun.jna.Memory(dataBytes.size.toLong() + 1).use { mem ->
                mem.write(0, dataBytes, 0, dataBytes.size)
                mem.setByte(dataBytes.size.toLong(), 0)
                return ensureLib().torvox_bridge_save_test_frame(handle, mem)
            }
        }
    }

    fun saveTestFrameWithSelection(
        dataDir: String,
        startRow: Int,
        startCol: Int,
        endRow: Int,
        endCol: Int,
        active: Boolean,
        mode: Int = 0,
    ): Int {
        com.sun.jna.Native.toByteArray(dataDir).let { dataBytes ->
            com.sun.jna.Memory(dataBytes.size.toLong() + 1).use { mem ->
                mem.write(0, dataBytes, 0, dataBytes.size)
                mem.setByte(dataBytes.size.toLong(), 0)
                return ensureLib().torvox_bridge_save_test_frame_with_selection(
                    handle,
                    mem,
                    startRow,
                    startCol,
                    endRow,
                    endCol,
                    if (active) 1 else 0,
                    mode,
                )
            }
        }
    }

    fun pollBel(): Boolean = ensureLib().torvox_bridge_poll_bel(handle) != 0

    fun pollShellIntegration(): Int = ensureLib().torvox_bridge_poll_shell_integration(handle)

    fun pollSyncActive(): Boolean = ensureLib().torvox_bridge_poll_sync_active(handle) != 0

    fun pollClipboard(): String? {
        val pointer = ensureLib().torvox_bridge_poll_clipboard(handle)
        if (pointer == 0L) return null
        val text = Pointer(pointer).getString(0, "UTF-8")
        ensureLib().torvox_bridge_free_string(pointer)
        return text
    }

    fun pollNotification(): Pair<String, String>? {
        val pointer = ensureLib().torvox_bridge_poll_notification(handle)
        if (pointer == 0L) return null
        // pointer points to [title_ptr, body_ptr] — two consecutive C string pointers
        val buffer = Pointer(pointer)
        val titlePtr = buffer.getPointer(0)
        val bodyPtr = buffer.getPointer(8)
        val title = titlePtr?.getString(0, "UTF-8") ?: ""
        val body = bodyPtr?.getString(0, "UTF-8") ?: ""
        ensureLib().torvox_bridge_free_notification(pointer)
        return if (title.isNotEmpty() || body.isNotEmpty()) title to body else null
    }

    /** Result of a single-lock [pollAll] call. */
    data class PollAllResult(
        val bel: Boolean,
        val clipboard: String?,
        val notification: Pair<String, String>?,
        val syncActive: Boolean,
        val shellIntegration: Int,
    )

    /**
     * Polls all deferred events (BEL, clipboard, notification, sync mode, shell
     * integration) in a single JNI call + surface-lock acquisition. Replaces the
     * 5 separate `poll_*` JNA calls that each acquired the surface mutex on their
     * own, eliminating ~4 extra round-trips and lock acquisitions per render frame.
     */
    fun pollAll(): PollAllResult {
        val pointer = ensureLib().torvox_bridge_poll_all(handle)
        if (pointer == 0L) {
            return PollAllResult(false, null, null, false, 0)
        }
        val buffer = Pointer(pointer)
        // Layout (see PollAllFFI in bridge.rs): bel:u8, sync_active:u8,
        // shell_integration:u8, _pad:u8, clipboard_ptr:i64, notification_ptr:i64
        val bel = buffer.getByte(0) != 0.toByte()
        val syncActive = buffer.getByte(1) != 0.toByte()
        val shellIntegration = buffer.getByte(2).toInt() and 0xFF
        val clipboardPtr = buffer.getLong(8)
        val notificationPtr = buffer.getLong(16)
        val clipboard =
            if (clipboardPtr != 0L) {
                val text = Pointer(clipboardPtr).getString(0, "UTF-8")
                ensureLib().torvox_bridge_free_string(clipboardPtr)
                text
            } else {
                null
            }
        val notification =
            if (notificationPtr != 0L) {
                val nbuf = Pointer(notificationPtr)
                val titlePtr = nbuf.getPointer(0)
                val bodyPtr = nbuf.getPointer(8)
                val title = titlePtr?.getString(0, "UTF-8") ?: ""
                val body = bodyPtr?.getString(0, "UTF-8") ?: ""
                ensureLib().torvox_bridge_free_notification(notificationPtr)
                if (title.isNotEmpty() || body.isNotEmpty()) title to body else null
            } else {
                null
            }
        ensureLib().torvox_bridge_free_poll_all(pointer)
        return PollAllResult(bel, clipboard, notification, syncActive, shellIntegration)
    }

    fun cwd(): String {
        val pointer = ensureLib().torvox_bridge_cwd(handle)
        if (pointer == null) return ""
        val str = pointer.getString(0, "UTF-8")
        ensureLib().torvox_bridge_free_cstring(pointer)
        return str
    }

    /**
     * Wait for PTY output or timeout via the Rust Condvar.
     * Returns true if output arrived, false on timeout.
     */
    fun waitOutput(timeoutMs: Long): Boolean = ensureLib().torvox_bridge_wait_output(handle, timeoutMs) == 1

    fun focusEvent(focused: Boolean) {
        ensureLib().torvox_bridge_focus_event(handle, if (focused) 1 else 0)
    }

    fun resize(
        rows: UInt,
        cols: UInt,
    ) {
        val library = ensureLib()
        val result = library.torvox_bridge_resize(handle, rows.toInt(), cols.toInt())
        if (result != 0) throw RuntimeException("resize failed with code $result")
    }

    fun recomputeGrid(
        width: UInt,
        height: UInt,
    ) {
        val library = ensureLib()
        val result = library.torvox_bridge_recompute_grid(handle, width.toInt(), height.toInt())
        if (result != 0) throw RuntimeException("recomputeGrid failed with code $result")
    }

    fun setSurfaceSize(
        width: Int,
        height: Int,
    ) {
        ensureLib().torvox_bridge_set_surface_size(handle, width, height)
    }

    fun updateNativeWindow(
        windowPointer: Long,
        width: Int,
        height: Int,
    ) {
        val library = ensureLib()
        val result =
            library.torvox_bridge_update_native_window(
                handle,
                (windowPointer and 0xFFFFFFFFL).toInt(),
                ((windowPointer shr 32) and 0xFFFFFFFFL).toInt(),
                width,
                height,
            )
        if (result != 0) throw RuntimeException("updateNativeWindow failed with code $result")
    }

    fun releaseSurface() {
        val library = ensureLib()
        library.torvox_bridge_release_surface(handle)
    }

    fun releaseGpuSurface() {
        val library = ensureLib()
        library.torvox_bridge_release_gpu_surface(handle)
    }

    fun setScrollOffset(offset: UInt) {
        ensureLib().torvox_bridge_set_scroll_offset(handle, offset.toInt())
    }

    fun waitUntilReadyForRender() {
        ensureLib().torvox_bridge_wait_until_ready_for_render(handle)
    }

    fun setSelection(
        startRow: UInt,
        startCol: UInt,
        endRow: UInt,
        endCol: UInt,
        active: Boolean,
        mode: Byte = 0,
    ) {
        val library = ensureLib()
        library.torvox_bridge_set_selection(
            handle,
            startRow.toInt(),
            startCol.toInt(),
            endRow.toInt(),
            endCol.toInt(),
            if (active) 1 else 0,
            mode.toInt(),
        )
    }

    fun expandAndSetSelection(
        row: UInt,
        col: UInt,
        mode: Byte = 0,
    ): Pair<Pair<UInt, UInt>, Pair<UInt, UInt>>? {
        if (row > LOW_16_MASK.toUInt() || col > LOW_16_MASK.toUInt()) {
            throw IllegalArgumentException(
                "expandAndSetSelection: row/col exceed the 16-bit wire packing range " +
                    "(row=$row, col=$col, max=${LOW_16_MASK})",
            )
        }
        val result =
            ensureLib().torvox_bridge_expand_and_set_selection(
                handle,
                row.toInt(),
                col.toInt(),
                mode.toInt(),
            )
        if (result < 0) return null
        val startRow = (result and LOW_16_MASK).toUInt()
        val startCol = ((result shr 16) and LOW_16_MASK).toUInt()
        val endRow = ((result shr 32) and LOW_16_MASK).toUInt()
        val endCol = ((result shr 48) and LOW_16_MASK).toUInt()
        return Pair(Pair(startRow, startCol), Pair(endRow, endCol))
    }

    fun isCellEmpty(
        row: UInt,
        col: UInt,
    ): Boolean = ensureLib().torvox_bridge_is_cell_empty(handle, row.toInt(), col.toInt()) != 0

    fun hasTextInRange(
        startRow: UInt,
        startCol: UInt,
        endRow: UInt,
        endCol: UInt,
    ): Boolean =
        ensureLib().torvox_bridge_has_text_in_range(handle, startRow.toInt(), startCol.toInt(), endRow.toInt(), endCol.toInt()) != 0

    /**
     * Move one endpoint of the active selection (set [handleSide]=0 to move the start, 1 to move
     * the end) to the cell ([anchorRow], [anchorCol]) while keeping the opposite endpoint fixed.
     * For Word mode the moved endpoint is re-expanded with the core [Selection] logic so drag-grow
     * stays consistent with long-press. Returns the resulting ordered selection bounds, or null on
     * failure.
     */
    fun setSelectionEndpoint(
        handleSide: Byte,
        anchorRow: UInt,
        anchorCol: UInt,
        otherRow: UInt,
        otherCol: UInt,
        mode: Byte,
        originRow: UInt,
        originCol: UInt,
    ): Pair<Pair<UInt, UInt>, Pair<UInt, UInt>>? {
        if (anchorRow > LOW_16_MASK.toUInt() ||
            anchorCol > LOW_16_MASK.toUInt() ||
            otherRow > LOW_16_MASK.toUInt() ||
            otherCol > LOW_16_MASK.toUInt() ||
            originRow > LOW_16_MASK.toUInt() ||
            originCol > LOW_16_MASK.toUInt()
        ) {
            throw IllegalArgumentException(
                "setSelectionEndpoint: row/col exceed the 16-bit wire packing range " +
                    "(anchorRow=$anchorRow, anchorCol=$anchorCol, otherRow=$otherRow, " +
                    "otherCol=$otherCol, originRow=$originRow, originCol=$originCol, max=$LOW_16_MASK)",
            )
        }
        val result =
            ensureLib().torvox_bridge_set_selection_endpoint(
                handle,
                handleSide,
                anchorRow.toInt(),
                anchorCol.toInt(),
                otherRow.toInt(),
                otherCol.toInt(),
                mode.toInt(),
                originRow.toInt(),
                originCol.toInt(),
            )
        if (result < 0) return null
        val startRow = (result and LOW_16_MASK).toUInt()
        val startCol = ((result shr 16) and LOW_16_MASK).toUInt()
        val endRow = ((result shr 32) and LOW_16_MASK).toUInt()
        val endCol = ((result shr 48) and LOW_16_MASK).toUInt()
        return Pair(Pair(startRow, startCol), Pair(endRow, endCol))
    }

    fun setSearchHighlights(serialized: ByteArray) {
        ensureLib().torvox_bridge_set_search_highlights(handle, serialized, serialized.size)
    }

    fun clearSearchHighlights() {
        ensureLib().torvox_bridge_set_search_highlights(handle, null, 0)
    }

    fun scrollbackLength(): UInt = ensureLib().torvox_bridge_scrollback_len(handle).toUInt()

    fun writeToPty(data: ByteArray): Boolean {
        val result = ensureLib().torvox_bridge_write_to_pty(handle, data, data.size)
        if (result == 0) {
            ensureLib().torvox_bridge_render(handle, 0)
        }
        return result == 0
    }

    fun processKeyEvent(
        keyCode: Int,
        modifiers: Byte,
        action: Byte,
        unicodeChar: Int,
        unshiftedChar: Int,
    ): Boolean {
        ensureOpen()
        val result =
            ensureLib().torvox_bridge_process_key_event(
                handle,
                keyCode,
                modifiers,
                action,
                unicodeChar,
                unshiftedChar,
            )
        return result == 0
    }

    fun setFontSize(sizeTenths: UInt) {
        ensureLib().torvox_bridge_set_font_size(handle, sizeTenths.toInt())
    }

    fun setFontSizeInPlace(sizeTenths: UInt) {
        val result = ensureLib().torvox_bridge_set_font_size_in_place(handle, sizeTenths.toInt())
        if (result != 0) android.util.Log.w("TorvoxBridge", "setFontSizeInPlace failed with code $result")
    }

    fun setExtraFontPaths(paths: List<String>) {
        if (paths.isEmpty()) return
        val pathBytes = paths.map { it.toByteArray(Charsets.UTF_8) }
        val count = pathBytes.size
        val pathPtrsMem = com.sun.jna.Memory(JNA_POINTER_SIZE * count)
        val lensMem = com.sun.jna.Memory(JNA_INT_SIZE * count)
        val buffers = mutableListOf<com.sun.jna.Memory>()
        for (i in 0 until count) {
            val bytes = pathBytes[i]
            val bytesMem = com.sun.jna.Memory(bytes.size.toLong())
            bytesMem.write(0, bytes, 0, bytes.size)
            pathPtrsMem.setPointer(i * JNA_POINTER_SIZE, bytesMem)
            lensMem.setInt(i * JNA_INT_SIZE, bytes.size)
            buffers.add(bytesMem)
        }
        val result = ensureLib().torvox_bridge_set_extra_font_paths(handle, pathPtrsMem, lensMem, count)
        if (result != 0) android.util.Log.w("TorvoxBridge", "setExtraFontPaths failed with code $result")
    }

    fun setFontFamily(familyName: String) {
        val bytes = familyName.toByteArray(Charsets.UTF_8)
        val result = ensureLib().torvox_bridge_set_font_family(handle, bytes, bytes.size)
        if (result != 0) throw RuntimeException("setFontFamily failed with code $result")
    }

    fun setTheme(theme: BridgeTheme) {
        val bytes = theme.wireEncodeBytes()
        ensureLib().torvox_bridge_set_theme(handle, bytes, bytes.size)
    }

    fun setSavePath(path: String) {
        val bytes = path.toByteArray(Charsets.UTF_8)
        ensureLib().torvox_bridge_set_save_path(handle, bytes, bytes.size)
    }

    fun getConfig(): TerminalConfig = callOk({ it.boltffi_torvox_bridge_get_config(handle) }) { TerminalConfig.wireDecode(it) }

    fun getThemeNames(): List<String> {
        val pointer = ensureLib().boltffi_torvox_bridge_get_theme_names(handle) ?: return emptyList()
        val result = WireReader(readWireBytes(readFfiBuf(pointer))).readString()
        return result.split("\u001F").filter { it.isNotEmpty() }
    }

    fun listFonts(): List<String> {
        val pointer = ensureLib().boltffi_torvox_bridge_list_fonts(handle) ?: return emptyList()
        val result = WireReader(readWireBytes(readFfiBuf(pointer))).readString()
        return result.split("\u001F").filter { it.isNotEmpty() }
    }

    fun getDefaultFontName(): String {
        val ptr = ensureLib().torvox_bridge_get_default_font_name(handle)
        if (ptr == 0L) return "monospace"
        val result = Pointer(ptr).getString(0)
        ensureLib().torvox_bridge_free_string(ptr)
        return result.ifEmpty { "monospace" }
    }

    fun getFontInfo(): String {
        val ptr = ensureLib().torvox_bridge_get_font_info(handle)
        if (ptr == 0L) return "No font loaded"
        val result = Pointer(ptr).getString(0)
        ensureLib().torvox_bridge_free_string(ptr)
        return result.ifEmpty { "No font loaded" }
    }

    fun setSystemLocale(locale: String) {
        val bytes = locale.toByteArray(Charsets.UTF_8) + 0.toByte()
        ensureLib().torvox_bridge_set_system_locale(handle, bytes)
    }

    fun loadFontFile(path: String): String? {
        val bytes = path.toByteArray(Charsets.UTF_8)
        val ptr = ensureLib().torvox_bridge_load_font_file(handle, bytes, bytes.size)
        if (ptr == 0L) return null
        val result = Pointer(ptr).getString(0, "UTF-8")
        ensureLib().torvox_bridge_free_string(ptr)
        return result
    }

    fun saveSession(path: String) {
        val bytes = path.toByteArray(Charsets.UTF_8)
        val result = ensureLib().torvox_bridge_save_session(handle, bytes, bytes.size)
        if (result != 0) throw RuntimeException("saveSession failed with code $result")
    }

    fun restoreSession(path: String) {
        val bytes = path.toByteArray(Charsets.UTF_8)
        val result = ensureLib().torvox_bridge_restore_session(handle, bytes, bytes.size)
        if (result != 0) throw RuntimeException("restoreSession failed with code $result")
    }

    fun hasSavedSession(path: String): Boolean {
        val bytes = path.toByteArray(Charsets.UTF_8)
        return ensureLib().torvox_bridge_has_saved_session(handle, bytes, bytes.size)
    }

    fun scrollbackLine(index: UInt): String? {
        val pointer = ensureLib().torvox_bridge_scrollback_line(handle, index.toInt())
        if (pointer == 0L) return null
        val text = Pointer(pointer).getString(0, "UTF-8")
        ensureLib().torvox_bridge_free_string(pointer)
        return text
    }

    fun getTerminalText(): String? {
        val pointer = ensureLib().torvox_bridge_get_terminal_text(handle)
        if (pointer == 0L) return null
        val text = Pointer(pointer).getString(0, "UTF-8")
        ensureLib().torvox_bridge_free_string(pointer)
        return text
    }

    fun getActiveSessionTitle(): String {
        val pointer = ensureLib().torvox_bridge_get_active_session_title(handle)
        if (pointer == 0L) return ""
        val text = Pointer(pointer).getString(0, "UTF-8")
        ensureLib().torvox_bridge_free_string(pointer)
        return text
    }

    fun getGridRows(): Int = ensureLib().torvox_bridge_get_grid_rows(handle)

    fun getGridCols(): Int = ensureLib().torvox_bridge_get_grid_cols(handle)

    fun getCellWidth(): Float = ensureLib().torvox_bridge_get_cell_width(handle)

    fun getCellHeight(): Float = ensureLib().torvox_bridge_get_cell_height(handle)

    fun getGridRowsColsPacked(): Long = ensureLib().torvox_bridge_get_grid_rows_cols(handle)

    fun listFontFamilies(): List<String> {
        val pointer = ensureLib().torvox_bridge_list_font_families(handle)
        if (pointer == 0L) return emptyList()
        val text = Pointer(pointer).getString(0, "UTF-8")
        ensureLib().torvox_bridge_free_string(pointer)
        return if (text.isNotEmpty()) text.split("\u001f") else emptyList()
    }

    fun setBackgroundImage(
        rgbaData: ByteArray,
        width: UInt,
        height: UInt,
    ) {
        ensureLib().torvox_bridge_set_background_image(handle, rgbaData, rgbaData.size, width.toInt(), height.toInt())
    }

    fun setBackgroundParams(
        blurRadius: UInt,
        alphaTenths: UInt,
    ) {
        ensureLib().torvox_bridge_set_background_params(handle, blurRadius.toInt(), alphaTenths.toInt())
    }

    fun clearBackgroundImage() {
        ensureLib().torvox_bridge_clear_background_image(handle)
    }

    fun setRenderPaused(paused: Boolean) = ensureLib().torvox_bridge_set_render_paused(handle, if (paused) 1 else 0)

    fun setCursorBlinkEnabled(enabled: Boolean) {
        ensureLib().torvox_bridge_set_cursor_blink_enabled(handle, if (enabled) 1 else 0)
    }

    fun setCursorBlinkSpeedMs(speedMs: Int) {
        ensureLib().torvox_bridge_set_cursor_blink_speed_ms(handle, speedMs)
    }

    fun resetCursorBlink() {
        ensureLib().torvox_bridge_reset_cursor_blink(handle)
    }

    fun setCursorStyle(style: String) {
        val bytes = style.toByteArray(Charsets.UTF_8)
        ensureLib().torvox_bridge_set_cursor_style(handle, bytes, bytes.size)
    }

    fun searchInScrollback(query: String): Pair<Int, Int>? {
        val queryBytes = query.toByteArray(Charsets.UTF_8)
        val pointer = ensureLib().torvox_bridge_search_in_scrollback(handle, queryBytes, queryBytes.size)
        if (pointer == 0L) return null
        val text = Pointer(pointer).getString(0, "UTF-8")
        ensureLib().torvox_bridge_free_string(pointer)
        val parts = text.split(',')
        return if (parts.size == 2) parts[0].toInt() to parts[1].toInt() else null
    }

    fun searchAllInScrollback(
        query: String,
        caseSensitive: Boolean,
        fuzzy: Boolean = false,
    ): List<Triple<Int, Int, Int>>? {
        val queryBytes = query.toByteArray(Charsets.UTF_8)
        val ptr =
            ensureLib().torvox_bridge_search_all_in_scrollback(
                handle,
                queryBytes,
                queryBytes.size,
                if (caseSensitive) 1 else 0,
                if (fuzzy) 1 else 0,
            )
        if (ptr == 0L) return null
        val text = Pointer(ptr).getString(0, "UTF-8")
        ensureLib().torvox_bridge_free_string(ptr)
        if (text.isEmpty()) return emptyList()
        return text.split(";").map { part ->
            val coords = part.split(",")
            Triple(coords[0].toInt(), coords[1].toInt(), coords[2].toInt())
        }
    }

    override fun close() {
        if (!closed) {
            closed = true
            nativeLib?.boltffi_torvox_bridge_free(handle)
        }
    }

    fun isClosed(): Boolean = closed

    companion object {
        /** Initialise the Rust logging (idempotent). */
        fun initLogger() {
            if (isRunningRobolectric()) return
            ensureLib().torvox_init_logger()
        }

        /** Set the Rust log file path (called from TorvoxApp.onCreate). */
        fun setLogFilePath(path: String) {
            if (isRunningRobolectric()) return
            val bytes = path.toByteArray(Charsets.UTF_8)
            ensureLib().torvox_set_log_file_path(bytes, bytes.size)
        }
    }
}

/**
 * Create a new TorvoxBridge instance with the given config.
 *
 * @see FR-049 Bridge: boltffi ↔ JNA wire format
 */
fun createBridge(config: TerminalConfig): TorvoxBridge {
    val library = ensureLib()
    val wireBytes = config.wireEncode()
    val handle = library.boltffi_torvox_bridge_new(wireBytes, wireBytes.size)
    if (handle == 0L) {
        val errMsg = library.boltffi_last_error_message()?.toString(Charsets.UTF_8) ?: "unknown"
        throw RuntimeException("Failed to create TorvoxBridge: $errMsg")
    }
    return TorvoxBridge(handle)
}
