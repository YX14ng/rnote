package io.github.yx14ng.rnote.viewer

import android.graphics.Color

/** Color (an `android.graphics.Color` int) and width, in document units, of brush strokes. */
data class Brush(val color: Int, val width: Double)

/** The colors and widths the editor starts with. */
object BrushPalette {
    /** The brush width before the user picks one, in document units. */
    const val DEFAULT_WIDTH = 3.0

    /** The colors the tool sidebar starts with, before the user changes any. */
    val DEFAULT_SIDEBAR_COLORS = listOf(
        Color.BLACK,
        Color.rgb(0xE0, 0x1B, 0x24),
        Color.rgb(0x1C, 0x71, 0xD8),
        Color.rgb(0x2E, 0xC2, 0x7E),
        Color.rgb(0xF5, 0xC2, 0x11),
    )

    /** Black or white, whichever stands out on [background]. */
    fun contrastingColor(background: Int): Int {
        val luminance = (0.299 * Color.red(background) + 0.587 * Color.green(background) +
            0.114 * Color.blue(background)) / 255
        return if (luminance < 0.5) Color.WHITE else Color.BLACK
    }
}
