package io.github.yx14ng.rnote.viewer

import android.app.AlertDialog
import android.content.Context
import android.util.TypedValue
import android.widget.ArrayAdapter
import android.widget.CheckBox
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Spinner
import android.widget.TextView

/**
 * A small settings form in a dialog, built from framework views: labeled choices and
 * checkboxes. Each `add` call returns a getter for the value the user ends up with.
 */
internal class FormDialog(private val context: Context, private val title: Int) {
    private val fields = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        val padding = dp(PADDING_DP)
        setPadding(padding, padding / 2, padding, 0)
    }

    /** A drop-down with [options] (string resources or plain strings), starting at [selected]. */
    fun addChoice(label: Int, options: List<CharSequence>, selected: Int): () -> Int {
        fields.addView(TextView(context).apply {
            setText(label)
            setPadding(0, dp(PADDING_DP) / 2, 0, 0)
        })
        val spinner = Spinner(context).apply {
            adapter = ArrayAdapter(context, android.R.layout.simple_spinner_dropdown_item, options)
            setSelection(selected.coerceIn(0, options.lastIndex))
        }
        fields.addView(spinner)
        return { spinner.selectedItemPosition }
    }

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

    private fun dp(value: Float): Int =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, value, context.resources.displayMetrics).toInt()

    private companion object {
        const val PADDING_DP = 24f
    }
}
