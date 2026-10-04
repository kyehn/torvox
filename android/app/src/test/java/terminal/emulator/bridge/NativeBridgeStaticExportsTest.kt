package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 每个 `external fun` 必须是 `@JvmStatic`。
 *
 * 原生导出全部是静态的（形参第二位为 `jclass`）。缺注解时 JVM 绑定的是**实例**方法，
 * 于是第 3 个 JNI 槽位传的是 `this`，而原生按 `jclass` 接收——两者都是 64 位，
 * 加上 `_class` 从不解引用，调用表面正常；只有 debuggable 构建开启的 CheckJNI 会报
 * 参数类型错误。曾有 6 个生产导出（setTheme / setFontFamily / loadFontFile /
 * setExtraFontPaths / getCellHeight / setScrollOffset）在无声地错位。
 */
class NativeBridgeStaticExportsTest {

    @Test
    fun `every external export is static`() {
        val missing =
            NativeBridge::class.java.declaredMethods
                .filter { it.modifiers and java.lang.reflect.Modifier.NATIVE != 0 }
                .filterNot { method -> method.annotations.any { it is JvmStatic } }
                .map { it.name }
                .sorted()
        assertEquals("缺少 @JvmStatic 的 JNI 导出", emptyList<String>(), missing)
    }

    @Test
    fun `the bridge really exposes native exports`() {
        // 断言本身有效：确保上面的扫描不是空集合上的恒真断言。
        val nativeMethods =
            NativeBridge::class.java.declaredMethods
                .count { it.modifiers and java.lang.reflect.Modifier.NATIVE != 0 }
        assertTrue("NativeBridge 必须声明 native 方法，实际 $nativeMethods 个", nativeMethods > 0)
    }
}
