package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Composing-region reconciliation (warp WarpInputView.kt:587-615
 * forwardComposingDiff) — pure function tests: grow / contract
 * (emoji-aware code-point counting) / diverged / no-op.
 */
@RunWith(RobolectricTestRunner::class)
class ComposingDiffTest {
    @Test
    fun grow_appends_only_the_suffix() {
        val edit = ComposingDiff.reconcile("a", "ab")
        assertEquals(0, edit.backspaces)
        assertEquals("b", edit.append)
    }

    @Test
    fun identical_composition_is_noop() {
        val edit = ComposingDiff.reconcile("abc", "abc")
        assertEquals(0, edit.backspaces)
        assertEquals("", edit.append)
        assertTrue("identical must be empty edit", edit.isEmpty)
    }

    @Test
    fun contract_backspaces_removed_code_points() {
        // 'a🎉b' has 3 code points; removing the suffix 'b' is 1 backspace.
        val edit = ComposingDiff.reconcile("a🎉b", "a")
        assertEquals(2, edit.backspaces)
        assertEquals("", edit.append)
    }

    @Test
    fun contract_emoji_is_one_backspace_not_two() {
        // UTF-16 length of 🎉 is 2; code-point count is 1. Removing just the
        // emoji from 'a🎉' must be ONE backspace.
        val edit = ComposingDiff.reconcile("a🎉", "a")
        assertEquals(1, edit.backspaces)
        assertEquals("", edit.append)
    }

    @Test
    fun diverged_backspaces_all_then_appends() {
        val edit = ComposingDiff.reconcile("abc", "xy")
        assertEquals(3, edit.backspaces)
        assertEquals("xy", edit.append)
    }

    @Test
    fun grow_after_diverged_prefix() {
        // Common prefix 'a', then diverged suffix.
        val edit = ComposingDiff.reconcile("ab", "acd")
        assertEquals(1, edit.backspaces)
        assertEquals("cd", edit.append)
    }

    @Test
    fun contract_to_empty_is_all_backspaces() {
        val edit = ComposingDiff.reconcile("世界", "")
        assertEquals(2, edit.backspaces)
        assertEquals("", edit.append)
    }

    @Test
    fun empty_to_nonempty_is_pure_append() {
        val edit = ComposingDiff.reconcile("", "héllo")
        assertEquals(0, edit.backspaces)
        assertEquals("héllo", edit.append)
    }

    @Test
    fun same_high_surrogate_emoji_replaces_whole_code_point() {
        // 👍 (U+1F44D) 与 👎 (U+1F44E) 共享高代理位，只差低代理位。
        // 按 UTF-16 下标求公共前缀会切在代理对中间，待追加段变成半个码元，
        // 编不出合法 UTF-8（被替换成 U+FFFD）。
        val edit = ComposingDiff.reconcile("👍", "👎")
        assertEquals(1, edit.backspaces)
        assertEquals("👎", edit.append)
    }

    @Test
    fun same_high_surrogate_emoji_after_prefix_replaces_whole_code_point() {
        val edit = ComposingDiff.reconcile("a👍", "a👎")
        assertEquals(1, edit.backspaces)
        assertEquals("👎", edit.append)
    }

    @Test
    fun shared_emoji_prefix_is_kept_whole() {
        // 公共前缀本身含代理对时，切分点仍须落在码点边界上。
        val edit = ComposingDiff.reconcile("👍a", "👍b")
        assertEquals(1, edit.backspaces)
        assertEquals("b", edit.append)
    }
}
