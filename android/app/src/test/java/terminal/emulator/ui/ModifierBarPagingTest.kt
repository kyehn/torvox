package terminal.emulator.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * paginateToolbarKeys mirrors termux's ViewPager extra-keys paging: the flat
 * layout splits at the midpoint into two rows, pairs top/bottom columns, and
 * chunks columns into swipeable pages. The default 14-key layout must be a
 * single 7-column page; adding more keys grows pages of at most 7 columns.
 */
class ModifierBarPagingTest {

    private val keys: List<ToolbarItem> = ToolbarKey.entries.map { ToolbarItem.Default(it) }

    private fun defaultKeys(count: Int): List<ToolbarItem> = keys.take(count)

    @Test
    fun `default 14 key layout is one page of 7 columns`() {
        val pages = paginateToolbarKeys(defaultKeys(14))
        assertEquals(1, pages.size)
        assertEquals(7, pages[0].size)
        // Row 1 is the first 7 keys; row 2 the last 7.
        assertEquals("ESC", pages[0][0].first?.let { (it as ToolbarItem.Default).key.defaultLabel })
        assertEquals("TAB", pages[0][0].second?.let { (it as ToolbarItem.Default).key.defaultLabel })
        assertEquals("PGUP", pages[0][6].first?.let { (it as ToolbarItem.Default).key.defaultLabel })
        assertEquals("PGDN", pages[0][6].second?.let { (it as ToolbarItem.Default).key.defaultLabel })
    }

    @Test
    fun `extra keys page at the column limit`() {
        // 21 keys: midpoint 11, so 11 columns -> pages of 7 + 4 columns.
        val pages = paginateToolbarKeys(defaultKeys(21))
        assertEquals(2, pages.size)
        assertEquals(7, pages[0].size)
        assertEquals(4, pages[1].size)
    }

    @Test
    fun `odd key count pairs trailing single key above empty slot`() {
        // 15 keys: midpoint 8 -> 8 columns, the last column has a top key
        // and no bottom key (row 2 only holds 7).
        val pages = paginateToolbarKeys(defaultKeys(15))
        assertEquals(2, pages.size)
        val lastColumn = pages[1].last()
        // Row 1 is the first 8 keys (ends at TAB); row 2 the remaining 7.
        assertEquals("TAB", (lastColumn.first as ToolbarItem.Default).key.defaultLabel)
        assertNull(lastColumn.second)
    }

    @Test
    fun `empty layout pages to nothing`() {
        assertEquals(0, paginateToolbarKeys(emptyList()).size)
    }
}
