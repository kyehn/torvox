package terminal.emulator.runtime

import android.annotation.SuppressLint
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * ClipboardAccess write/read semantics: a plain-text write round-trips
 * through `clipboardText()`, the clip label survives, and an empty
 * clipboard reports `Ok(null)` — 空剪贴板是合法状态，必须与读取失败
 * （`Result.failure`）可区分，否则 OSC 52 应答会把失败说成空剪贴板。
 */
@RunWith(RobolectricTestRunner::class)
class ClipboardAccessTest {

    private lateinit var access: ClipboardAccess

    @Before
    fun setUp() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        access = ClipboardAccess(context, "Test")
    }

    @Test
    fun `plain write round trip preserves content`() {
        access.setClipboardText("osc-52-content")
        assertEquals(
            "osc-52-content",
            access.clipboardText().getOrNull(),
        )
    }

    @Test
    @SuppressLint("DeprecatedCall") // primaryClip/getPrimaryClip: no @Deprecated in API 37; slack-lint rule data lag
    fun `clipboard label is preserved`() {
        access.setClipboardText("content", label = "custom label")
        val manager = ApplicationProvider.getApplicationContext<Context>()
            .getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
        assertEquals("custom label", manager.primaryClip?.description?.label)
        assertEquals("content", manager.primaryClip?.getItemAt(0)?.text?.toString())
    }

    @Test
    fun `empty clipboard reads as empty not failure`() {
        val result = access.clipboardText()
        assertTrue("空剪贴板不是读取失败", result.isSuccess)
        assertNull(result.getOrNull())
    }

    @Test
    fun `hasClipboardText is false on empty clipboard`() {
        // Pre-condition: a prior test may have left clip text; clear it.
        org.junit.Assert.assertFalse(
            "empty/fresh clipboard must report no text",
            access.hasClipboardText(),
        )
    }

    @Test
    fun `hasClipboardText is true after a write`() {
        access.setClipboardText("paste-me")
        org.junit.Assert.assertTrue(
            "clipboard with text must report hasClipboardText",
            access.hasClipboardText(),
        )
    }

    /**
     * 非文本剪贴（复制了一个链接 / 一张图片）在平台上 `hasPrimaryClip()` 为真，
     * 但没有任何可粘贴文本。粘贴菜单若据此提供 PASTE，点下去只会得到「剪贴板无文本」
     * ——正是该接口声称要消除的反馈，故此处钉住 hasClipboardText 只为文本项返回真。
     */
    @Test
    @SuppressLint("DeprecatedCall") // 剪贴板读写无替代面：测试必须构造真实非文本 clip
    fun `hasClipboardText is false for a non-text clip`() {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val manager = context.getSystemService(Context.CLIPBOARD_SERVICE)
            as android.content.ClipboardManager
        val uri = android.net.Uri.parse("content://terminal.emulator.test/link")
        manager.setPrimaryClip(
            android.content.ClipData.newRawUri("copied link", uri),
        )
        org.junit.Assert.assertFalse(
            "clip with no text must not report hasClipboardText",
            access.hasClipboardText(),
        )
    }
}
