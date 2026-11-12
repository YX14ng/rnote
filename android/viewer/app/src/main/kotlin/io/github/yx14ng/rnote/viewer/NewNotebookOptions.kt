package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.LayerDrawable
import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView

/**
 * The controls of the "new notebook" and "import PDF" screens, in the Graphite look of the
 * design: one-of-several rows (segmented, chips, color circles) and labeled rows.
 */
internal object NewNotebookOptions {
    /** The darker blue of chosen options and the main button. */
    val CHOSEN = Color.rgb(0x2B, 0x6C, 0xC4)
    /** The light blue of links and of the ring around a chosen color. */
    val LINK = Color.rgb(0x7F, 0xB2, 0xF0)

    /**
     * A row of [options] of which one is chosen, kept in a pill (`segmented`) or as separate
     * chips; [onPick] gets the index tapped. Returns a way to show another choice.
     */
    fun choices(
        context: Context,
        options: List<String>,
        selected: Int,
        segmented: Boolean,
        onPick: (Int) -> Unit,
    ): Pair<LinearLayout, (Int) -> Unit> {
        val row = LinearLayout(context).apply {
            if (segmented) {
                background = Graphite.rounded(context, Graphite.RAISED, 14f, Graphite.OUTLINE)
                val pad = dp(context, 4f)
                setPadding(pad, pad, pad, pad)
            }
        }
        val buttons = options.mapIndexed { i, label ->
            TextView(context).apply {
                text = label
                textSize = 15f
                gravity = Gravity.CENTER
                minWidth = dp(context, if (segmented) 0f else 72f)
                setPadding(dp(context, 18f), 0, dp(context, 18f), 0)
                isClickable = true
                setOnClickListener { onPick(i) }
            }
        }
        buttons.forEachIndexed { i, button ->
            row.addView(button, LinearLayout.LayoutParams(WRAP_CONTENT, dp(context, 44f)).apply {
                if (i > 0) marginStart = dp(context, if (segmented) 4f else 8f)
            })
        }
        val show = { chosen: Int ->
            buttons.forEachIndexed { i, button ->
                val on = i == chosen
                button.setTextColor(if (on) Color.WHITE else Graphite.TEXT)
                val radius = if (segmented) 10f else 12f
                val content = when {
                    on -> Graphite.rounded(context, CHOSEN, radius)
                    segmented -> null
                    else -> Graphite.rounded(context, Graphite.RAISED, radius, Graphite.OUTLINE)
                }
                button.background = Graphite.pressable(context, content, radius)
                button.isSelected = on
            }
        }
        show(selected)
        return row to show
    }

    /** A row of color circles, the one at [selected] ringed; [onPick] gets the index tapped. */
    fun swatches(
        context: Context,
        colors: List<Pair<Int, String>>,
        selected: Int?,
        onPick: (Int) -> Unit,
    ): Pair<LinearLayout, (Int?) -> Unit> {
        val row = LinearLayout(context)
        val circles = colors.mapIndexed { i, (color, label) ->
            View(context).apply {
                contentDescription = label
                tooltipText = label
                isClickable = true
                setOnClickListener { onPick(i) }
                tag = color
            }
        }
        circles.forEachIndexed { i, circle ->
            row.addView(circle, LinearLayout.LayoutParams(dp(context, 48f), dp(context, 48f)).apply {
                if (i > 0) marginStart = dp(context, 6f)
            })
        }
        val show = { chosen: Int? ->
            circles.forEachIndexed { i, circle -> circle.background = swatch(context, circle.tag as Int, i == chosen) }
        }
        show(selected)
        return row to show
    }

    /** A circle of [color] with a thin outline, and a light ring around it if [chosen]. */
    private fun swatch(context: Context, color: Int, chosen: Boolean): LayerDrawable {
        val ring = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(Color.TRANSPARENT)
            if (chosen) setStroke(dp(context, 2f), LINK)
        }
        val dot = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(color)
            setStroke(dp(context, 1f), Color.rgb(0x4A, 0x4D, 0x57))
        }
        val inset = dp(context, 4f)
        return LayerDrawable(arrayOf(ring, dot)).apply { setLayerInset(1, inset, inset, inset, inset) }
    }

    /** A row with [label] on the left and [content] on the right, over a divider. */
    fun row(context: Context, label: Int, content: View): LinearLayout = LinearLayout(context).apply {
        gravity = Gravity.CENTER_VERTICAL
        minimumHeight = dp(context, 52f)
        background = LayerDrawable(arrayOf(GradientDrawable().apply { setColor(Graphite.DIVIDER) }, GradientDrawable().apply {
            setColor(Graphite.RAISED)
        })).apply { setLayerInset(1, 0, 0, 0, dp(context, 1f)) }
        addView(TextView(context).apply {
            setText(label)
            textSize = 16f
            setTextColor(Graphite.TEXT)
        }, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
        addView(content, LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT))
    }

    /** A value with a drop-down arrow, e.g. "A4 · portrait"; tapping it runs [onClick]. */
    fun dropDown(context: Context, onClick: () -> Unit): TextView = TextView(context).apply {
        textSize = 16f
        setTextColor(Graphite.TEXT)
        maxLines = 1
        ellipsize = TextUtils.TruncateAt.END
        gravity = Gravity.CENTER_VERTICAL
        minHeight = dp(context, 44f)
        setPadding(dp(context, 10f), 0, dp(context, 6f), 0)
        setCompoundDrawablesRelativeWithIntrinsicBounds(0, 0, R.drawable.ic_chevron_down, 0)
        compoundDrawableTintList = ColorStateList.valueOf(Graphite.TEXT_SECONDARY)
        compoundDrawablePadding = dp(context, 6f)
        background = Graphite.pressable(context, null, 12f)
        isClickable = true
        setOnClickListener { onClick() }
    }

    /** A bold label above a group of options. */
    fun heading(context: Context, label: Int): TextView = TextView(context).apply {
        setText(label)
        textSize = 16f
        setTextColor(Graphite.TEXT)
        paint.isFakeBoldText = true
        minWidth = dp(context, 132f)
    }

    /** A rounded info box with an "i" icon and [text], at the foot of a screen. */
    fun hint(context: Context, text: Int): LinearLayout = LinearLayout(context).apply {
        gravity = Gravity.CENTER_VERTICAL
        background = Graphite.rounded(context, Graphite.RAISED, 14f)
        val pad = dp(context, 14f)
        setPadding(pad, pad, pad, pad)
        addView(FrameLayout(context).apply {
            addView(ImageView(context).apply {
                setImageResource(R.drawable.ic_info)
                imageTintList = ColorStateList.valueOf(Graphite.TEXT_SECONDARY)
            }, FrameLayout.LayoutParams(dp(context, 20f), dp(context, 20f)))
        })
        addView(TextView(context).apply {
            setText(text)
            textSize = 14f
            setTextColor(Graphite.TEXT_SECONDARY)
        }, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f).apply { marginStart = dp(context, 10f) })
    }

    /** A full-width vertical section with [padding] dp around it, on [color] if given. */
    fun column(context: Context, padding: Float, color: Int? = null): LinearLayout = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        val pad = dp(context, padding)
        setPadding(pad, pad, pad, pad)
        if (color != null) setBackgroundColor(color)
        layoutParams = LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT)
    }

    fun dp(context: Context, value: Float): Int = Graphite.dp(context, value)
}
