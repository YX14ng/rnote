package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.AlertDialog
import android.content.Context
import android.view.View
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.widget.CheckBox
import android.widget.LinearLayout
import android.widget.Spinner
import android.widget.TextView

/**
 * Asks how to export the PDF before it is made: on white paper or as it is, and, in a
 * notebook of sheets, which sheets. The white paper choice is remembered.
 */
internal object PdfOptionsDialog {
    private const val PREFERENCES = "pdf_export"
    private const val KEY_WHITE_PAPER = "white_paper"

    /**
     * Shows the dialog; [sheetCount] and [currentSheet] are null for a notebook without
     * sheets. [action] names the button, e.g. "Export"; [onChosen] gets the options.
     */
    fun ask(
        activity: Activity,
        sheetCount: Int?,
        currentSheet: Int?,
        action: Int,
        onChosen: (PdfOptions) -> Unit,
    ) {
        val preferences = activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
        val padding = Graphite.dp(activity, 24f)
        val content = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(padding, padding / 2, padding, 0)
        }
        val whitePaper = CheckBox(activity).apply {
            setText(R.string.pdf_white_paper)
            isChecked = preferences.getBoolean(KEY_WHITE_PAPER, false)
        }
        content.addView(whitePaper)
        val sheets = sheetCount?.takeIf { it > 1 }?.let { count ->
            SheetChoice(activity, content, count, currentSheet ?: 0)
        }
        AlertDialog.Builder(activity)
            .setTitle(R.string.pdf_options_title)
            .setView(content)
            .setPositiveButton(action) { _, _ ->
                preferences.edit().putBoolean(KEY_WHITE_PAPER, whitePaper.isChecked).apply()
                onChosen(PdfOptions(whitePaper.isChecked, sheets?.chosen()))
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    /** All sheets, the current one, or a range chosen with two drop-downs. */
    private class SheetChoice(activity: Activity, content: LinearLayout, private val count: Int, private val current: Int) {
        private val kind = spinner(
            activity,
            listOf(
                activity.getString(R.string.pdf_sheets_all),
                activity.getString(R.string.pdf_sheets_current, current + 1),
                activity.getString(R.string.pdf_sheets_range),
            ),
            0,
        )
        private val numbers = (1..count).map(Int::toString)
        private val from = spinner(activity, numbers, 0)
        private val to = spinner(activity, numbers, count - 1)
        private val range = LinearLayout(activity).apply {
            orientation = LinearLayout.HORIZONTAL
            addView(label(activity, R.string.pdf_sheets_from))
            addView(from)
            addView(label(activity, R.string.pdf_sheets_to))
            addView(to)
            visibility = View.GONE
        }

        init {
            content.addView(label(activity, R.string.pdf_sheets))
            content.addView(kind)
            content.addView(range)
            kind.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
                override fun onItemSelected(parent: AdapterView<*>?, view: View?, position: Int, id: Long) {
                    range.visibility = if (position == PdfOptions.Sheets.RANGE.ordinal) View.VISIBLE else View.GONE
                }

                override fun onNothingSelected(parent: AdapterView<*>?) {}
            }
        }

        fun chosen(): IntRange? = PdfOptions.sheets(
            PdfOptions.Sheets.entries[kind.selectedItemPosition],
            current,
            from.selectedItemPosition,
            to.selectedItemPosition,
            count,
        )

        private fun spinner(activity: Activity, items: List<String>, selected: Int) = Spinner(activity).apply {
            adapter = ArrayAdapter(activity, android.R.layout.simple_spinner_dropdown_item, items)
            setSelection(selected)
        }

        private fun label(activity: Activity, text: Int) = TextView(activity).apply { setText(text) }
    }
}
