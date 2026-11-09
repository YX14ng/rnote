package io.github.yx14ng.rnote.viewer

import android.graphics.Color

/** Color (an `android.graphics.Color` int) and width, in document units, of brush strokes. */
data class Brush(val color: Int, val width: Double)

/** A choice in the color or width menus; [label] is a string resource. */
class BrushOption<T>(val label: Int, val value: T)

/** The colors and widths the editing menus offer. */
object BrushPalette {
    val colors = listOf(
        BrushOption(R.string.color_black, Color.BLACK),
        BrushOption(R.string.color_white, Color.WHITE),
        BrushOption(R.string.color_red, Color.rgb(0xE0, 0x1B, 0x24)),
        BrushOption(R.string.color_blue, Color.rgb(0x1C, 0x71, 0xD8)),
        BrushOption(R.string.color_green, Color.rgb(0x2E, 0xC2, 0x7E)),
        BrushOption(R.string.color_yellow, Color.rgb(0xF5, 0xC2, 0x11)),
    )

    val widths = listOf(
        BrushOption(R.string.width_fine, 1.5),
        BrushOption(R.string.width_medium, 3.0),
        BrushOption(R.string.width_thick, 6.0),
    )

    val DEFAULT_WIDTH = widths[1].value

    /** Black or white, whichever stands out on [background]. */
    fun contrastingColor(background: Int): Int {
        val luminance = (0.299 * Color.red(background) + 0.587 * Color.green(background) +
            0.114 * Color.blue(background)) / 255
        return if (luminance < 0.5) Color.WHITE else Color.BLACK
    }
}
