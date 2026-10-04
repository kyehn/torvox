package terminal.emulator.util

// 终端单元格宽度的唯一来源，取自 Markus Kuhn 的 wcwidth 表。

/** Markus Kuhn wcwidth 表中的 BMP 宽字符区间。 */
fun isWideBmp(cp: Int): Boolean = cp in 0x1100..0x115F || // 谚文字母
    cp in 0x2329..0x232A || // 尖括号
    cp in 0x2E80..0x303E || // CJK 部首补充 .. CJK 符号和标点
    cp in 0x3041..0x33FF || // 平假名 .. CJK 兼容
    cp in 0x3400..0x4DBF || // CJK 扩展 A
    cp in 0x4E00..0x9FFF || // CJK 统一表意文字
    cp in 0xA000..0xA4CF || // 彝文音节
    cp in 0xAC00..0xD7A3 || // 谚文音节
    cp in 0xF900..0xFAFF || // CJK 兼容表意文字
    cp in 0xFE30..0xFE4F || // CJK 兼容形式
    cp in 0xFF00..0xFF60 || // 全角形式
    cp in 0xFFE0..0xFFE6 // 全角符号

/** 宽字符的星平面区间（emoji 与 CJK 扩展 B-G）。 */
fun isWideAstral(cp: Int): Boolean = cp in 0x1F1E6..0x1F1FF || // 区域指示符（旗帜）
    cp in 0x1F300..0x1F64F || // 表情符号
    cp in 0x1F680..0x1F6FF || // 交通与地图符号
    cp in 0x1F700..0x1F8FF || // 炼金术 .. 几何图形扩展
    cp in 0x1F900..0x1F9FF || // 补充符号
    cp in 0x1FA00..0x1FAFF || // 国际象棋 .. 符号扩展 A
    cp in 0x20000..0x2FFFD || // CJK 扩展 B-F
    cp in 0x30000..0x3FFFD // CJK 扩展 G

/** 码点是否占两个单元格。 */
fun isWideCodePoint(cp: Int): Boolean = isWideBmp(cp) || isWideAstral(cp)
