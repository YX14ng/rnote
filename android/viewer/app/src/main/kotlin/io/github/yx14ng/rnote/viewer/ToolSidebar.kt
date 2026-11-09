package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout

/**
 * The tools as a column of icons floating beside the canvas, like the desktop app's pen
 * picker, with the desktop icons in the same order, and below them a palette of colors.
 *
 * Tapping a tool calls [onToolSelected], and [selectedTool] marks the active one. Tapping a
 * color calls [onColorSelected] with its slot; long-pressing it calls [onColorEdit].
 */
internal class ToolSidebar(context: Context) : LinearLayout(context) {
    var onToolSelected: ((Tool) -> Unit)? = null
    var onColorSelected: ((Int) -> Unit)? = null
    var onColorEdit: ((Int) -> Unit)? = null

    private val buttons = TOOLS.associate { (tool, icon, label) -> tool to toolButton(tool, icon, label) }

    private val swatches: List<View> =
        List(PALETTE_SIZE) { slot ->
            View(context).apply {
                contentDescription = context.getString(R.string.palette_color, slot + 1)
                tooltipText = contentDescription
                setOnClickListener { onColorSelected?.invoke(slot) }
                setOnLongClickListener {
                    onColorEdit?.invoke(slot)
                    true
                }
            }
        }

    var selectedTool: Tool = Tool.BRUSH
        set(value) {
            field = value
            for ((tool, button) in buttons) {
                val selected = tool == value
                button.isSelected = selected
                button.background = if (selected) highlight() else ripple()
                button.imageTintList = ColorStateList.valueOf(if (selected) Color.WHITE else ICON_COLOR)
            }
        }

    init {
        orientation = VERTICAL
        gravity = Gravity.CENTER_HORIZONTAL
        val padding = dp(PADDING_DP)
        setPadding(padding, padding, padding, padding)
        background = GradientDrawable().apply {
            setColor(BAR_COLOR)
            cornerRadius = dp(BAR_RADIUS_DP).toFloat()
        }
        elevation = dp(ELEVATION_DP).toFloat()
        for (button in buttons.values) {
            addView(button, LayoutParams(dp(BUTTON_DP), dp(BUTTON_DP)))
        }
        addView(
            View(context).apply { setBackgroundColor(DIVIDER_COLOR) },
            LayoutParams(dp(DIVIDER_WIDTH_DP), dp(1f)).apply { setMargins(0, dp(6f), 0, dp(6f)) },
        )
        val swatchMargin = dp((SWATCH_CELL_DP - SWATCH_DP) / 2)
        for (swatch in swatches) {
            addView(
                swatch,
                LayoutParams(dp(SWATCH_DP), dp(SWATCH_DP)).apply {
                    setMargins(swatchMargin, swatchMargin, swatchMargin, swatchMargin)
                },
            )
        }
        selectedTool = Tool.BRUSH
    }

    /** Shows the palette's [colors], ringing the one that is [current], if any. */
    fun showPalette(colors: List<Int>, current: Int) {
        swatches.forEachIndexed { slot, swatch ->
            val color = colors.getOrElse(slot) { Color.BLACK }
            val selected = color == current
            swatch.background = GradientDrawable().apply {
                shape = GradientDrawable.OVAL
                setColor(color)
                // A light ring marks the current color; a faint one keeps dark colors visible.
                if (selected) setStroke(dp(3f), Color.WHITE) else setStroke(dp(1f), SWATCH_OUTLINE)
            }
            swatch.isSelected = selected
        }
    }

    private fun toolButton(tool: Tool, icon: Int, label: Int) = ImageButton(context).apply {
        setImageResource(icon)
        contentDescription = context.getString(label)
        tooltipText = contentDescription
        scaleType = ImageView.ScaleType.CENTER
        setOnClickListener { onToolSelected?.invoke(tool) }
    }

    private fun highlight() = GradientDrawable().apply {
        setColor(HIGHLIGHT_COLOR)
        cornerRadius = dp(HIGHLIGHT_RADIUS_DP).toFloat()
    }

    /** The theme's touch feedback, without a resting background. */
    private fun ripple(): Drawable? {
        val value = TypedValue()
        context.theme.resolveAttribute(android.R.attr.selectableItemBackgroundBorderless, value, true)
        return context.getDrawable(value.resourceId)
    }

    private fun dp(value: Float): Int =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, value, resources.displayMetrics).toInt()

    private companion object {
        /** Tool, icon and label, in the desktop app's order. */
        val TOOLS = listOf(
            Triple(Tool.BRUSH, R.drawable.ic_pen_brush, R.string.action_pen),
            Triple(Tool.SHAPER, R.drawable.ic_pen_shaper, R.string.action_shapes),
            Triple(Tool.TYPEWRITER, R.drawable.ic_pen_typewriter, R.string.action_text),
            Triple(Tool.ERASER, R.drawable.ic_pen_eraser, R.string.action_eraser),
            Triple(Tool.SELECTOR, R.drawable.ic_pen_lasso, R.string.action_select),
        )

        val BAR_COLOR = Color.argb(0xEB, 0x30, 0x30, 0x35)
        val HIGHLIGHT_COLOR = Color.argb(0xFF, 0x35, 0x84, 0xE4)
        val ICON_COLOR = Color.argb(0xD9, 0xFF, 0xFF, 0xFF)
        val DIVIDER_COLOR = Color.argb(0x55, 0xFF, 0xFF, 0xFF)
        val SWATCH_OUTLINE = Color.argb(0x80, 0xFF, 0xFF, 0xFF)

        /** How many colors the palette holds. */
        const val PALETTE_SIZE = 5
        const val SWATCH_CELL_DP = 44f
        const val SWATCH_DP = 32f
        const val DIVIDER_WIDTH_DP = 32f

        const val PADDING_DP = 6f
        const val BUTTON_DP = 52f
        const val BAR_RADIUS_DP = 16f
        const val HIGHLIGHT_RADIUS_DP = 12f
        const val ELEVATION_DP = 6f
    }
}
