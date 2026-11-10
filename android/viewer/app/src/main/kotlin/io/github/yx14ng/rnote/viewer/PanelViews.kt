package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.FrameLayout
import android.widget.GridLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Switch
import android.widget.TextView

/**
 * The building blocks of the floating panels (tool options, menu, selection bar), in the
 * [Graphite] look. Every control is a real button or switch, so it is reachable by keyboard
 * and accessibility services.
 */
internal object PanelViews {
    /** A vertical panel with the floating panel background. */
    fun column(context: Context, widthDp: Float? = null): LinearLayout = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        background = Graphite.panel(context)
        val pad = Graphite.dp(context, 12f)
        setPadding(pad, pad, pad, pad)
        elevation = Graphite.dp(context, 12f).toFloat()
        // Taps on the panel itself must not reach the overlay, which closes it.
        isClickable = true
        widthDp?.let { minimumWidth = Graphite.dp(context, it) }
    }

    fun title(context: Context, label: Int): TextView = TextView(context).apply {
        setText(label)
        textSize = 18f
        setTextColor(Graphite.TEXT)
        typeface = android.graphics.Typeface.create("sans-serif-medium", android.graphics.Typeface.NORMAL)
        val pad = Graphite.dp(context, 8f)
        setPadding(pad, pad / 2, pad, pad)
    }

    fun label(context: Context, label: Int): TextView = TextView(context).apply {
        setText(label)
        textSize = 13f
        setTextColor(Graphite.TEXT_SECONDARY)
        val pad = Graphite.dp(context, 8f)
        setPadding(pad, pad, pad, pad / 2)
    }

    /** Options side by side, the [selected] one highlighted; [onPick] gets the index. */
    fun segmented(context: Context, options: List<CharSequence>, selected: Int, onPick: (Int) -> Unit): LinearLayout =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            background = Graphite.rounded(context, Graphite.RAISED, Graphite.BUTTON_RADIUS_DP)
            val pad = Graphite.dp(context, 4f)
            setPadding(pad, pad, pad, pad)
            options.forEachIndexed { index, option ->
                addView(choiceButton(context, option, index == selected) { onPick(index) },
                    LinearLayout.LayoutParams(0, Graphite.dp(context, 40f), 1f))
            }
        }

    /** Equal cells in [columns] columns; [onPick] gets the index of the tapped one. */
    fun grid(context: Context, options: List<CharSequence>, selected: Int, columns: Int, onPick: (Int) -> Unit): GridLayout =
        GridLayout(context).apply {
            columnCount = columns
            val gap = Graphite.dp(context, 3f)
            options.forEachIndexed { index, option ->
                val button = choiceButton(context, option, index == selected) { onPick(index) }
                button.background = Graphite.pressable(
                    context,
                    Graphite.rounded(context, if (index == selected) Graphite.ACCENT else Graphite.RAISED, 10f),
                    10f,
                )
                addView(button, GridLayout.LayoutParams(
                    GridLayout.spec(GridLayout.UNDEFINED),
                    GridLayout.spec(GridLayout.UNDEFINED, 1f),
                ).apply {
                    width = 0
                    height = Graphite.dp(context, 44f)
                    setMargins(gap, gap, gap, gap)
                })
            }
        }

    /** A labelled switch row; [hint] adds a second, smaller line. */
    @SuppressLint("UseSwitchCompatOrMaterialCode") // No AndroidX in this app.
    fun switchRow(context: Context, label: Int, hint: Int?, checked: Boolean, onToggle: () -> Unit): LinearLayout =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            minimumHeight = Graphite.dp(context, 52f)
            val pad = Graphite.dp(context, 8f)
            setPadding(pad, 0, pad, 0)
            val texts = LinearLayout(context).apply {
                orientation = LinearLayout.VERTICAL
                addView(TextView(context).apply { setText(label); textSize = 15f; setTextColor(Graphite.TEXT) })
                if (hint != null) {
                    addView(TextView(context).apply { setText(hint); textSize = 12f; setTextColor(Graphite.TEXT_SECONDARY) })
                }
            }
            addView(texts, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
            val switch = Switch(context).apply {
                isChecked = checked
                contentDescription = context.getString(label)
                thumbTintList = ColorStateList(
                    arrayOf(intArrayOf(android.R.attr.state_checked), intArrayOf()),
                    intArrayOf(Color.WHITE, Graphite.ICON),
                )
                trackTintList = ColorStateList(
                    arrayOf(intArrayOf(android.R.attr.state_checked), intArrayOf()),
                    intArrayOf(Graphite.ACCENT, Graphite.OUTLINE),
                )
                setOnCheckedChangeListener { _, _ -> onToggle() }
            }
            addView(switch)
        }

    /** A menu row: icon, label and an optional keyboard shortcut hint. */
    fun menuRow(context: Context, icon: Int, label: Int, shortcut: String?, enabled: Boolean, onClick: () -> Unit) =
        LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            minimumHeight = Graphite.dp(context, 48f)
            val pad = Graphite.dp(context, 14f)
            setPadding(pad, 0, pad, 0)
            background = Graphite.pressable(context, null, 12f)
            isEnabled = enabled
            alpha = if (enabled) 1f else DISABLED_ALPHA
            isClickable = true
            setOnClickListener { onClick() }
            addView(icon(context, icon, Graphite.TEXT), LinearLayout.LayoutParams(Graphite.dp(context, 20f), Graphite.dp(context, 20f)))
            addView(TextView(context).apply {
                setText(label)
                textSize = 15f
                setTextColor(Graphite.TEXT)
                setPadding(Graphite.dp(context, 14f), 0, 0, 0)
            }, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
            if (shortcut != null) {
                addView(TextView(context).apply { text = shortcut; textSize = 12f; setTextColor(Graphite.TEXT_SECONDARY) })
            }
        }

    /** A large button with an icon over a short label, e.g. "Copy". */
    fun bigButton(context: Context, icon: Int, label: Int, tint: Int = Graphite.TEXT, onClick: () -> Unit) =
        LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            minimumWidth = Graphite.dp(context, 64f)
            minimumHeight = Graphite.dp(context, 56f)
            val pad = Graphite.dp(context, 8f)
            setPadding(pad, pad / 2, pad, pad / 2)
            background = Graphite.pressable(context, null, 12f)
            isClickable = true
            contentDescription = context.getString(label)
            setOnClickListener { onClick() }
            addView(icon(context, icon, tint), LinearLayout.LayoutParams(Graphite.dp(context, 20f), Graphite.dp(context, 20f)))
            addView(TextView(context).apply { setText(label); textSize = 12f; setTextColor(tint) })
        }

    fun divider(context: Context): View = View(context).apply { setBackgroundColor(Graphite.DIVIDER) }

    fun dividerParams(context: Context) = LinearLayout.LayoutParams(MATCH_PARENT, Graphite.dp(context, 1f)).apply {
        val margin = Graphite.dp(context, 8f)
        setMargins(margin, margin, margin, margin)
    }

    private fun icon(context: Context, icon: Int, tint: Int) = ImageView(context).apply {
        setImageResource(icon)
        imageTintList = ColorStateList.valueOf(tint)
    }

    private fun choiceButton(context: Context, text: CharSequence, selected: Boolean, onClick: () -> Unit) =
        TextView(context).apply {
            this.text = text
            gravity = Gravity.CENTER
            textSize = 14f
            setTextColor(if (selected) Color.WHITE else Graphite.TEXT)
            background = Graphite.pressable(
                context,
                if (selected) Graphite.rounded(context, Graphite.ACCENT, 10f) else null,
                10f,
            )
            isClickable = true
            isSelected = selected
            setOnClickListener { onClick() }
        }

    private const val DISABLED_ALPHA = 0.4f
}

/**
 * Covers the screen with a floating [panel]; a tap outside the panel calls [onDismiss].
 * Tall panels scroll.
 */
@SuppressLint("ViewConstructor") // Built in code only, with what it shows.
internal class PanelOverlay(
    context: Context,
    panel: View,
    gravity: Int,
    marginEndDp: Float,
    marginTopDp: Float,
    private val onDismiss: () -> Unit,
) : FrameLayout(context) {
    init {
        isClickable = true
        setOnClickListener { onDismiss() }
        val scroller = ScrollView(context).apply {
            isVerticalScrollBarEnabled = false
            addView(panel)
        }
        val margin = Graphite.dp(context, 8f)
        addView(scroller, LayoutParams(WRAP_CONTENT, WRAP_CONTENT, gravity).apply {
            marginEnd = Graphite.dp(context, marginEndDp)
            topMargin = Graphite.dp(context, marginTopDp)
            bottomMargin = margin
        })
    }
}
