package terminal.emulator.util

/** 半字节位数：一个字节的高 4 位右移量。 */
private const val NIBBLE_BITS = 4

/** 低半字节掩码。 */
private const val LOW_NIBBLE_MASK = 0x0F

/** 字节掩码：`Byte` 是有符号的，直接按位取会得到负数下标。 */
private const val BYTE_MASK = 0xFF

private val HEX_DIGITS = "0123456789abcdef".toCharArray()

/**
 * 小写十六进制表示。
 *
 * 不用标准库的 `ByteArray.toHexString()`：它在当前 Kotlin 版本仍是
 * `@ExperimentalStdlibApi`，签名与输出大小写都可能随版本变动，而这里的唯一用途是
 * 比对引导归档的 32 字节 sha256，长度固定、性能无关紧要。
 *
 * 之前这里写的是不存在的 `encodeToHex()`——那个名字从未存在于标准库，只在有编译
 * 环境的机器上才会暴露。输出取小写，与校验侧的 `equals(ignoreCase = true)` 配合。
 */
internal fun ByteArray.toLowerHex(): String {
    val out = CharArray(size * 2)
    forEachIndexed { index, byte ->
        val value = byte.toInt() and BYTE_MASK
        out[index * 2] = HEX_DIGITS[value ushr NIBBLE_BITS]
        out[index * 2 + 1] = HEX_DIGITS[value and LOW_NIBBLE_MASK]
    }
    return String(out)
}
