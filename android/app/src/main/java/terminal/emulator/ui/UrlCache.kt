package terminal.emulator.ui

import java.util.LinkedHashMap

/**
 * LRU cache of detected URLs per visible screen region.
 * Key: scrollOffset (top visible row index).
 * Value: list of detected URL strings.
 * Reference: research-supplement-4.md §1.3 URL lazy rebuild.
 */
object UrlCache {
    private const val MAX_ENTRIES = 8

    private val cache = object : LinkedHashMap<Int, List<String>>(MAX_ENTRIES, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Int, List<String>>?) = size > MAX_ENTRIES
    }

    @Synchronized
    fun getOrCompute(scrollOffset: Int, compute: () -> List<String>): List<String> = cache[scrollOffset] ?: compute().also { cache[scrollOffset] = it }

    @Synchronized
    fun invalidate(scrollOffset: Int) {
        cache.remove(scrollOffset)
    }

    @Synchronized
    fun invalidateAll() {
        cache.clear()
    }
}
