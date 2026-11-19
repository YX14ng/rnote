package io.github.yx14ng.rnote.viewer

import kotlin.math.floor

/** The arithmetic of a document of sheets, for the page column. */
internal object Pages {
    /**
     * The sheet in the middle of a view showing [visibleHeight] of the document from
     * [viewTop], among [count] sheets [sheetHeight] tall.
     */
    fun current(viewTop: Double, visibleHeight: Double, sheetHeight: Double, count: Int): Int {
        if (count <= 0 || sheetHeight <= 0.0) return 0
        val middle = viewTop + visibleHeight / 2
        return floor(middle / sheetHeight).toInt().coerceIn(0, count - 1)
    }

    /** The sheets a view showing [visibleHeight] of the document from [viewTop] crosses. */
    fun visible(viewTop: Double, visibleHeight: Double, sheetHeight: Double, count: Int): IntRange {
        if (count <= 0 || sheetHeight <= 0.0) return IntRange.EMPTY
        val first = floor(viewTop / sheetHeight).toInt().coerceIn(0, count - 1)
        val last = floor((viewTop + visibleHeight) / sheetHeight).toInt().coerceIn(first, count - 1)
        return first..last
    }

    /** Whether removing [pages] takes something written away, so it is worth asking first. */
    fun removingLosesContent(pages: Set<Int>, withContent: BooleanArray): Boolean =
        pages.any { withContent.getOrElse(it) { false } }

    /** Where sheet [page] starts, in document coordinates. */
    fun top(page: Int, sheetHeight: Double): Double = page * sheetHeight
}
