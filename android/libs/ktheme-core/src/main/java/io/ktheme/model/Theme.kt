package io.ktheme.model

data class ThemeMetadata(
    val id: String = "",
    val name: String = "",
    val description: String = ""
)

data class SemanticRoles(
    val success: String? = null,
    val critical: String? = null,
    val warning: String? = null,
    val info: String? = null
)

data class ColorScheme(
    val primary: String = "#000000",
    val secondary: String = "#777777",
    val background: String = "#FFFFFF",
    val surface: String = "#FFFFFF",
    val error: String = "#FF0000",
    val semanticRoles: SemanticRoles? = null
)

data class Theme(
    val metadata: ThemeMetadata = ThemeMetadata(),
    val colorScheme: ColorScheme = ColorScheme()
)
