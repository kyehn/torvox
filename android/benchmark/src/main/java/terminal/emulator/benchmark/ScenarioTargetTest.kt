package terminal.emulator.benchmark

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 被测目标解析的判据。
 *
 * 跨应用对比靠仪器参数选包：参数写错就会静默地只测默认包，对比数据看着齐、其实只有一个
 * 应用。故空值、空串与只有分隔符都 MUST 退回默认包名，多个包名按逗号切分并逐项去空白。
 */
class ScenarioTargetTest {
    @Test
    fun fallsBackToDefaultPackage() {
        assertEquals(TARGET_PACKAGE, parseTargetPackage(null))
        assertEquals(TARGET_PACKAGE, parseTargetPackage(""))
        assertEquals(TARGET_PACKAGE, parseTargetPackage("   "))
        assertEquals(listOf(TARGET_PACKAGE), parseTargetPackages(null))
        assertEquals(listOf(TARGET_PACKAGE), parseTargetPackages(""))
        assertEquals(listOf(TARGET_PACKAGE), parseTargetPackages(" , ,"))
    }

    @Test
    fun readsSingleTargetPackage() {
        assertEquals("com.example.other", parseTargetPackage(" com.example.other "))
    }

    @Test
    fun splitsTargetPackagesAndDropsEmptyEntries() {
        assertEquals(
            listOf("com.example.first", "com.example.second"),
            parseTargetPackages(" com.example.first , ,com.example.second "),
        )
    }
}
