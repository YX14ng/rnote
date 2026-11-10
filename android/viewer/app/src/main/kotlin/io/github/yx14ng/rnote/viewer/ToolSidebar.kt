package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.LayerDrawable
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import java.util.Locale

/**
 * The tools as a column of icons floating beside the canvas, like the desktop app's pen
 * picker, with the desktop icons in the same order; below them a palette of colors and the
 * current brush width.
 *
 * Tapping a tool calls [onToolSelected], or [onToolOptions] when it is already the active
 * one. Tapping a color calls [onColorSelected] with its slot; long-pressing it calls
 * [onColorEdit]. The width button calls [onWidthTapped].
 */
internal class ToolSidebar(context: Context) : LinearLayout(context) {
    var onToolSelected: ((Tool) -> Unit)? = null
    var onToolOptions: ((Tool) -> Unit)? = null
    var onColorSelected: ((Int) -> Unit)? = null
    var onColorEdit: ((Int) -> Unit)? = null
    var onWidthTapped: (() -> Unit)? = null

    private val buttons = TOOLS.associate { (tool, icon, label) -> tool to toolButton(tool, icon, label) }

    private val swatches: List<View> = List(PALETTE_SIZE) { slot ->
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

    private val widthBar = View(context)
    private val widthLabel = TextView(context).apply {
        textSize = 11f
        setTextColor(Graphite.TEXT)
        gravity = Gravity.CENTER
    }
    private val widthButton = LinearLayout(context).apply {
        orientation = VERTICAL
        gravity = Gravity.CENTER
        background = Graphite.pressable(context, Graphite.rounded(context, Graphite.RAISED, TOOL_RADIUS_DP), TOOL_RADIUS_DP)
        contentDescription = context.getString(R.string.brush_width)
        tooltipText = contentDescription
        addView(widthBar, LayoutParams(dp(WIDTH_BAR_DP), dp(4f)))
        addView(widthLabel, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply { topMargin = dp(3f) })
        setOnClickListener { onWidthTapped?.invoke() }
    }

    var selectedTool: Tool = Tool.BRUSH
        set(value) {
            field = value
            for ((tool, button) in buttons) {
                val selected = tool == value
                button.isSelected = selected
                val resting = if (selected) Graphite.rounded(context, Graphite.ACCENT, TOOL_RADIUS_DP) else null
                button.background = Graphite.pressable(context, resting, TOOL_RADIUS_DP)
                button.imageTintList = ColorStateList.valueOf(if (selected) Color.WHITE else Graphite.ICON)
            }
        }

    init {
        orientation = VERTICAL
        gravity = Gravity.CENTER_HORIZONTAL
        val padding = dp(8f)
        setPadding(padding, padding, padding, padding)
        background = Graphite.rounded(context, Graphite.RAIL, RAIL_RADIUS_DP, Graphite.OUTLINE)
        elevation = dp(8f).toFloat()
        val toolSpacing = dp(2f)
        for (button in buttons.values) {
            addView(button, LayoutParams(dp(TOOL_DP), dp(TOOL_DP)).apply { setMargins(0, toolSpacing, 0, toolSpacing) })
        }
        addView(divider(), dividerParams())
        for (swatch in swatches) {
            val cell = dp(SWATCH_CELL_DP)
            addView(FrameLayout(context).apply { addView(swatch, FrameLayout.LayoutParams(cell, cell, Gravity.CENTER)) })
        }
        addView(divider(), dividerParams())
        addView(widthButton, LayoutParams(dp(TOOL_DP), dp(TOOL_DP)))
        selectedTool = Tool.BRUSH
    }

    /** Shows the palette's [colors], ringing the one that is [current], if any. */
    fun showPalette(colors: List<Int>, current: Int) {
        swatches.forEachIndexed { slot, swatch ->
            val color = colors.getOrElse(slot) { Color.BLACK }
            val selected = color == current
            swatch.isSelected = selected
            swatch.background = swatchDrawable(color, selected)
        }
    }

    /** Shows the current brush [width] (document units) on the width button. */
    fun showWidth(width: Double) {
        widthBar.background = Graphite.rounded(context, Graphite.TEXT, 2f)
        widthBar.layoutParams = widthBar.layoutParams.apply { height = dp((width.toFloat() * 0.8f).coerceIn(1.5f, 10f)) }
        widthLabel.text = formatWidth(width)
    }

    /** A dot in [color]; the current one gets a white ring, separated by the rail color. */
    private fun swatchDrawable(color: Int, selected: Boolean): LayerDrawable {
        fun oval(fill: Int) = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(fill)
        }
        val ring = if (selected) Color.WHITE else Graphite.OUTLINE
        val gap = if (selected) dp(2f) else dp(1f)
        return LayerDrawable(arrayOf(oval(ring), oval(Graphite.BAR), oval(color))).apply {
            setLayerInset(1, gap, gap, gap, gap)
            val inner = if (selected) dp(4f) else dp(1f)
            setLayerInset(2, inner, inner, inner, inner)
        }
    }

    private fun toolButton(tool: Tool, icon: Int, label: Int) = ImageButton(context).apply {
        setImageResource(icon)
        contentDescription = context.getString(label)
        tooltipText = contentDescription
        scaleType = ImageView.ScaleType.CENTER
        setOnClickListener {
            if (tool == selectedTool) onToolOptions?.invoke(tool) else onToolSelected?.invoke(tool)
        }
    }

    private fun divider() = View(context).apply { setBackgroundColor(DIVIDER_COLOR) }

    private fun dividerParams() = LayoutParams(dp(32f), dp(1f)).apply { setMargins(0, dp(8f), 0, dp(8f)) }

    private fun dp(value: Float): Int = Graphite.dp(context, value)

    companion object {
        /** Tool, icon and label, in the desktop app's order. */
        private val TOOLS = listOf(
            Triple(Tool.BRUSH, R.drawable.ic_pen_brush, R.string.action_pen),
            Triple(Tool.SHAPER, R.drawable.ic_pen_shaper, R.string.action_shapes),
            Triple(Tool.TYPEWRITER, R.drawable.ic_pen_typewriter, R.string.action_text),
            Triple(Tool.ERASER, R.drawable.ic_pen_eraser, R.string.action_eraser),
            Triple(Tool.SELECTOR, R.drawable.ic_pen_lasso, R.string.action_select),
        )

        private val DIVIDER_COLOR = Color.rgb(0x3A, 0x3D, 0x45)

        /** How many colors the palette holds. */
        private const val PALETTE_SIZE = 5
        private const val TOOL_DP = 48f
        private const val TOOL_RADIUS_DP = 16f
        private const val RAIL_RADIUS_DP = 24f
        private const val SWATCH_CELL_DP = 40f
        private const val WIDTH_BAR_DP = 22f

        /** A brush width for display: whole numbers as such, others with one decimal. */
        fun formatWidth(width: Double): String =
            if (width >= 10 || width == Math.rint(width)) {
                String.format(Locale.ROOT, "%.0f", width)
            } else {
                String.format(Locale.ROOT, "%.1f", width)
            }
    }
}
