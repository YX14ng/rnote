package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.widget.Toast
import kotlin.math.abs

/** The text format, shape style and document dialogs. */
internal object FormatDialogs {
    fun showTextFormat(context: Context, current: TextFormat, onApply: (TextFormat) -> Unit) {
        val form = FormDialog(context, R.string.title_text_format)
        val family = form.addChoice(
            R.string.label_font,
            form.labels(FontFamily.entries.map { it.label }),
            current.fontFamily.ordinal,
        )
        val sizes = TextFormat.SIZES
        val closestSize = sizes.indices.minBy { abs(sizes[it] - current.fontSize) }
        val size = form.addChoice(R.string.label_text_size, sizes.map { it.toInt().toString() }, closestSize)
        val bold = form.addCheck(R.string.label_bold, current.bold)
        val italic = form.addCheck(R.string.label_italic, current.italic)
        val alignment = form.addChoice(
            R.string.label_alignment,
            form.labels(TextAlign.entries.map { it.label }),
            current.alignment.ordinal,
        )
        form.show {
            onApply(
                TextFormat(
                    fontFamily = FontFamily.entries[family()],
                    fontSize = sizes[size()],
                    bold = bold(),
                    italic = italic(),
                    alignment = TextAlign.entries[alignment()],
                ),
            )
        }
    }

    /**
     * Shows the page, layout and background settings. [onApply] gets one operation with only
     * what the user changed, or nothing if nothing changed or a typed value is invalid.
     */
    fun showDocument(
        context: Context,
        current: DocumentSettings,
        onApply: ((NativeDocument) -> EditState) -> Unit,
    ) {
        val form = FormDialog(context, R.string.title_document)

        // Sizes other than the presets are typed in, as are other background colors.
        val currentSize = PageSize.entries.firstOrNull { it.matches(current.pageWidthMm, current.pageHeightMm) }
        val customSizeIndex = PageSize.entries.size
        lateinit var customWidth: TextField
        lateinit var customHeight: TextField
        val size = form.addChoice(
            R.string.label_page_size,
            form.labels(PageSize.entries.map { it.label } + R.string.page_custom),
            currentSize?.ordinal ?: customSizeIndex,
        ) { index ->
            customWidth.visible = index == customSizeIndex
            customHeight.visible = index == customSizeIndex
        }
        customWidth = form.addNumber(R.string.label_page_width, current.pageWidthMm)
        customHeight = form.addNumber(R.string.label_page_height, current.pageHeightMm)
        val orientation = form.addChoice(
            R.string.label_orientation,
            form.labels(listOf(R.string.orientation_portrait, R.string.orientation_landscape)),
            if (current.landscape) 1 else 0,
        )
        val layout = form.addChoice(
            R.string.label_layout,
            form.labels(DocumentLayout.entries.map { it.label }),
            current.layout.ordinal,
        )
        val currentColor = BackgroundColor.entries.firstOrNull { it.color == current.backgroundColor }
        val customColorIndex = BackgroundColor.entries.size
        lateinit var customColor: TextField
        val color = form.addChoice(
            R.string.label_background,
            form.labels(BackgroundColor.entries.map { it.label } + R.string.background_custom),
            currentColor?.ordinal ?: customColorIndex,
        ) { index -> customColor.visible = index == customColorIndex }
        customColor = form.addText(R.string.label_background_hex, FormValues.formatColor(current.backgroundColor))
        val pattern = form.addChoice(
            R.string.label_pattern,
            form.labels(Pattern.entries.map { it.label }),
            current.pattern.ordinal,
        )
        val patternColor = form.addText(R.string.label_pattern_color, FormValues.formatColor(current.patternColor))
        val patternSize = form.addNumber(R.string.label_pattern_spacing, current.patternSize)
        val borders = form.addCheck(R.string.label_page_borders, current.showBorders)

        form.show {
            val changes = mutableListOf<(NativeDocument) -> EditState>()

            val pickedSize = PageSize.entries.getOrNull(size())
            val sides = if (pickedSize != null) {
                pickedSize.widthMm to pickedSize.heightMm
            } else {
                val width = FormValues.parseNumber(customWidth.text, PAGE_SIDE_RANGE_MM)
                val height = FormValues.parseNumber(customHeight.text, PAGE_SIDE_RANGE_MM)
                if (width == null || height == null) return@show invalid(context)
                width to height
            }
            val shortSide = minOf(sides.first, sides.second)
            val longSide = maxOf(sides.first, sides.second)
            val (width, height) = if (orientation() == 1) longSide to shortSide else shortSide to longSide
            if (abs(width - current.pageWidthMm) > SAME_SIZE_MM || abs(height - current.pageHeightMm) > SAME_SIZE_MM) {
                changes += { it.setPageSize(width, height) }
            }

            val pickedLayout = DocumentLayout.entries[layout()]
            if (pickedLayout != current.layout) {
                changes += { it.setLayout(pickedLayout) }
            }

            val pickedColor = BackgroundColor.entries.getOrNull(color())
            val backgroundColor = pickedColor?.color
                ?: FormValues.parseColor(customColor.text)
                ?: return@show invalid(context)
            // A typed pattern color wins; otherwise a preset background brings its own.
            val pickedPatternColor = when {
                patternColor.edited -> FormValues.parseColor(patternColor.text) ?: return@show invalid(context)
                pickedColor != null && pickedColor != currentColor -> pickedColor.patternColor
                else -> current.patternColor
            }
            val pickedPattern = Pattern.entries[pattern()]
            val pickedPatternSize = if (patternSize.edited) {
                FormValues.parseNumber(patternSize.text, PATTERN_SIZE_RANGE) ?: return@show invalid(context)
            } else {
                current.patternSize
            }
            if (backgroundColor != current.backgroundColor || pickedPattern != current.pattern ||
                pickedPatternColor != current.patternColor || pickedPatternSize != current.patternSize
            ) {
                changes += { it.setBackground(backgroundColor, pickedPattern, pickedPatternColor, pickedPatternSize) }
            }

            val showBorders = borders()
            if (showBorders != current.showBorders) {
                changes += { it.setPageBorders(showBorders) }
            }

            if (changes.isNotEmpty()) {
                onApply { document ->
                    val states = changes.map { change -> change(document) }
                    states.last().copy(boundsChanged = states.any { it.boundsChanged })
                }
            }
        }
    }

    private fun invalid(context: Context) {
        Toast.makeText(context, R.string.invalid_value, Toast.LENGTH_LONG).show()
    }

    /** Page sides accepted, in millimeters: from a label to a poster. */
    private val PAGE_SIDE_RANGE_MM = 20.0..2000.0

    /** Pattern spacing accepted, in document units; matches `PATTERN_SIZE_RANGE` natively. */
    private val PATTERN_SIZE_RANGE = 4.0..512.0

    /** Below this difference, in millimeters, the page size is considered unchanged. */
    private const val SAME_SIZE_MM = 0.05
}
