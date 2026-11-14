package terminal.emulator.util

import java.io.File

/**
 * 目录列举的注入点：返回 null 表示「列举不出来」（I/O 失败），不是「目录为空」。
 *
 * 注入点只为在 JVM 单测里构造该前提：靠权限位制造不可列举的目录在 root 或
 * 特殊挂载下不成立，用例会被静默跳过——等于没测。
 *
 * 三处调用方（文档提供者的查询侧与变更侧、bootstrap 第二阶段）共用同一个接缝，
 * 语义与理由只有这一份。
 */
internal typealias DirectoryListing = (File) -> Array<File>?
