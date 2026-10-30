package terminal.emulator.util

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * OSC 7 工作目录 → 可读路径（[formatWorkingDirectory]）。
 *
 * shell 按 xterm 约定上报 `file://<host>/<path>`，host 可能是 `localhost`、空串或主机名。
 * 界面上只该出现路径：把 `file://localhost` 一并显示，会把真正有区分度的目录挤掉。
 */
class WorkingDirectoryTest {

    @Test
    fun `local host prefix is stripped`() {
        assertEquals("/home/user/src", formatWorkingDirectory("file://localhost/home/user/src"))
    }

    /** `file:///path`（空 host）是同一约定的另一种合法写法。 */
    @Test
    fun `empty host keeps the whole path`() {
        assertEquals("/home/user/src", formatWorkingDirectory("file:///home/user/src"))
    }

    /** 主机名同样被去掉：远端主机名对本地会话列表没有意义。 */
    @Test
    fun `named host is stripped as well`() {
        assertEquals("/srv/app", formatWorkingDirectory("file://buildbox/srv/app"))
    }

    /**
     * 根目录不能被当成「没有路径」。
     *
     * `file://localhost/` 的路径是 `/`，若实现按 `indexOf('/') > 0` 判定就会返回空串，
     * 会话在根目录时列表里什么都不显示。
     */
    @Test
    fun `root directory survives`() {
        assertEquals("/", formatWorkingDirectory("file://localhost/"))
        assertEquals("/", formatWorkingDirectory("file:///"))
    }

    /**
     * 裸路径原样返回。
     *
     * 部分 shell 直接发 `/home/user` 而不是 URI；把它当 URI 解析会得出空串，
     * 而那份信息本来是完全可用的。
     */
    @Test
    fun `bare paths pass through untouched`() {
        assertEquals("/home/user", formatWorkingDirectory("/home/user"))
        assertEquals("relative/dir", formatWorkingDirectory("relative/dir"))
    }

    @Test
    fun `empty input stays empty`() {
        assertEquals("", formatWorkingDirectory(""))
    }

    /**
     * 只有 scheme 没有路径时返回空串。
     *
     * 这不是合法上报，但也不能凭空造一个 `/` 出来——那会让界面显示一个 shell 并未
     * 声称的目录。
     */
    @Test
    fun `scheme without a path yields empty`() {
        assertEquals("", formatWorkingDirectory("file://localhost"))
        assertEquals("", formatWorkingDirectory("file://"))
    }
}
