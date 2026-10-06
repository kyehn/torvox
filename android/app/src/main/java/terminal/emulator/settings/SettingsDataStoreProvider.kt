package terminal.emulator.settings

import android.content.Context
import android.os.StrictMode
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.emptyPreferences
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

// 非数据类：提供 DataStore 装配（含 StrictMode 磁盘读副作用的单例服务），
// 对其生成 equals/hashCode 会产生误导。
@Suppress("UseDataClass")
@Singleton
class SettingsDataStoreProvider
@Inject
constructor(@ApplicationContext private val context: Context) {
    internal val prefsDir: File =
        StrictMode.allowThreadDiskReads().let { prev ->
            context.getDir("prefs", Context.MODE_PRIVATE).also {
                StrictMode.setThreadPolicy(prev)
            }
        }

    val dataStore: DataStore<Preferences> =
        PreferenceDataStoreFactory.create(
            corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
        ) {
            File(prefsDir, "settings.preferences_pb")
        }

    /** 屏幕宽度（dp），用于按设备自适应计算默认字号。 */
    internal val screenWidthDp: Float
        get() {
            val metrics = context.resources.displayMetrics
            return metrics.widthPixels / metrics.density
        }
}
