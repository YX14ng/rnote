package io.github.yx14ng.rnote.viewer

import kotlin.math.abs

/**
 * Adding a page by pulling past the end of a document of pages, like StarNote: a drag
 * towards the end while the view is already there starts the pull, and holding it for
 * [holdMs] adds the page. Leaving the end or lifting the finger first adds nothing; a
 * touch adds one page at most.
 *
 * Distances are scroll distances: positive [onDrag] `dy` moves towards the end. Times are
 * in milliseconds from any fixed origin.
 */
internal class PagePull(private val holdMs: Long = HOLD_MS) {
    private var startedAt: Long? = null
    private var lastAt = 0L

    // Set once the page was added, until the finger lifts.
    private var done = false

    /** How far along the pull is, from 0 to 1, or null when there is none. */
    val progress: Float?
        get() = startedAt?.let { start -> ((lastAt - start).toFloat() / holdMs).coerceIn(0f, 1f) }

    /** The finger moved by ([dx], [dy]); returns true when the page is to be added now. */
    fun onDrag(dx: Float, dy: Float, atEnd: Boolean, now: Long): Boolean {
        if (done) return false
        if (startedAt == null) {
            if (atEnd && dy > 0 && dy >= abs(dx)) {
                startedAt = now
                lastAt = now
            }
            return false
        }
        return onTick(atEnd, now)
    }

    /** Time passed with the finger down; returns true when the page is to be added now. */
    fun onTick(atEnd: Boolean, now: Long): Boolean {
        val start = startedAt ?: return false
        if (!atEnd) {
            cancel()
            return false
        }
        lastAt = now
        if (now - start < holdMs) return false
        startedAt = null
        done = true
        return true
    }

    /** Stops the pull, e.g. when a pinch starts; a new drag may start another. */
    fun cancel() {
        startedAt = null
    }

    /** The finger lifted: the next touch may add a page again. */
    fun onLift() {
        startedAt = null
        done = false
    }

    private companion object {
        const val HOLD_MS = 1000L
    }
}
