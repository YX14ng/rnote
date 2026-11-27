package io.github.yx14ng.rnote.viewer

import androidx.annotation.StringRes

/** What the stylus does while its side button is held and it touches the screen. */
enum class PenButton(@StringRes val label: Int) {
    ERASER(R.string.pen_button_eraser),
    LASSO(R.string.pen_button_lasso),
    NOTHING(R.string.pen_button_nothing);

    /**
     * The tool a stylus touch uses: its eraser end always erases; with the button held, the
     * tool this stands for; otherwise the [selected] one.
     */
    fun toolFor(eraserEnd: Boolean, buttonHeld: Boolean, selected: Tool): Tool = when {
        eraserEnd -> Tool.ERASER
        !buttonHeld -> selected
        this == ERASER -> Tool.ERASER
        this == LASSO -> Tool.SELECTOR
        else -> selected
    }

    /** The choice after this one, for the menu row that goes through them. */
    fun next(): PenButton = entries[(ordinal + 1) % entries.size]
}
