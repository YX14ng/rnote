package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.TextWatcher
import android.view.Gravity
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.TextView

/**
 * The search field that takes the place of the first row of the editor's bar: the query,
 * where the results stand ("2 of 7", or why the writing can't be searched yet), the
 * language the writing is read in, and previous and next.
 */
@SuppressLint("ViewConstructor") // Built in code only, with its callbacks.
internal class SearchBar(
    context: Context,
    private val onQuery: (String) -> Unit,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
    private val onLanguage: (InkLanguage) -> Unit,
    onClose: () -> Unit,
) : LinearLayout(context) {
    private val main = Handler(Looper.getMainLooper())
    private val field = EditText(context).apply {
        setHint(R.string.search_hint)
        setTextColor(Graphite.TEXT)
        setHintTextColor(Graphite.TEXT_SECONDARY)
        textSize = 16f
        isSingleLine = true
        imeOptions = EditorInfo.IME_ACTION_SEARCH or EditorInfo.IME_FLAG_NO_EXTRACT_UI
        background = null
    }
    private val status = TextView(context).apply {
        textSize = 13f
        setTextColor(Graphite.TEXT_SECONDARY)
        maxLines = 1
    }
    private val chip = TextView(context).apply {
        textSize = 14f
        setTextColor(Graphite.TEXT)
        gravity = Gravity.CENTER
        background = Graphite.rounded(context, Graphite.RAISED, Graphite.BUTTON_RADIUS_DP, Graphite.OUTLINE)
        contentDescription = context.getString(R.string.search_language)
        tooltipText = contentDescription
        setOnClickListener { pickLanguage() }
    }
    private val search = Runnable { onQuery(field.text.toString()) }

    init {
        gravity = Gravity.CENTER_VERTICAL
        setPadding(dp(4f), 0, dp(8f), 0)
        addView(Graphite.iconButton(context, R.drawable.ic_close, R.string.search_close) { onClose() }, square())
        addView(field, LayoutParams(0, LayoutParams.MATCH_PARENT, 1f))
        addView(status, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply { marginEnd = dp(8f) })
        addView(chip, LayoutParams(dp(CHIP_WIDTH_DP), dp(CHIP_HEIGHT_DP)).apply { marginEnd = dp(4f) })
        addView(Graphite.iconButton(context, R.drawable.ic_chevron_down, R.string.search_previous) { onPrevious() }
            .apply { rotation = 180f }, square())
        addView(Graphite.iconButton(context, R.drawable.ic_chevron_down, R.string.search_next) { onNext() }, square())
        field.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit

            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) = Unit

            override fun afterTextChanged(s: Editable?) {
                // Searched once typing stops, not on every letter.
                main.removeCallbacks(search)
                main.postDelayed(search, TYPING_PAUSE_MS)
            }
        })
        field.setOnEditorActionListener { _, _, _ ->
            main.removeCallbacks(search)
            search.run()
            true
        }
    }

    /** Shows where the results stand, or why the writing can't be searched yet. */
    fun setStatus(text: String) {
        status.text = text
        status.visibility = if (text.isEmpty()) GONE else VISIBLE
    }

    fun setLanguage(language: InkLanguage) {
        chip.text = language.chip
    }

    /** Puts the cursor in the field, with the keyboard up. */
    fun focus() {
        field.requestFocus()
        context.getSystemService(InputMethodManager::class.java).showSoftInput(field, 0)
    }

    /** Empties the field and puts the keyboard away. */
    fun clear() {
        main.removeCallbacks(search)
        field.text.clear()
        context.getSystemService(InputMethodManager::class.java).hideSoftInputFromWindow(windowToken, 0)
    }

    private fun pickLanguage() {
        val menu = PopupMenu(context, chip)
        InkLanguage.entries.forEachIndexed { index, language ->
            menu.menu.add(0, index, index, language.label)
        }
        menu.setOnMenuItemClickListener { item ->
            onLanguage(InkLanguage.entries[item.itemId])
            true
        }
        menu.show()
    }

    private fun square() = LayoutParams(dp(BUTTON_DP), dp(BUTTON_DP))

    private fun dp(value: Float): Int = Graphite.dp(context, value)

    private companion object {
        const val TYPING_PAUSE_MS = 300L
        const val BUTTON_DP = 44f
        const val CHIP_WIDTH_DP = 52f
        const val CHIP_HEIGHT_DP = 32f
    }
}
