package io.ktheme.engine

object HexColor {
    fun parseArgb(hex: String): Long {
        val cleanHex = hex.removePrefix("#")
        return when (cleanHex.length) {
            6 -> ("FF" + cleanHex).toLong(16)
            8 -> cleanHex.toLong(16)
            else -> 0xFF000000L
        }
    }
}
