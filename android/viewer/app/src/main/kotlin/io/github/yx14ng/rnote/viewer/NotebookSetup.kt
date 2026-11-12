package io.github.yx14ng.rnote.viewer

import android.graphics.Color
import org.json.JSONObject

/** How the pages of a new notebook grow. [json] is the name in `notebook_setup.rs`. */
enum class SetupLayout(val json: String, val label: Int) {
    FIXED_SIZE("fixed_size", R.string.new_notebook_sheets),
    CONTINUOUS_VERTICAL("continuous_vertical", R.string.new_notebook_continuous),
    INFINITE("infinite", R.string.new_notebook_infinite),
}

/** What is drawn on the paper. [json] is the name in `notebook_setup.rs`. */
enum class Template(val json: String, val label: Int) {
    BLANK("none", R.string.template_blank),
    LINES("lines", R.string.template_lines),
    GRID("grid", R.string.template_grid),
    DOTS("dots", R.string.template_dots),
}

/** The colors a notebook cover can have, to tell subjects apart at a glance. */
enum class CoverColor(val argb: Int, val label: Int) {
    BLUE(Color.rgb(0x3D, 0x6F, 0xB6), R.string.cover_blue),
    GREEN(Color.rgb(0x3F, 0x8F, 0x6B), R.string.cover_green),
    ORANGE(Color.rgb(0xC9, 0x77, 0x3A), R.string.cover_orange),
    RED(Color.rgb(0xB5, 0x52, 0x4E), R.string.cover_red),
    VIOLET(Color.rgb(0x7A, 0x5B, 0xB0), R.string.cover_violet),
    TURQUOISE(Color.rgb(0x2F, 0x8C, 0x93), R.string.cover_turquoise),
    MUSTARD(Color.rgb(0xB8, 0x92, 0x2E), R.string.cover_mustard),
    GRAPHITE(Color.rgb(0x5E, 0x64, 0x70), R.string.cover_graphite),
}

/** The line spacings offered, in millimeters. */
val SPACINGS_MM = listOf(4.0, 5.0, 6.0, 8.0, 10.0)

/**
 * What a new notebook starts with: the JSON `NotebookSetup` of `notebook_setup.rs`. Colors
 * are ARGB.
 */
data class NotebookSetup(
    val pageWidthMm: Double,
    val pageHeightMm: Double,
    val layout: SetupLayout,
    val backgroundArgb: Int,
    val template: Template,
    val patternArgb: Int,
    val patternSizeMm: Double,
    val cover: Int?,
) {
    val landscape: Boolean
        get() = pageWidthMm > pageHeightMm

    /** The standard size of the pages, or null for another size. */
    val pageSize: PageSize?
        get() = PageSize.entries.firstOrNull { it.matches(pageWidthMm, pageHeightMm) }

    /** The same setup with [size] in [landscape] or portrait. */
    fun withPageSize(size: PageSize, landscape: Boolean): NotebookSetup =
        if (landscape) copy(pageWidthMm = size.heightMm, pageHeightMm = size.widthMm)
        else copy(pageWidthMm = size.widthMm, pageHeightMm = size.heightMm)

    /** The same setup on [paper], with the template color that suits it. */
    fun withPaper(paper: BackgroundColor): NotebookSetup =
        copy(backgroundArgb = paper.color, patternArgb = paper.patternColor)

    fun toJson(): String = JSONObject()
        .put("page_width_mm", pageWidthMm)
        .put("page_height_mm", pageHeightMm)
        .put("layout", layout.json)
        .put("background_argb", unsigned(backgroundArgb))
        .put("pattern", template.json)
        .put("pattern_argb", unsigned(patternArgb))
        .put("pattern_size_mm", patternSizeMm)
        .put("cover", cover?.let(::unsigned) ?: JSONObject.NULL)
        .toString()

    companion object {
        fun fromJson(json: String): NotebookSetup {
            val value = JSONObject(json)
            return NotebookSetup(
                pageWidthMm = value.getDouble("page_width_mm"),
                pageHeightMm = value.getDouble("page_height_mm"),
                layout = SetupLayout.entries.first { it.json == value.getString("layout") },
                backgroundArgb = value.getLong("background_argb").toInt(),
                template = Template.entries.first { it.json == value.getString("pattern") },
                patternArgb = value.getLong("pattern_argb").toInt(),
                patternSizeMm = value.getDouble("pattern_size_mm"),
                cover = if (value.isNull("cover")) null else value.getLong("cover").toInt(),
            )
        }

        /** Colors travel as unsigned ARGB, as the native side reads them. */
        fun unsigned(color: Int): Long = color.toLong() and 0xFFFFFFFFL
    }
}
