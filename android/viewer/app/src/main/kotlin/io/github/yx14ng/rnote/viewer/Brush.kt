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
        0xFFE0_1B24.toInt(),
        0xFF1C_71D8.toInt(),
        0xFF2E_C27E.toInt(),
        0xFFF5_C211.toInt(),
    )

    /**
     * [color] as ink on [paper]: a nearly black ink on dark paper is white, and a nearly white
     * one on light paper is black, keeping its transparency; other colors stay. The palette
     * keeps "black" and "white", and the pen writes what shows on the notebook open.
     */
    fun onPaper(color: Int, paper: Int): Int {
        val paperDark = luminance(paper) < 0.5
        val ink = luminance(color)
        val alpha = color and ALPHA_MASK
        return when {
            paperDark && ink < DARK_INK -> alpha or 0x00FF_FFFF
            !paperDark && ink > LIGHT_INK -> alpha
            else -> color
        }
    }

    /** The perceived brightness of [color], from 0 (black) to 1 (white). */
    private fun luminance(color: Int): Double {
        val red = (color shr 16) and 0xFF
        val green = (color shr 8) and 0xFF
        val blue = color and 0xFF
        return (0.299 * red + 0.587 * green + 0.114 * blue) / 255
    }

    private const val ALPHA_MASK = 0xFF shl 24

    /** Inks darker than this count as black, lighter than [LIGHT_INK] as white. */
    private const val DARK_INK = 0.25
    private const val LIGHT_INK = 0.9

    /** Black or white, whichever stands out on [background]. */
    fun contrastingColor(background: Int): Int {
        val luminance = (0.299 * Color.red(background) + 0.587 * Color.green(background) +
            0.114 * Color.blue(background)) / 255
        return if (luminance < 0.5) Color.WHITE else Color.BLACK
    }
}
