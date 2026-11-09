package io.github.yx14ng.rnote.viewer

import android.graphics.Color
import java.util.Locale

/** Parses and formats the free values typed in forms: numbers and colors. */
internal object FormValues {
    /** [value] with at most two decimals and no trailing zeros, e.g. `210` or `215.9`. */
    fun formatNumber(value: Double): String =
        String.format(Locale.ROOT, "%.2f", value).trimEnd('0').trimEnd('.')

    /**
     * The number in [text], accepting a decimal comma, or null if it isn't one or falls
     * outside [range].
     */
    fun parseNumber(text: String, range: ClosedFloatingPointRange<Double>): Double? =
        text.replace(',', '.').toDoubleOrNull()?.takeIf { it.isFinite() && it in range }

    /** `#RRGGBB`, or `#AARRGGBB` for colors that aren't opaque. */
    fun formatColor(color: Int): String =
        if (Color.alpha(color) == OPAQUE) {
            String.format(Locale.ROOT, "#%06X", color and RGB_MASK)
        } else {
            String.format(Locale.ROOT, "#%08X", color)
        }

    /** The color in [text] as `RRGGBB` or `AARRGGBB` hex, with or without `#`, or null. */
    fun parseColor(text: String): Int? {
        val hex = text.removePrefix("#")
        if (hex.length != RGB_DIGITS && hex.length != ARGB_DIGITS) {
            return null
        }
        val value = hex.toLongOrNull(radix = 16) ?: return null
        return if (hex.length == RGB_DIGITS) (value or OPAQUE_ALPHA).toInt() else value.toInt()
    }

    private const val OPAQUE = 0xFF
    private const val RGB_MASK = 0xFFFFFF
    private const val OPAQUE_ALPHA = 0xFF000000L
    private const val RGB_DIGITS = 6
    private const val ARGB_DIGITS = 8
}
