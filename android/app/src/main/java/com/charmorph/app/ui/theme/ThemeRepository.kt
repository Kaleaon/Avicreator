package com.charmorph.app.ui.theme

import com.charmorph.storage.SettingsRepository
import io.ktheme.model.Theme
import io.ktheme.presets.Presets
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class ThemeRepository @Inject constructor(
    private val settingsRepository: SettingsRepository
) {
    val fallbackTheme: Theme
        get() = getThemeOrDefault("navy-gold")

    val activePresetId: Flow<String> = settingsRepository.themePresetId

    val activeTheme: Flow<Theme> = settingsRepository.themePresetId.map { presetId ->
        getThemeOrDefault(presetId)
    }

    fun getThemeOrDefault(presetId: String): Theme {
        return try {
            Presets.load(presetId)
        } catch (e: Exception) {
            Presets.load("navy-gold")
        }
    }

    fun getAvailablePresets(): List<Theme> {
        return Presets.all()
    }

    suspend fun selectPreset(presetId: String) {
        settingsRepository.setThemePresetId(presetId)
    }
}
