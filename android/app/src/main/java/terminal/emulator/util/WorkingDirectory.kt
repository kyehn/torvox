package terminal.emulator.util

/** `file://` 方案的固定前缀。 */
private const val FILE_SCHEME_PREFIX = "file://"

/** 路径分隔符。 */
private const val PATH_SEPARATOR = '/'

/**
 * OSC 7 上报的工作目录 → 可读路径。
 *
 * shell 按 xterm 约定上报 `file://<host>/<path>`，host 可能是 `localhost`、空串或主机名。
 * 界面只需要路径部分——把 `file://localhost` 显示在会话列表里只会挤掉真正有用的信息。
 *
 * 不是 `file://` 开头的原样返回：部分 shell 直接上报裸路径，那也是合法的 OSC 7 载荷，
 * 强行当作 URI 解析反而会把 `/home/u` 这类值弄成空串。
 *
 * 拿不到路径（`file://` 之后没有斜杠）时返回空串，调用方据此回退到标题——
 * 拼接一个猜测路径会让列表显示一个并不存在的目录。
 */
internal fun formatWorkingDirectory(raw: String): String {
    if (raw.isEmpty()) return ""
    if (!raw.startsWith(FILE_SCHEME_PREFIX)) return raw
    val afterScheme = raw.removePrefix(FILE_SCHEME_PREFIX)
    val pathStart = afterScheme.indexOf(PATH_SEPARATOR)
    return if (pathStart >= 0) afterScheme.substring(pathStart) else ""
}
