package terminal.emulator.benchmark

import android.os.SystemClock
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice

/**
 * 会话标题哨兵：把 PS1 改成不含转义的普通提示符，避免 mkshrc 默认 PS1 每次改写会话
 * 标题（带 OSC 0）把哨兵立刻覆盖掉；再通过会话抽屉读到的标题校验哨兵是否留得住。
 */
internal fun disablePromptTitleReport(device: UiDevice) {
    device.executeShellCommand("input text PS1='>'")
    device.executeShellCommand("input keyevent 66")
}

/**
 * 等标题哨兵出现在会话抽屉里：抽屉打开时应用强制刷新会话标题，故读到的是当下的
 * 标题而不是打开前的旧值。
 */
internal fun awaitSessionTitleCanary(device: UiDevice, canary: String): Boolean {
    val drawerKey = requireElement(device, DRAWER_KEY_RESOURCE_ID, "抽屉键")
    drawerKey.click()
    requireElement(device, SESSION_DRAWER_RESOURCE_ID, "会话抽屉")
    val deadline = SystemClock.elapsedRealtime() + SESSION_TITLE_TIMEOUT_MILLIS
    var canaryVisible = false
    while (SystemClock.elapsedRealtime() < deadline) {
        if (device.findObject(By.textContains(canary)) != null) {
            canaryVisible = true
            break
        }
        device.waitForIdle()
        SystemClock.sleep(SESSION_TITLE_POLL_MILLIS)
    }
    var closeAttempts = 0
    while (
        closeAttempts < SESSION_DRAWER_CLOSE_ATTEMPTS &&
        device.findObject(By.res(SESSION_DRAWER_RESOURCE_ID)) != null
    ) {
        device.pressBack()
        device.waitForIdle()
        closeAttempts++
    }
    return canaryVisible
}
