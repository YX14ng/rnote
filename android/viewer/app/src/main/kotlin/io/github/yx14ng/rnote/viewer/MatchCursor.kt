package io.github.yx14ng.rnote.viewer

/** The result of a search on screen, moving through them in a loop. */
internal class MatchCursor {
    private var count = 0

    /** The result shown, from 0; null without results. */
    var current: Int? = null
        private set

    /** The result shown and how many there are, counting from 1: "2 of 7"; 0 of 0 without. */
    val position: Pair<Int, Int>
        get() = (current?.plus(1) ?: 0) to count

    /** A new search found [count] results: the first one is shown. */
    fun reset(count: Int) {
        this.count = count
        current = if (count > 0) 0 else null
    }

    fun next() {
        current = current?.let { (it + 1) % count }
    }

    fun previous() {
        current = current?.let { (it - 1 + count) % count }
    }
}
