package io.github.yx14ng.rnote.viewer

import android.app.AlertDialog
import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.util.TypedValue
import android.view.View
import android.widget.EditText
import android.widget.GridLayout
import android.widget.LinearLayout
import android.widget.Toast

/** Picks a color: from a grid of suggested colors, or typed in hex for any other. */
internal object ColorPicker {
    /** Suggested colors, a row per hue family from dark to light, then grays. */
    private val SUGGESTED = listOf(
        0xFF000000, 0xFF3D3846, 0xFF77767B, 0xFFC0BFBC, 0xFFFFFFFF, 0xFF865E3C,
        0xFFA51D2D, 0xFFE01B24, 0xFFF66151, 0xFFC64600, 0xFFFF7800, 0xFFFFA348,
        0xFFE5A50A, 0xFFF5C211, 0xFFF9F06B, 0xFF26A269, 0xFF2EC27E, 0xFF8FF0A4,
        0xFF1A5FB4, 0xFF1C71D8, 0xFF62A0EA, 0xFF613583, 0xFF9141AC, 0xFFDC8ADD,
    ).map { it.toInt() }

    private const val COLUMNS = 6
    private const val SWATCH_DP = 40f
    private const val SPACING_DP = 6f
    private const val PADDING_DP = 20f

    /** Shows the picker starting at [current]; [onPick] gets the chosen color. */
    fun show(context: Context, title: Int, current: Int, onPick: (Int) -> Unit) {
        fun dp(value: Float) =
            TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, value, context.resources.displayMetrics).toInt()

        val preview = View(context)
        val hex = EditText(context).apply {
            inputType = InputType.TYPE_CLASS_TEXT
            isSingleLine = true
            setText(FormValues.formatColor(current))
        }
        fun showPreview(color: Int) {
            preview.background = GradientDrawable().apply {
                setColor(color)
                cornerRadius = dp(8f).toFloat()
                setStroke(dp(1f), Color.GRAY)
            }
        }
        showPreview(current)
        hex.addTextChangedListener(object : TextWatcher {
            override fun afterTextChanged(text: Editable?) {
                FormValues.parseColor(text.toString().trim())?.let(::showPreview)
            }

            override fun beforeTextChanged(text: CharSequence?, start: Int, count: Int, after: Int) {}

            override fun onTextChanged(text: CharSequence?, start: Int, before: Int, count: Int) {}
        })

        val grid = GridLayout(context).apply { columnCount = COLUMNS }
        for (color in SUGGESTED) {
            val swatch = View(context).apply {
                background = GradientDrawable().apply {
                    shape = GradientDrawable.OVAL
                    setColor(color)
                    setStroke(dp(1f), Color.GRAY)
                }
                contentDescription = FormValues.formatColor(color)
                setOnClickListener { hex.setText(FormValues.formatColor(color)) }
            }
            grid.addView(
                swatch,
                GridLayout.LayoutParams().apply {
                    width = dp(SWATCH_DP)
                    height = dp(SWATCH_DP)
                    setMargins(dp(SPACING_DP), dp(SPACING_DP), dp(SPACING_DP), dp(SPACING_DP))
                },
            )
        }

        val hexRow = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            addView(preview, LinearLayout.LayoutParams(dp(SWATCH_DP), dp(SWATCH_DP)).apply { marginEnd = dp(12f) })
            addView(hex, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
        }
        val content = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val padding = dp(PADDING_DP)
            setPadding(padding, padding / 2, padding, 0)
            addView(grid)
            addView(hexRow, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT).apply { topMargin = dp(12f) })
        }

        AlertDialog.Builder(context)
            .setTitle(title)
            .setView(content)
            .setPositiveButton(R.string.apply) { _, _ ->
                val color = FormValues.parseColor(hex.text.toString().trim())
                if (color == null) {
                    Toast.makeText(context, R.string.invalid_value, Toast.LENGTH_LONG).show()
                } else {
                    onPick(color)
                }
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }
}
