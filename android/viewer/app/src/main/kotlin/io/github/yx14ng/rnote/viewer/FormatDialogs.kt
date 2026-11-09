package io.github.yx14ng.rnote.viewer

import android.content.Context
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

    fun showShapeStyle(context: Context, current: ShapeStyle, onApply: (ShapeStyle) -> Unit) {
        val form = FormDialog(context, R.string.title_shape_style)
        val style = form.addChoice(
            R.string.label_shape_style,
            form.labels(listOf(R.string.shape_style_smooth, R.string.shape_style_rough)),
            if (current.rough) 1 else 0,
        )
        // "No fill" first, then the brush colors.
        val fills = listOf<Int?>(null) + BrushPalette.colors.map { it.value }
        val fill = form.addChoice(
            R.string.label_fill,
            form.labels(listOf(R.string.fill_none) + BrushPalette.colors.map { it.label }),
            fills.indexOf(current.fill).coerceAtLeast(0),
        )
        form.show { onApply(ShapeStyle(rough = style() == 1, fill = fills[fill()])) }
    }

    /**
     * Shows the page, layout and background settings. [onApply] gets one operation with only
     * what the user changed, or nothing if nothing changed.
     */
    fun showDocument(
        context: Context,
        current: DocumentSettings,
        onApply: ((NativeDocument) -> EditState) -> Unit,
    ) {
        val form = FormDialog(context, R.string.title_document)

        // A page size that matches no preset is kept as "Custom".
        val currentSize = PageSize.entries.firstOrNull { it.matches(current.pageWidthMm, current.pageHeightMm) }
        val sizeOptions = PageSize.entries.map { context.getString(it.label) } +
            context.getString(R.string.page_custom, current.pageWidthMm, current.pageHeightMm)
        val size = form.addChoice(
            R.string.label_page_size,
            sizeOptions,
            currentSize?.ordinal ?: PageSize.entries.size,
        )
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
        // A background color that matches no preset is kept as "Current".
        val currentColor = BackgroundColor.entries.firstOrNull { it.color == current.backgroundColor }
        val colorOptions = form.labels(BackgroundColor.entries.map { it.label }) +
            context.getString(R.string.background_current)
        val color = form.addChoice(
            R.string.label_background,
            colorOptions,
            currentColor?.ordinal ?: BackgroundColor.entries.size,
        )
        val pattern = form.addChoice(
            R.string.label_pattern,
            form.labels(Pattern.entries.map { it.label }),
            current.pattern.ordinal,
        )
        val borders = form.addCheck(R.string.label_page_borders, current.showBorders)

        form.show {
            val changes = mutableListOf<(NativeDocument) -> EditState>()

            val pickedSize = PageSize.entries.getOrNull(size())
            val landscape = orientation() == 1
            if (pickedSize != currentSize || landscape != current.landscape) {
                val (shortSide, longSide) = if (pickedSize != null) {
                    pickedSize.widthMm to pickedSize.heightMm
                } else {
                    minOf(current.pageWidthMm, current.pageHeightMm) to maxOf(current.pageWidthMm, current.pageHeightMm)
                }
                val (width, height) = if (landscape) longSide to shortSide else shortSide to longSide
                changes += { it.setPageSize(width, height) }
            }

            val pickedLayout = DocumentLayout.entries[layout()]
            if (pickedLayout != current.layout) {
                changes += { it.setLayout(pickedLayout) }
            }

            val pickedColor = BackgroundColor.entries.getOrNull(color())
            val pickedPattern = Pattern.entries[pattern()]
            if (pickedColor != currentColor || pickedPattern != current.pattern) {
                val backgroundColor = pickedColor?.color ?: current.backgroundColor
                val patternColor = pickedColor?.patternColor ?: current.patternColor
                changes += { it.setBackground(backgroundColor, pickedPattern, patternColor) }
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
}
