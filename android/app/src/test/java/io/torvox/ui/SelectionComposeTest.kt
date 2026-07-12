package io.torvox.ui

import androidx.compose.ui.test.junit4.AndroidComposeTestRule
import io.torvox.RobolectricActivityRule
import io.torvox.TestActivity
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], application = android.app.Application::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class SelectionComposeTest {
    @Suppress("DEPRECATION")
    @get:Rule
    val composeTestRule: AndroidComposeTestRule<RobolectricActivityRule<TestActivity>, TestActivity> =
        AndroidComposeTestRule(
            RobolectricActivityRule(TestActivity::class.java),
            activityProvider = { it.activity },
        )

    @Test
    fun expandWordSelection_findsWordBoundaries() {
        val (start, end) = expandWordOnLine("hello world", 0)
        assertEquals(0, start)
        assertEquals(5, end)
    }

    @Test
    fun expandWordSelection_emptyString() {
        val result = expandWordOnLine("", 0)
        assertEquals(0, result.first.toLong())
        assertEquals(0, result.second.toLong())
    }

    @Test
    fun expandWordSelection_atWhitespace() {
        val line = "hello world"
        val col = 5
        val result = expandWordOnLine(line, col)
        // At the space character, should expand to nearest word char
        assertTrue("startCol ($result.first) < endCol ($result.second)", result.first < result.second)
    }

    @Test
    fun expandWordSelection_underscoreIsWordChar() {
        assertTrue("underscore is word char", isWordChar('_'))
    }

    @Test
    fun expandWordSelection_digitIsWordChar() {
        assertTrue("digit is word char", isWordChar('5'))
    }

    @Test
    fun expandWordSelection_dotHyphenSlashIsWordChar() {
        assertTrue("period is word char (URL)", isWordChar('.'))
        assertTrue("hyphen is word char (path)", isWordChar('-'))
        assertTrue("slash is word char (URL)", isWordChar('/'))
    }

    @Test
    fun expandWordSelection_punctuationIsNotWordChar() {
        assertFalse("comma is not word char", isWordChar(','))
        assertFalse("paren is not word char", isWordChar(')'))
        assertFalse("space is not word char", isWordChar(' '))
    }

    @Test
    fun expandWordSelection_middleOfWord() {
        val (start, end) = expandWordOnLine("testing select", 8)
        assertEquals(8, start)
        assertEquals(14, end)
    }

    @Test
    fun expandWordSelection_singleWord() {
        val (start, end) = expandWordOnLine("torvox", 2)
        assertEquals(0, start)
        assertEquals(6, end)
    }

    @Test
    fun expandWordSelection_outOfBoundsCol() {
        val (start, end) = expandWordOnLine("hello", 20)
        assertEquals(20, start)
        assertEquals(20, end)
    }

    @Test
    fun isWordChar_letter_isTrue() {
        assertTrue("'a' is a word char", isWordChar('a'))
        assertTrue("'Z' is a word char", isWordChar('Z'))
    }

    @Test
    fun isWordChar_digit_isTrue() {
        assertTrue("'0' is a word char", isWordChar('0'))
        assertTrue("'9' is a word char", isWordChar('9'))
    }

    @Test
    fun isWordChar_underscore_isTrue() {
        assertTrue("'_' is a word char", isWordChar('_'))
    }

    @Test
    fun isWordChar_space_isFalse() {
        assertFalse("space is not a word char", isWordChar(' '))
    }

    @Test
    fun isWordChar_punctuation_isFalse() {
        assertFalse("',' is not a word char", isWordChar(','))
        assertFalse("'!' is not a word char", isWordChar('!'))
        assertFalse("'(' is not a word char", isWordChar('('))
    }
}
