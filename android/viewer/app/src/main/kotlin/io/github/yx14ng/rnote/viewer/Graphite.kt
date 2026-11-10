package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.RippleDrawable
import android.util.TypedValue
import android.view.Gravity
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.TextView

/**
 * The "Graphite" look shared by the editor's floating panels and bars: dark neutral chrome,
 * so the page stands out, and one blue accent. Colors mirror `res/values/colors.xml`.
 */
internal object Graphite {
    val BAR = Color.rgb(0x17, 0x18, 0x1C)
    val RAIL = Color.argb(0xF0, 0x17, 0x18, 0x1C)
    val SURFACE = Color.rgb(0x1B, 0x1C, 0x21)
    val RAISED = Color.rgb(0x23, 0x25, 0x2B)
    val OUTLINE = Color.rgb(0x30, 0x32, 0x3A)
    val DIVIDER = Color.rgb(0x2C, 0x2E, 0x34)
    val TEXT = Color.rgb(0xEC, 0xED, 0xEF)
    val TEXT_SECONDARY = Color.rgb(0xA9, 0xAC, 0xB5)
    val ICON = Color.rgb(0xC9, 0xCB, 0xD2)
    val ACCENT = Color.rgb(0x35, 0x84, 0xE4)
    val DANGER = Color.rgb(0xFF, 0x8A, 0x84)
    val RIPPLE = Color.argb(0x33, 0xFF, 0xFF, 0xFF)

    const val PANEL_RADIUS_DP = 22f
    const val BUTTON_RADIUS_DP = 14f

    fun dp(context: Context, value: Float): Int =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, value, context.resources.displayMetrics).toInt()

    /** A rounded rectangle in [color], with an optional outline. */
    fun rounded(context: Context, color: Int, radiusDp: Float, outline: Int? = null): GradientDrawable =
        GradientDrawable().apply {
            setColor(color)
            cornerRadius = dp(context, radiusDp).toFloat()
            if (outline != null) setStroke(dp(context, 1f), outline)
        }

    /** The background of floating panels: surface color, outline, large radius. */
    fun panel(context: Context): Drawable = rounded(context, SURFACE, PANEL_RADIUS_DP, OUTLINE)

    /** [content] with a light ripple on touch, clipped to the same rounded shape. */
    fun pressable(context: Context, content: Drawable?, radiusDp: Float): Drawable =
        RippleDrawable(ColorStateList.valueOf(RIPPLE), content, rounded(context, Color.WHITE, radiusDp))

    /** A square icon button tinted for the dark chrome. */
    fun iconButton(context: Context, icon: Int, label: Int, onClick: () -> Unit): ImageButton =
        ImageButton(context).apply {
            setImageResource(icon)
            imageTintList = ColorStateList.valueOf(TEXT)
            scaleType = ImageView.ScaleType.CENTER
            contentDescription = context.getString(label)
            tooltipText = contentDescription
            background = pressable(context, null, BUTTON_RADIUS_DP)
            setOnClickListener { onClick() }
        }

    /** A small uppercase section title, like "FILE". */
    fun sectionTitle(context: Context, label: Int): TextView = TextView(context).apply {
        setText(label)
        isAllCaps = true
        letterSpacing = 0.04f
        textSize = 12f
        setTextColor(TEXT_SECONDARY)
        val pad = dp(context, 14f)
        setPadding(pad, dp(context, 10f), pad, dp(context, 4f))
        gravity = Gravity.START
    }
}
