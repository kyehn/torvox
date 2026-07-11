package io.torvox.settings

import android.content.Context
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
    internal val prefsDir: File = context.getDir("prefs", Context.MODE_PRIVATE)

    val dataStore: DataStore<Preferences> =
        PreferenceDataStoreFactory.create {
            File(prefsDir, "settings.preferences_pb")
        }
}
