package io.github.yx14ng.rnote.viewer

/**
 * When the open notebook saves itself while it is edited: a short while after the last edit,
 * or, while the writing goes on without a pause, once it has gone unsaved for a minute. Times
 * are milliseconds on any clock that only goes forward.
 */
internal class AutosaveSchedule {
    private var firstUnsaved: Long? = null
    private var lastEdit = 0L

    /** The document changed at [now]. */
    fun edited(now: Long) {
        if (firstUnsaved == null) firstUnsaved = now
        lastEdit = now
    }

    /** The document was saved: nothing is due until it changes again. */
    fun saved() {
        firstUnsaved = null
    }

    /** When it should be saved, or null if there is nothing unsaved. */
    fun dueAt(): Long? {
        val first = firstUnsaved ?: return null
        return minOf(lastEdit + IDLE_MS, first + MAX_UNSAVED_MS)
    }

    companion object {
        /** How long after the last edit it saves. */
        const val IDLE_MS = 4_000L

        /** How long it goes unsaved at most while the writing doesn't pause. */
        const val MAX_UNSAVED_MS = 60_000L
    }
}
