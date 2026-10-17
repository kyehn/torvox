package terminal.emulator.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * 结果计数必须说真话：未按过上一个/下一个时上游没有选中命中，此时报「第 1 个」是假话，
 * 只能报总数。判别力：计数若恒按 `currentResultIndex + 1` 渲染，第一条用例会读到
 * 「第 1 个，共 3 个」而失败。
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34])
class TextSearchBarCounterRobolectricTest {
    @get:Rule val composeRule = createComposeRule()

    private fun showCounter(resultCount: Int, currentResultIndex: Int?) {
        composeRule.setContent {
            MaterialTheme {
                TextSearchBar(
                    query = "MARKER",
                    onQueryChange = {},
                    resultCount = resultCount,
                    currentResultIndex = currentResultIndex,
                    onPrevious = {},
                    onNext = {},
                    onClose = {},
                )
            }
        }
    }

    @Test
    fun `no selection yet reports the total instead of a position`() {
        showCounter(resultCount = 3, currentResultIndex = null)
        composeRule.onNodeWithTag("SearchResultCount").assertTextEquals("共 3 个")
    }

    @Test
    fun `selected match reports its position`() {
        showCounter(resultCount = 3, currentResultIndex = 1)
        composeRule.onNodeWithTag("SearchResultCount").assertTextEquals("第 2 个，共 3 个")
    }

    @Test
    fun `no hit reports no result`() {
        showCounter(resultCount = 0, currentResultIndex = null)
        composeRule.onNodeWithTag("SearchResultCount").assertTextEquals("无结果")
    }
}
