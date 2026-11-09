package io.github.yx14ng.rnote.viewer

import android.app.AlertDialog
import android.content.Context
import android.text.InputType
import android.util.TypedValue
import android.view.View
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Spinner
import android.widget.TextView

/**
 * A small settings form in a dialog, built from framework views: labeled choices, text
 * fields and checkboxes. Each `add` call returns a way to read the value the user ends up
 * with.
 */
internal class FormDialog(private val context: Context, private val title: Int) {
    private val fields = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        val padding = dp(PADDING_DP)
        setPadding(padding, padding / 2, padding, 0)
    }

    /**
     * A drop-down with [options], starting at [selected]. [onSelected] runs with the index
     * shown once the drop-down is laid out, and again on every change.
     */
    fun addChoice(
        label: Int,
        options: List<CharSequence>,
        selected: Int,
        onSelected: ((Int) -> Unit)? = null,
    ): () -> Int {
        fields.addView(labelView(label))
        val spinner = Spinner(context).apply {
            adapter = ArrayAdapter(context, android.R.layout.simple_spinner_dropdown_item, options)
            setSelection(selected.coerceIn(0, options.lastIndex))
        }
        if (onSelected != null) {
            spinner.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
                override fun onItemSelected(parent: AdapterView<*>?, view: View?, position: Int, id: Long) =
                    onSelected(position)

                override fun onNothingSelected(parent: AdapterView<*>?) {}
            }
        }
        fields.addView(spinner)
        return { spinner.selectedItemPosition }
    }

    /** A one-line text field starting with [value], e.g. for a color in hex. */
    fun addText(label: Int, value: String): TextField = addField(label, value, InputType.TYPE_CLASS_TEXT)

    /** A field for a decimal number starting with [value]. */
    fun addNumber(label: Int, value: Double): TextField = addField(
        label,
        FormValues.formatNumber(value),
        InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL,
    )

    fun addCheck(label: Int, checked: Boolean): () -> Boolean {
        val checkBox = CheckBox(context).apply {
            setText(label)
            isChecked = checked
        }
        fields.addView(checkBox)
        return { checkBox.isChecked }
    }

    /** Shows the form; [onApply] runs when the user confirms, reading the getters. */
    fun show(onApply: () -> Unit) {
        AlertDialog.Builder(context)
            .setTitle(title)
            .setView(ScrollView(context).apply { addView(fields) })
            .setPositiveButton(R.string.apply) { _, _ -> onApply() }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    fun labels(resources: List<Int>): List<CharSequence> = resources.map { context.getString(it) }

    private fun addField(label: Int, value: String, inputType: Int): TextField {
        val labelView = labelView(label)
        val editText = EditText(context).apply {
            this.inputType = inputType
            isSingleLine = true
            setText(value)
        }
        fields.addView(labelView)
        fields.addView(editText)
        return TextField(labelView, editText, value)
    }

    private fun labelView(label: Int) = TextView(context).apply {
        setText(label)
        setPadding(0, dp(PADDING_DP) / 2, 0, 0)
    }

    private fun dp(value: Float): Int =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, value, context.resources.displayMetrics).toInt()

    private companion object {
        const val PADDING_DP = 24f
    }
}

/** A labeled text field in a [FormDialog]. */
internal class TextField(private val label: TextView, private val editText: EditText, private val initial: String) {
    /** The text as typed, without surrounding spaces. */
    val text: String
        get() = editText.text.toString().trim()

    /** Whether the user changed the text. */
    val edited: Boolean
        get() = text != initial.trim()

    var visible: Boolean
        get() = editText.visibility == View.VISIBLE
        set(value) {
            val visibility = if (value) View.VISIBLE else View.GONE
            label.visibility = visibility
            editText.visibility = visibility
        }
}
