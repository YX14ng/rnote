package io.github.yx14ng.rnote.viewer

/**
 * How a PDF export comes out: [onWhitePaper] for printing a notebook written in white on
 * dark paper (white background, dark ink), and only the [sheets] chosen (0-based, both ends
 * included), or every sheet when null.
 */
internal data class PdfOptions(val onWhitePaper: Boolean = false, val sheets: IntRange? = null) {
    /** The sheets to take, as the dialog offers them. */
    enum class Sheets { ALL, CURRENT, RANGE }

    companion object {
        /**
         * The sheets of a notebook of [count] sheets that [choice] takes: the [current] one,
         * or those from [from] to [to] in either order, kept inside the notebook.
         */
        fun sheets(choice: Sheets, current: Int, from: Int, to: Int, count: Int): IntRange? {
            val last = (count - 1).coerceAtLeast(0)
            return when (choice) {
                Sheets.ALL -> null
                Sheets.CURRENT -> current.coerceIn(0, last).let { it..it }
                Sheets.RANGE -> minOf(from, to).coerceIn(0, last)..maxOf(from, to).coerceIn(0, last)
            }
        }
    }
}
