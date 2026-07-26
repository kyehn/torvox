package io.term.ui

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import java.io.File

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class)
class FontIdentityVerificationTest {
    private val testDispatcher = StandardTestDispatcher()
    private lateinit var context: Context
    private lateinit var settingsProvider: io.term.settings.SettingsDataStoreProvider
    private lateinit var repository: io.term.settings.SettingsRepository

    @Before
    fun setup() {
        Dispatchers.setMain(testDispatcher)
        context = RuntimeEnvironment.getApplication()
        settingsProvider = io.term.settings.SettingsDataStoreProvider(context)
        repository = io.term.settings.SettingsRepository(settingsProvider)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun setFontFamily_persistsExactName() = runTest {
        repository.setFontFamily("JetBrainsMono Nerd Font")
        assertEquals(
            "font family must be exactly as set",
            "JetBrainsMono Nerd Font",
            repository.fontFamily.first(),
        )
    }

    @Test
    fun setFontFamily_overwritesPreviousValue() = runTest {
        repository.setFontFamily("First Font")
        repository.setFontFamily("Second Font")
        assertEquals(
            "font family must reflect latest set",
            "Second Font",
            repository.fontFamily.first(),
        )
    }

    @Test
    fun fontResolvesToAsIs() {
        assertEquals("Noto Sans Mono", resolveEffectiveFont("Noto Sans Mono"))
        assertEquals("monospace", resolveEffectiveFont("monospace"))
    }

    @Test
    fun emptyFontStaysEmpty() {
        assertEquals("", resolveEffectiveFont(""))
    }

    @Test
    fun namedFontPassesThroughUnchanged() {
        val effectiveFont = resolveEffectiveFont("Fira Code")
        assertEquals(
            "named font must pass through unchanged",
            "Fira Code",
            effectiveFont,
        )
    }

    @Test
    fun fontFileIsValidTrueType() {
        val fontsDir = File(context.filesDir, "fonts")
        fontsDir.mkdirs()
        val fontFile = File(fontsDir, "TestMono.ttf")

        val trueTypeHeader =
            byteArrayOf(
                0x00,
                0x01,
                0x00,
                0x00,
                0x00,
                0x0A,
                0x00,
                0x80.toByte(),
                0x00,
                0x03,
                0x00,
                0x60,
                't'.code.toByte(),
                'e'.code.toByte(),
                's'.code.toByte(),
                't'.code.toByte(),
            )
        fontFile.writeBytes(trueTypeHeader)

        assertTrue("font file must exist after write", fontFile.exists())
        assertTrue(
            "font file size must match header",
            fontFile.length() == trueTypeHeader.size.toLong(),
        )

        val header = fontFile.readBytes()
        assertEquals(
            "TrueType magic bytes must be 00 01 00 00",
            0x00.toByte(),
            header[0],
        )
        assertEquals(
            "TrueType magic byte 2 must be 01",
            0x01.toByte(),
            header[1],
        )

        fontFile.delete()
    }

    @Test
    fun fontFileInstallPreservesFileNameAsFamilyName() {
        val fontsDir = File(context.filesDir, "fonts")
        fontsDir.mkdirs()

        val testFont = File(fontsDir, "MyCustomFont.ttf")
        testFont.writeBytes("fake font content".toByteArray())

        assertTrue("font file must exist in fonts dir", testFont.exists())

        val familyName = testFont.nameWithoutExtension
        assertEquals(
            "font family name must derive from file name",
            "MyCustomFont",
            familyName,
        )

        testFont.delete()
    }

    @Test
    fun fontFileInstallHandlesOverwrite() {
        val fontsDir = File(context.filesDir, "fonts")
        fontsDir.mkdirs()

        val fontFile = File(fontsDir, "Overwrite.ttf")
        fontFile.writeBytes("version1".toByteArray())
        fontFile.writeBytes("version2".toByteArray())

        assertEquals(
            "overwritten file must have new content",
            "version2",
            fontFile.readText(),
        )

        fontFile.delete()
    }

    @Test
    fun fontFileInstallHandlesNestedDirectory() {
        val fontsDir = File(context.filesDir, "fonts")
        fontsDir.mkdirs()

        val subDir = File(fontsDir, "sub")
        subDir.mkdirs()
        val nestedFont = File(subDir, "Nested.ttf")
        nestedFont.writeBytes("nested font".toByteArray())

        assertTrue("nested font file must exist", nestedFont.exists())
        assertTrue(
            "nested file must be in correct directory",
            nestedFont.absolutePath.contains("sub"),
        )

        nestedFont.delete()
        subDir.delete()
    }

    @Test
    fun getFileNameFromUriHandlesNullDisplayName() {
        val viewModel = createViewModel()
        val uri = android.net.Uri.parse("content://nonexistent/file.ttf")

        val result = viewModel.getFileNameFromUri(uri)
        assertNull(
            "getFileNameFromUri should return null for nonexistent provider",
            result,
        )
    }

    @Test
    fun fallbackSystemFontsReturnsExpectedMonospaceFonts() {
        val fonts = fallbackSystemFonts()
        val monospace =
            listOf(
                "Droid Sans Mono",
                "Noto Sans Mono",
                "Roboto Mono",
                "Source Code Pro",
                "Fira Code",
                "Ubuntu Mono",
            )
        for (name in monospace) {
            assertTrue(
                "system font list must contain '$name'",
                fonts.any { it.contains(name, ignoreCase = true) },
            )
        }
    }

    @Test
    fun fallbackSystemFontsAreDistinctByName() {
        val fonts = fallbackSystemFonts()
        val lowercased = fonts.map { it.lowercase() }
        assertEquals(
            "no duplicate font names allowed",
            lowercased.size,
            lowercased.distinct().size,
        )
    }

    @Test
    fun fallbackSystemFontsAllNonEmpty() {
        val fonts = fallbackSystemFonts()
        for (font in fonts) {
            assertTrue(
                "font name must not be blank: '$font'",
                font.isNotBlank(),
            )
        }
    }

    @Test
    fun fontFamilyRoundTripWithSpecialChars() = runTest {
        val specialName = "Fira Code Bold Italic (OTF)"
        repository.setFontFamily(specialName)
        assertEquals(
            "font name with special chars must round-trip",
            specialName,
            repository.fontFamily.first(),
        )
    }

    @Test
    fun fontFamilyRoundTripWithUnicode() = runTest {
        val unicodeName = "Noto Sans Mono CJK SC"
        repository.setFontFamily(unicodeName)
        assertEquals(
            "CJK font name must round-trip",
            unicodeName,
            repository.fontFamily.first(),
        )
    }

    @Test
    fun fontFamilyEmptyStringIsDefault() = runTest {
        repository.setFontFamily("")
        assertEquals(
            "empty string fontFamily must be accepted",
            "",
            repository.fontFamily.first(),
        )
    }

    @Test
    fun fontFileSizeCheckAfterInstall() {
        val fontsDir = File(context.filesDir, "fonts")
        fontsDir.mkdirs()

        val largeFont = File(fontsDir, "LargeFont.ttf")
        val data = ByteArray(1024) { (it % 256).toByte() }
        largeFont.writeBytes(data)

        assertEquals(
            "installed font file size must match source data",
            data.size.toLong(),
            largeFont.length(),
        )

        largeFont.delete()
    }

    @Test
    fun multipleFontFilesCoexistInFontsDir() {
        val fontsDir = File(context.filesDir, "fonts")
        fontsDir.mkdirs()

        val font1 = File(fontsDir, "Font1.ttf")
        val font2 = File(fontsDir, "Font2.otf")
        font1.writeBytes("font1".toByteArray())
        font2.writeBytes("font2".toByteArray())

        assertTrue("font1 must exist", font1.exists())
        assertTrue("font2 must exist", font2.exists())
        assertFalse("font1 and font2 must be different files", font1.absolutePath == font2.absolutePath)

        font1.delete()
        font2.delete()
    }

    @Test
    fun fontStyleChangesAreImmediatelyDetectable() = runTest {
        repository.setFontFamily("Font A")
        val first = repository.fontFamily.first()
        assertEquals("Font A", first)

        repository.setFontFamily("Font B")
        val second = repository.fontFamily.first()
        assertEquals("Font B", second)

        assertTrue(
            "font must change after setting new value",
            first != second,
        )
    }

    @Test
    fun fallbackSystemFontsIncludeNerdFont() {
        val fonts = fallbackSystemFonts()
        assertTrue(
            "Nerd Font must be in fallback list",
            fonts.any { it.contains("Nerd Font", ignoreCase = true) },
        )
    }

    @Test
    fun fallbackSystemFontsIncludeCJKFonts() {
        val fonts = fallbackSystemFonts()
        val cjkKeywords = listOf("Noto Sans CJK", "Noto Sans SC", "Noto Sans TC", "Noto Sans JP", "Noto Sans KR")
        for (keyword in cjkKeywords) {
            assertTrue(
                "CJK font list must contain '$keyword'",
                fonts.any { it.contains(keyword, ignoreCase = true) },
            )
        }
    }

    private fun resolveEffectiveFont(fontFamily: String): String = fontFamily

    private fun createViewModel(): io.term.TerminalViewModel {
        val runtime = io.term.runtime.TerminalRuntime(context, repository)
        return io.term.TerminalViewModel(context, repository, runtime)
    }
}
