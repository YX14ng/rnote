package io.github.yx14ng.rnote.viewer

/**
 * When the writing of the open notebook is read again: right after opening it, [delayMs]
 * after the last edit, and later again when a run couldn't read everything (waiting twice
 * as long each time, up to five minutes); one run at a time. Times are in milliseconds.
 */
internal class InkSchedule(private val delayMs: Long = DELAY_MS) {
    // When the last edit asked for a run; null once a run took it.
    private var requestedAt: Long? = null
    private var running = false
    private var closed = false
    // How long the next retry waits.
    private var retryMs = FIRST_RETRY_MS

    /** The notebook was opened: read it right away. */
    fun opened(now: Long) {
        requestedAt = now - delayMs
    }

    fun edited(now: Long) {
        requestedAt = now
    }

    /** How long until a run is due, if one was asked for. */
    fun dueIn(now: Long): Long? = requestedAt?.let { at -> maxOf(0L, at + delayMs - now) }

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

    /** A run ended at [now], with [unread] fragments it couldn't read. */
    fun finished(now: Long, unread: Int) {
        running = false
        if (unread == 0) {
            retryMs = FIRST_RETRY_MS
            return
        }
        // An edit during the run asked for one sooner.
        if (requestedAt == null) requestedAt = now + retryMs - delayMs
        retryMs = minOf(retryMs * 2, MAX_RETRY_MS)
    }

    /** The notebook was closed: nothing runs anymore. */
    fun close() {
        closed = true
    }

    companion object {
        const val DELAY_MS = 3_000L
        private const val FIRST_RETRY_MS = 30_000L
        private const val MAX_RETRY_MS = 300_000L
    }
}
