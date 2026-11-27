package io.github.yx14ng.rnote.viewer

/**
 * Work that needs the open notebook, held while it loads: a photo, a picture or a PDF that
 * comes back after Android restarted the app (as it may while the camera is open) reaches
 * the editor before the notebook is open again. On the main thread.
 */
internal class UntilOpen {
    private val waiting = ArrayDeque<() -> Unit>()
    private var open = true

    /** Runs [action] now if the notebook is open, else once it is. */
    fun run(action: () -> Unit) {
        if (open) action() else waiting.addLast(action)
    }

    /** A notebook starts loading. */
    fun loading() {
        open = false
    }

    /** The notebook is open: what waited for it runs, in order. */
    fun opened() {
        open = true
        while (waiting.isNotEmpty()) waiting.removeFirst()()
    }

    /** The notebook couldn't be opened: what waited is dropped, never applied to another. */
    fun failed() {
        waiting.clear()
    }
}
