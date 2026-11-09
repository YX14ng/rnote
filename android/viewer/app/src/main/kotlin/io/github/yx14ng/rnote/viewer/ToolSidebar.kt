package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.util.TypedValue
import android.view.Gravity
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout

/**
 * The tools as a column of icons floating beside the canvas, like the desktop app's pen
 * picker, with the desktop icons in the same order. Tapping one calls [onToolSelected];
 * [selectedTool] marks the active one.
 */
internal class ToolSidebar(context: Context) : LinearLayout(context) {
    var onToolSelected: ((Tool) -> Unit)? = null

    private val buttons = TOOLS.associate { (tool, icon, label) -> tool to toolButton(tool, icon, label) }

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
        selectedTool = Tool.BRUSH
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
            Triple(Tool.SELECTOR, R.drawable.ic_pen_selector, R.string.action_select),
        )

        val BAR_COLOR = Color.argb(0xEB, 0x30, 0x30, 0x35)
        val HIGHLIGHT_COLOR = Color.argb(0xFF, 0x35, 0x84, 0xE4)
        val ICON_COLOR = Color.argb(0xD9, 0xFF, 0xFF, 0xFF)

        const val PADDING_DP = 6f
        const val BUTTON_DP = 52f
        const val BAR_RADIUS_DP = 16f
        const val HIGHLIGHT_RADIUS_DP = 12f
        const val ELEVATION_DP = 6f
    }
}
