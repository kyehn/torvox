package terminal.emulator

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * STATUS ③「会话 exit 0 时不关闭会话/应用」的判据：最后一个会话消失后应用必须关闭
 * （termux 语义），而冷启动时「尚未创建会话」的空表不得触发关闭——这条正是最容易被
 * 「简化」成 `sessionIds.isEmpty() && finish()` 而把应用在启动瞬间关掉的回归点。
 */
class SessionTableWatcherTest {

    @Test
    fun coldStartWithNoSessionsYetDoesNotFinish() {
        val watcher = SessionTableWatcher()
        assertFalse("冷启动空会话表必须不关闭应用", watcher.onSessionCount(0))
        assertFalse("空表重复发射同样不关闭", watcher.onSessionCount(0))
    }

    @Test
    fun lastSessionClosedFinishesOnce() {
        val watcher = SessionTableWatcher()
        assertFalse("会话存在时不关闭", watcher.onSessionCount(1))
        assertTrue("最后一个会话消失即关闭", watcher.onSessionCount(0))
        assertFalse("已触发后残留发射不得重复关闭", watcher.onSessionCount(0))
    }

    @Test
    fun backgroundSessionStillCountsAsASession() {
        val watcher = SessionTableWatcher()
        assertFalse("多个会话存在时不关闭", watcher.onSessionCount(3))
        assertFalse("仍有会话存活时不关闭", watcher.onSessionCount(1))
        assertTrue("最后一个会话消失即关闭", watcher.onSessionCount(0))
    }
}
