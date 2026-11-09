package io.github.yx14ng.rnote.viewer

import android.graphics.Color
import kotlin.math.abs

/**
 * Font families for text. Generic names keep documents portable: the desktop app resolves
 * them with the system fonts, and the app maps them to Android's in its fontconfig setup.
 */
enum class FontFamily(val family: String, val label: Int) {
    SERIF("serif", R.string.font_serif),
    SANS_SERIF("sans-serif", R.string.font_sans_serif),
    MONOSPACE("monospace", R.string.font_monospace),
    HANDWRITING("cursive", R.string.font_handwriting),
}

/** Alignment of the lines in a text box. Values match `jni_bridge/settings.rs`. */
enum class TextAlign(val nativeValue: Int, val label: Int) {
    START(0, R.string.align_start),
    CENTER(1, R.string.align_center),
    END(2, R.string.align_end),
    FILL(3, R.string.align_fill),
}

/** How text looks. [fontSize] is in document units. */
data class TextFormat(
    val fontFamily: FontFamily = FontFamily.SERIF,
    val fontSize: Double = 32.0,
    val bold: Boolean = false,
    val italic: Boolean = false,
    val alignment: TextAlign = TextAlign.START,
) {
    companion object {
        val SIZES = listOf(12.0, 16.0, 24.0, 32.0, 48.0, 64.0)
    }
}

/** How shapes are drawn; [fill] is an `android.graphics.Color` int, or null for none. */
data class ShapeStyle(val rough: Boolean = false, val fill: Int? = null)

/** How the document grows. Values match `layout_from_int` in `jni_bridge/common.rs`. */
enum class DocumentLayout(val nativeValue: Int, val label: Int) {
    FIXED_SIZE(0, R.string.layout_fixed_size),
    CONTINUOUS_VERTICAL(1, R.string.layout_continuous_vertical),
    SEMI_INFINITE(2, R.string.layout_semi_infinite),
    INFINITE(3, R.string.layout_infinite),
}

/** Background patterns. Values are `PatternStyle` discriminants in rnote-engine. */
enum class Pattern(val nativeValue: Int, val label: Int) {
    NONE(0, R.string.pattern_none),
    LINES(1, R.string.pattern_lines),
    GRID(2, R.string.pattern_grid),
    DOTS(3, R.string.pattern_dots),
    ISOMETRIC_GRID(4, R.string.pattern_isometric_grid),
    ISOMETRIC_DOTS(5, R.string.pattern_isometric_dots),
}

/** Standard page sizes, portrait, in millimeters. */
enum class PageSize(val widthMm: Double, val heightMm: Double, val label: Int) {
    A3(297.0, 420.0, R.string.page_a3),
    A4(210.0, 297.0, R.string.page_a4),
    A5(148.0, 210.0, R.string.page_a5),
    LETTER(215.9, 279.4, R.string.page_letter),
    LEGAL(215.9, 355.6, R.string.page_legal),
    ;

    /** Whether a page of [widthMm] x [heightMm] is this size, in either orientation. */
    fun matches(widthMm: Double, heightMm: Double): Boolean {
        val portrait = minOf(widthMm, heightMm) to maxOf(widthMm, heightMm)
        return abs(portrait.first - this.widthMm) < TOLERANCE_MM && abs(portrait.second - this.heightMm) < TOLERANCE_MM
    }

    private companion object {
        const val TOLERANCE_MM = 1.0
    }
}

/** Background colors offered by the document dialog, with a pattern color that suits each. */
enum class BackgroundColor(val color: Int, val label: Int) {
    WHITE(Color.WHITE, R.string.background_white),
    CREAM(Color.rgb(0xFB, 0xF6, 0xE9), R.string.background_cream),
    LIGHT_GRAY(Color.rgb(0xEE, 0xEE, 0xEE), R.string.background_light_gray),
    DARK(Color.rgb(0x28, 0x2A, 0x36), R.string.background_dark),
    BLACK(Color.BLACK, R.string.background_black),
    ;

    val patternColor: Int
        get() = if (BrushPalette.contrastingColor(color) == Color.WHITE) DARK_PATTERN else LIGHT_PATTERN

    private companion object {
        val LIGHT_PATTERN = Color.rgb(0xC8, 0xCC, 0xD6)
        val DARK_PATTERN = Color.rgb(0x62, 0x72, 0xA4)
    }
}

/** The document's page size, layout and background, as the native side reports them. */
data class DocumentSettings(
    val pageWidthMm: Double,
    val pageHeightMm: Double,
    val layout: DocumentLayout,
    val backgroundColor: Int,
    val pattern: Pattern,
    val patternColor: Int,
    val showBorders: Boolean,
    /** Spacing of the background pattern, in document units. */
    val patternSize: Double,
) {
    val landscape: Boolean
        get() = pageWidthMm > pageHeightMm

    companion object {
        /** Decodes the array from `documentSettings` in `jni_bridge/document.rs`. */
        fun fromArray(values: DoubleArray) = DocumentSettings(
            pageWidthMm = values[0],
            pageHeightMm = values[1],
            layout = DocumentLayout.entries.first { it.nativeValue == values[2].toInt() },
            backgroundColor = values[3].toInt(),
            pattern = Pattern.entries.first { it.nativeValue == values[4].toInt() },
            patternColor = values[5].toInt(),
            showBorders = values[6] != 0.0,
            patternSize = values[7],
        )
    }
}
