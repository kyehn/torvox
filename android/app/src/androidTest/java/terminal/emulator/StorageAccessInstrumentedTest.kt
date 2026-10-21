package terminal.emulator

import android.os.Environment
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * 外部存储可达性与清单声明的契约。
 *
 * DESIGN「声明文件权限（不提示，允许用户自行授权），支持 `/sdcard` 目录」：
 * 文件权限必须由本应用**显式**声明——靠传递依赖顺带捎上不算声明，依赖一升级
 * 权限就没了，用户看到的是「装了却访问不了外部存储」。
 *
 * 「不提示」指应用不主动弹权限对话框：授权入口在系统设置里，由用户自行开启。
 * 因此本用例只断言「声明存在」与「声明不弹窗」，不代替用户去点授权。
 */
class StorageAccessInstrumentedTest {
    @Test
    fun manifest_declares_file_permissions_explicitly() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val declared =
            context.packageManager
                .getPackageInfo(context.packageName, android.content.pm.PackageManager.GET_PERMISSIONS)
                .requestedPermissions
                ?.toSet()
                .orEmpty()
        assertTrue(
            "应用必须显式声明 WRITE_EXTERNAL_STORAGE，实际声明=$declared",
            declared.contains(android.Manifest.permission.WRITE_EXTERNAL_STORAGE),
        )
        assertTrue(
            "应用必须显式声明 MANAGE_EXTERNAL_STORAGE，实际声明=$declared",
            declared.contains(android.Manifest.permission.MANAGE_EXTERNAL_STORAGE),
        )
    }

    /**
     * `/sdcard` 必须在应用进程内可读可写。
     *
     * 走 `Environment.getExternalStorageDirectory()` 而非硬编码 `/sdcard`：前者是
     * 平台给出的真实路径，跨设备与多用户场景下都可能不同。
     *
     * 断言的是「能列出目录」与「能写文件并读回」两件事——只断言 `exists()` 会把
     * 「路径在、却进不去」的情形放过，那正是用户报障的样子。
     *
     * 授权入口在系统设置里、由用户自行开启（DESIGN「不提示，允许用户自行授权」），
     * 故本用例先自行完成那一步授权（等价于用户在系统设置里打开「所有文件访问」），
     * 再断言授权后的行为；「是否弹窗索要」由应用侧保证，不在本用例范围。
     */
    @Test
    fun external_storage_root_is_reachable_when_authorized() {
        grantAllFilesAccess()

        val externalRoot = Environment.getExternalStorageDirectory()
        assertTrue(
            "外部存储根目录必须存在：$externalRoot",
            externalRoot.exists(),
        )
        val children =
            requireNotNull(
                externalRoot.list(),
            ) {
                "外部存储根目录必须可列出（未授予「所有文件访问」时会被系统拒绝）"
            }
        UxTestUtils.metric("external_storage_entries", children.size)
        // 实写一次：能列目录不等于能写——写权限缺失时只有写会失败。
        val probe = File(externalRoot, "storage-probe.txt")
        try {
            probe.writeText("storage probe")
            assertEquals("storage probe", probe.readText())
        } finally {
            probe.delete()
        }
    }

    /**
     * 打开「所有文件访问」（`MANAGE_EXTERNAL_STORAGE` 的 appop）。
     *
     * 走 `appops set` 而非 `pm grant`：该权限在 Android 11+ 是由 appop 承载的
     * 特殊权限，`pm grant` 对它无效。命令以 shell 身份执行，等价于用户在
     * 系统设置里手动打开同一开关。
     */
    private fun grantAllFilesAccess() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val packageName = instrumentation.targetContext.packageName
        val output =
            instrumentation.uiAutomation.executeShellCommand("appops set $packageName MANAGE_EXTERNAL_STORAGE allow")
                .use { descriptor ->
                    java.io.FileInputStream(descriptor.fileDescriptor).readBytes().toString(Charsets.UTF_8)
                }
        assertTrue("开启「所有文件访问」失败：$output", output.isBlank())
    }
}
