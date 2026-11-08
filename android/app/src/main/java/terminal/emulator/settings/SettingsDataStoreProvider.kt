package terminal.emulator.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.emptyPreferences
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

// 非数据类：提供 DataStore 装配的单例服务，
// 对其生成 equals/hashCode 会产生误导。
@Suppress("UseDataClass")
@Singleton
class SettingsDataStoreProvider
@Inject
constructor(@ApplicationContext private val context: Context) {
    /**
     * 设置持久化的存储。
     *
     * 目录创建推迟到 DataStore 的惰性文件回调里：DataStore 只在首次读写时于自己的
     * IO 作用域（`Dispatchers.IO`）上调用该回调，主线程构造期因此不再产生磁盘写，
     * StrictMode 的 DiskWriteViolation 随之消失。构造期先 `getDir` 建目录再把路径
     * 交给 DataStore，则那次建目录就落在主线程上——StrictMode 允许线程磁盘读
     * （`allowThreadDiskReads`）也只放宽「读」，放宽不了「写」。
     */
    val dataStore: DataStore<Preferences> =
        PreferenceDataStoreFactory.create(
            corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
        ) {
            File(context.getDir("prefs", Context.MODE_PRIVATE), "settings.preferences_pb")
        }

    /**
     * 屏幕宽度（dp），用于按设备自适应计算默认字号。
     *
     * 与 `TerminalRuntime.screenWidthDp` 取同一个来源：`resources.configuration`。
     * 此处曾自行用 `displayMetrics.widthPixels / density` 换算，那是**应用窗口**的
     * 物理宽换算，与 `configuration.screenWidthDp` 在系统字体缩放、display cutout、
     * 多窗口下都给不出同一个值。同一个「屏幕宽」三处三种算法时，自适应默认字号与
     * 字号上界会按不同宽度推导——两者本应是同一条列↔字号比例。
     */
    internal val screenWidthDp: Float
        get() =
            context.resources.configuration.screenWidthDp
                .toFloat()
}
