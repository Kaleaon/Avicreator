package com.charmorph.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import io.ktheme.compose.KthemeTheme
import io.ktheme.model.Theme
import io.ktheme.presets.Presets

@Composable
fun CharMorphTheme(
    theme: Theme? = null,
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    val activeTheme = theme ?: Presets.load("navy-gold")
    KthemeTheme(theme = activeTheme, content = content)
}
