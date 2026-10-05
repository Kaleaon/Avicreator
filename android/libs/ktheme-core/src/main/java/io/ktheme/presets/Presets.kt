package io.ktheme.presets

import io.ktheme.model.ColorScheme
import io.ktheme.model.SemanticRoles
import io.ktheme.model.Theme
import io.ktheme.model.ThemeMetadata

object Presets {
    private val presetsMap = mapOf(
        "navy-gold" to Theme(
            metadata = ThemeMetadata("navy-gold", "Navy Gold", "Navy and Gold theme"),
            colorScheme = ColorScheme(
                primary = "#002040",
                secondary = "#D4AF37",
                background = "#0A101D",
                surface = "#121B2D",
                error = "#D32F2F",
                semanticRoles = SemanticRoles(success = "#388E3C", critical = "#D32F2F")
            )
        ),
        "dark-emerald" to Theme(
            metadata = ThemeMetadata("dark-emerald", "Dark Emerald", "Dark Emerald theme"),
            colorScheme = ColorScheme(
                primary = "#043927",
                secondary = "#50C878",
                background = "#051A10",
                surface = "#0B291B",
                error = "#D32F2F",
                semanticRoles = SemanticRoles(success = "#50C878", critical = "#D32F2F")
            )
        )
    )

    fun load(id: String): Theme {
        return presetsMap[id] ?: presetsMap["navy-gold"]!!
    }

    fun all(): List<Theme> {
        return presetsMap.values.toList()
    }
}
