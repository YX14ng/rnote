package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.TextWatcher
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.GridLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import java.util.Locale

/**
 * The mini color menu of a palette color, like other note-taking apps': a saturation and
 * value square, hue and opacity bars, the hex code and opacity, a preview with `+` to keep
 * the color, and the kept colors. Every change goes to [onChange] at once.
 */
internal class ColorPanel(
    private val context: Context,
    start: Int,
    private var saved: List<Int>,
    private val onChange: (Int) -> Unit,
    /** The kept colors changed: one was added or removed. */
    private val onSavedChange: (List<Int>) -> Unit,
) {
    private var hsva = Hsva.of(start)
    private var updatingFields = false

    private val square = SaturationValueField(context) { s, v -> update(hsva.copy(saturation = s, value = v)) }
    private val hueBar = HueBar(context) { hue -> update(hsva.copy(hue = hue)) }
    private val alphaBar = AlphaBar(context) { alpha -> update(hsva.copy(alpha = alpha)) }
    private val hex = field(InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_CAP_CHARACTERS, HEX_DIGITS) { text ->
        val rgb = text.toIntOrNull(16)
        if (text.length == HEX_DIGITS && rgb != null) {
            val color = (rgb and 0xFFFFFF) or ((hsva.alpha * 255).toInt() shl 24)
            update(Hsva.of(color).copy(alpha = hsva.alpha), fromText = true)
        }
    }
    private val opacity = field(InputType.TYPE_CLASS_NUMBER, 3) { text ->
        text.toIntOrNull()?.takeIf { it in 0..100 }?.let { update(hsva.copy(alpha = it / 100f), fromText = true) }
    }
    private val preview = View(context)
    private val savedGrid = GridLayout(context).apply { columnCount = SAVED_COLUMNS }

    /** The panel's view. */
    val view: View = PanelViews.column(context).apply {
        addView(square, LinearLayout.LayoutParams(MATCH_PARENT, dp(200f)))
        addView(hueBar, LinearLayout.LayoutParams(MATCH_PARENT, dp(34f)).apply { topMargin = dp(6f) })
        addView(alphaBar, LinearLayout.LayoutParams(MATCH_PARENT, dp(34f)))
        addView(valuesRow(), LinearLayout.LayoutParams(MATCH_PARENT, dp(48f)).apply { topMargin = dp(8f) })
        addView(PanelViews.divider(context), PanelViews.dividerParams(context))
        addView(savedGrid, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))
    }

    init {
        showSaved()
        update(hsva, notify = false)
    }

    private fun update(new: Hsva, fromText: Boolean = false, notify: Boolean = true) {
        hsva = new
        val color = new.toColor()
        square.hue = new.hue
        square.saturation = new.saturation
        square.value = new.value
        hueBar.hue = new.hue
        alphaBar.color = color
        preview.background = Graphite.rounded(context, color, 10f, Graphite.OUTLINE)
        if (!fromText) {
            updatingFields = true
            hex.setText(String.format(Locale.ROOT, "%06X", color and 0xFFFFFF))
            opacity.setText((new.alpha * 100).toInt().toString())
            updatingFields = false
        }
        if (notify) onChange(color)
    }

    private fun valuesRow() = LinearLayout(context).apply {
        gravity = Gravity.CENTER_VERTICAL
        addView(TextView(context).apply {
            text = context.getString(R.string.color_hex)
            setTextColor(Graphite.TEXT_SECONDARY)
            textSize = 14f
        }, LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply { marginEnd = dp(8f) })
        addView(hex, LinearLayout.LayoutParams(0, MATCH_PARENT, 1.4f))
        addView(opacity, LinearLayout.LayoutParams(0, MATCH_PARENT, 0.8f).apply { marginStart = dp(6f) })
        addView(TextView(context).apply { text = "%"; setTextColor(Graphite.TEXT_SECONDARY) },
            LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply { marginStart = dp(4f) })
        val keep = android.widget.FrameLayout(context).apply {
            addView(preview, android.widget.FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
            addView(ImageView(context).apply {
                setImageResource(R.drawable.ic_add)
                imageTintList = ColorStateList.valueOf(Color.WHITE)
                background = GradientDrawable().apply { shape = GradientDrawable.OVAL; setColor(Color.argb(0x66, 0, 0, 0)) }
                val pad = dp(3f)
                setPadding(pad, pad, pad, pad)
            }, android.widget.FrameLayout.LayoutParams(dp(26f), dp(26f), Gravity.CENTER))
            foreground = Graphite.pressable(context, null, 10f)
            isClickable = true
            contentDescription = context.getString(R.string.color_keep)
            tooltipText = contentDescription
            setOnClickListener { keep(hsva.toColor()) }
        }
        addView(keep, LinearLayout.LayoutParams(dp(84f), MATCH_PARENT).apply { marginStart = dp(10f) })
    }

    /** A text field that calls [onEdit] with its trimmed text as the user types. */
    private fun field(inputType: Int, maxLength: Int, onEdit: (String) -> Unit) = EditText(context).apply {
        this.inputType = inputType
        filters = arrayOf(InputFilter.LengthFilter(maxLength))
        isSingleLine = true
        imeOptions = EditorInfo.IME_ACTION_DONE
        gravity = Gravity.CENTER
        textSize = 15f
        setTextColor(Graphite.TEXT)
        background = Graphite.rounded(context, Graphite.RAISED, 10f)
        addTextChangedListener(object : TextWatcher {
            override fun afterTextChanged(text: Editable?) {
                if (!updatingFields) onEdit(text.toString().trim())
            }

            override fun beforeTextChanged(text: CharSequence?, start: Int, count: Int, after: Int) {}

            override fun onTextChanged(text: CharSequence?, start: Int, before: Int, count: Int) {}
        })
    }

    private fun keep(color: Int) {
        saved = (listOf(color) + saved.filter { it != color }).take(MAX_SAVED)
        onSavedChange(saved)
        showSaved()
    }

    /** The kept colors: tap to use one, long-press to forget it. */
    private fun showSaved() {
        savedGrid.removeAllViews()
        val size = dp(40f)
        val gap = dp(5f)
        for (color in saved) {
            savedGrid.addView(View(context).apply {
                background = GradientDrawable().apply {
                    cornerRadius = dp(10f).toFloat()
                    setColor(color)
                    setStroke(dp(1f), Graphite.OUTLINE)
                }
                foreground = Graphite.pressable(context, null, 10f)
                contentDescription = String.format(Locale.ROOT, "#%08X", color)
                setOnClickListener { update(Hsva.of(color)) }
                setOnLongClickListener {
                    saved = saved.filter { it != color }
                    onSavedChange(saved)
                    showSaved()
                    true
                }
            }, GridLayout.LayoutParams().apply {
                width = size
                height = size
                setMargins(gap, gap, gap, gap)
            })
        }
    }

    private fun dp(value: Float): Int = Graphite.dp(context, value)

    companion object {
        const val PANEL_WIDTH_DP = 340f
        private const val HEX_DIGITS = 6
        private const val SAVED_COLUMNS = 6
        private const val MAX_SAVED = 12
    }
}
