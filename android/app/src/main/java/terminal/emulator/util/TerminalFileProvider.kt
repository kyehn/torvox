package terminal.emulator.util

/**
 * 选择菜单打开文件走系统 API 可回写（FileProvider 授权读写）。
 * 独立子类以避开 debugoverlay 的同名 FileProvider 清单合并冲突。
 */
class TerminalFileProvider : androidx.core.content.FileProvider()
