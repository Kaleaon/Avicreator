package io.ktheme.compose

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import io.ktheme.model.Theme
import io.ktheme.presets.Presets

val LocalKtheme = compositionLocalOf<Theme> {
    Presets.load("navy-gold")
}

@Composable
fun KthemeTheme(
    theme: Theme = Presets.load("navy-gold"),
    content: @Composable () -> Unit
) {
    CompositionLocalProvider(
        LocalKtheme provides theme,
        content = content
    )
}
