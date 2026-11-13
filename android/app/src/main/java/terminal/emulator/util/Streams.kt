package terminal.emulator.util

import java.io.InputStream

/**
 * 读取至多 [limit] 字节；流提前结束则返回实际读到的内容。
 *
 * minSdk 31 没有 `InputStream.readNBytes`（API 33），调用处需要限界读取以避免 OOM。
 */
fun InputStream.readAtMost(limit: Int): ByteArray {
    val buffer = ByteArray(limit)
    var offset = 0
    while (offset < limit) {
        val read = read(buffer, offset, limit - offset)
        if (read < 0) break
        offset += read
    }
    return if (offset == limit) buffer else buffer.copyOf(offset)
}
