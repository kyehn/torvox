package terminal.emulator.bridge

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.lang.reflect.Modifier

/**
 * JNI 导出必须是静态方法：`object` 里的 `external fun` 不加 `@JvmStatic` 会编译成
 * 实例方法，第二个参数槽位是 `jobject` 而原生签名按 `jclass` 解释——只因两者同为
 * 引用才「恰好」能跑，CheckJNI 下报错。本测试锁死该约定。
 */
class NativeBridgeStaticTest {
    @Test
    fun `every jni export is a static method`() {
        val nonStatic =
            NativeBridge::class
                .java.declaredMethods
                .filter { Modifier.isNative(it.modifiers) }
                .filterNot { Modifier.isStatic(it.modifiers) }
                .map { it.name }
        assertEquals(
            "JNI 导出必须全部 @JvmStatic：${nonStatic.joinToString()}",
            emptyList<String>(),
            nonStatic,
        )
    }

    @Test
    fun `jni exports are declared`() {
        val exports = NativeBridge::class.java.declaredMethods.count { Modifier.isNative(it.modifiers) }
        assertTrue("NativeBridge 必须声明 JNI 导出", exports > 0)
    }
}
