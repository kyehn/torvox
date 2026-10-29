package terminal.emulator.installer

/**
 * Termux 引导程序包（bootstrap）的发布坐标：仓库、版本与变体。
 *
 * 单一来源：设置页的「Termux 预设」按钮由此拼出下载 URL，测试也从这里取，
 * 使「我们支持哪个版本」这件事只写一次。此前版本号在设置页与三处测试夹具里
 * 各写一遍且已经漂移（设置页 `2026.02.12-r1`、夹具 `2026.06.21-r1`），任何一侧
 * 改动都不会让另一侧失败。
 *
 * 版本取自 termux-app `app/build.gradle` 的 downloadBootstraps 任务（DESIGN
 * Bootstrap 节）。夹具若需要另一个已知可下载的版本，那是**有意的**选择——安装器
 * 测试要的是一份真实存在的归档，与「产品预设指向哪个版本」是两件事，故在夹具处
 * 显式写出并注明。
 */
internal object TermuxBootstrap {
    /** 引导程序版本（`apt.android-7` 分支上的 termux-packages 发布标签）。 */
    const val RELEASE: String = "2026.02.12-r1"

    /** 包变体：DESIGN 规定只用 apt-android-7。 */
    const val VARIANT: String = "apt.android-7"

    private const val DOWNLOAD_BASE = "https://github.com/termux/termux-packages/releases/download"

    /**
     * 该版本 + 变体的下载 URL。`+` 在路径里必须转义为 `%2B`，否则 GitHub 会 404
     * ——这是一个静默失败（下载失败只表现为安装进度条停在某个百分比）。
     */
    fun downloadUrl(architecture: String): String =
        "$DOWNLOAD_BASE/bootstrap-$RELEASE%2B$VARIANT/bootstrap-$architecture.zip"

    /**
     * 该版本 + 变体 + 架构的归档 SHA-256；`null` = 本仓未钉此架构的校验和。
     *
     * 与 [RELEASE] 同源：termux-app `app/build.gradle` 的 `downloadBootstraps`
     * 逐架构公布 SHA-256（aarch64/arm/i686/x86_64）。本仓只支持 aarch64 与
     * x86_64，故只钉这两个；跟进版本时两者必须同时改，否则安装会立刻失败
     * ——这正是要的效果：宁可装不上，也不要装上未校验的归档。
     *
     * 为什么必须有它：https 只防链路篡改、不保证内容正确，而引导 URL 在设置里
     * 可由用户编辑，故 https 不构成信任根。归档随后会被解压并**执行**其中的
     * postinst 脚本，校验和是唯一能证明确实拿到了声明版本的手段。
     */
    fun sha256(architecture: String): String? = when (architecture) {
        "aarch64" -> "ea2aeba8819e517db711f8c32369e89e7c52cee73e07930ff91185e1ab93f4f3"
        "x86_64" -> "b7fd0f2e3a4de534be3144f9f91acc768630fc463eaf134ab2e64c545e834f7a"
        else -> null
    }
}
