package io.github.yx14ng.rnote.viewer

/**
 * Lets the library open one editor at a time: a double tap on a card would otherwise open
 * two editors of the same notebook, and writing in the one left behind ends in a conflict.
 */
internal class LaunchGuard {
    private var starting = false

    /** Whether an editor may start now; if so, none may until [resumed]. */
    fun tryStart(): Boolean {
        if (starting) return false
        starting = true
        return true
    }

    /** The library is shown again. */
    fun resumed() {
        starting = false
    }
}
