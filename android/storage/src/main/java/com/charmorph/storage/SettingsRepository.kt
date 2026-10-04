package com.charmorph.storage

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.preferencesDataStore
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

import androidx.datastore.preferences.core.stringPreferencesKey

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "settings")

@Singleton
class SettingsRepository @Inject constructor(
    @ApplicationContext private val context: Context
) {
    companion object {
        val SHOW_ANATOMICAL_DETAILS = booleanPreferencesKey("show_anatomical_details")
        val THEME_PRESET_ID = stringPreferencesKey("theme_preset_id")
    }

    val showAnatomicalDetails: Flow<Boolean> = context.dataStore.data
        .map { preferences ->
            preferences[SHOW_ANATOMICAL_DETAILS] ?: false
        }

    val themePresetId: Flow<String> = context.dataStore.data
        .map { preferences ->
            preferences[THEME_PRESET_ID] ?: "navy-gold"
        }

    suspend fun setShowAnatomicalDetails(show: Boolean) {
        context.dataStore.edit { preferences ->
            preferences[SHOW_ANATOMICAL_DETAILS] = show
        }
    }

    suspend fun setThemePresetId(presetId: String) {
        context.dataStore.edit { preferences ->
            preferences[THEME_PRESET_ID] = presetId
        }
    }
}
