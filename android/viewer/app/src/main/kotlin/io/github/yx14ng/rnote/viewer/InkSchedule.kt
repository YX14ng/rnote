package io.github.yx14ng.rnote.viewer

/**
 * When the writing of the open notebook is read again: right after opening it, and
 * [delayMs] after the last edit, one run at a time. Times are in milliseconds.
 */
internal class InkSchedule(private val delayMs: Long = DELAY_MS) {
    // When the last edit asked for a run; null once a run took it.
    private var requestedAt: Long? = null
    private var running = false
    private var closed = false

    /** The notebook was opened: read it right away. */
    fun opened(now: Long) {
        requestedAt = now - delayMs
    }

    fun edited(now: Long) {
        requestedAt = now
    }

    /** Whether a run should start now. */
    fun due(now: Long): Boolean {
        val at = requestedAt ?: return false
        return !closed && !running && now - at >= delayMs
    }

    /** A run started: it takes the request; edits from now on ask for another. */
    fun started() {
        running = true
        requestedAt = null
    }

    fun finished() {
        running = false
    }

    /** The notebook was closed: nothing runs anymore. */
    fun close() {
        closed = true
    }

    companion object {
        const val DELAY_MS = 3_000L
    }
}
