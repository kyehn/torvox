package io.term.settings

import android.content.Context
import android.os.StrictMode
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import dagger.hilt.android.qualifiers.ApplicationContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class SettingsDataStoreProvider
@Inject
constructor(
    @ApplicationContext private val context: Context,
) {
    internal val prefsDir: File =
        StrictMode.allowThreadDiskReads().let { prev ->
            context.getDir("prefs", Context.MODE_PRIVATE).also {
                StrictMode.setThreadPolicy(prev)
            }
        }

    val dataStore: DataStore<Preferences> =
        PreferenceDataStoreFactory.create {
            File(prefsDir, "settings.preferences_pb")
        }
}
