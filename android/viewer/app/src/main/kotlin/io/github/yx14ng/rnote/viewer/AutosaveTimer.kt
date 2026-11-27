package io.github.yx14ng.rnote.viewer

import android.os.Handler
import android.os.Looper
import android.os.SystemClock

/**
 * Saves the open notebook when [AutosaveSchedule] says so, never under the pen: on the main
 * thread, as the edits come.
 */
internal class AutosaveTimer(
    /** Whether there is something to save: a notebook open, changed, and not discarded. */
    private val hasChanges: () -> Boolean,
    private val penDown: () -> Boolean,
    private val save: () -> Unit,
) {
    private val schedule = AutosaveSchedule()
    private val main = Handler(Looper.getMainLooper())
    private val check = Runnable { saveIfDue() }

    /** The notebook changed just now. */
    fun edited() {
        schedule.edited(SystemClock.uptimeMillis())
        wake()
    }

    /** The notebook was saved: nothing is due until it changes again. */
    fun saved() = schedule.saved()

    /** Another notebook, or none: what was due belonged to the one before. */
    fun stop() {
        schedule.saved()
        main.removeCallbacks(check)
    }

    private fun wake() {
        val due = schedule.dueAt() ?: return
        main.removeCallbacks(check)
        main.postDelayed(check, (due - SystemClock.uptimeMillis()).coerceAtLeast(0))
    }

    private fun saveIfDue() {
        val due = schedule.dueAt() ?: return
        if (SystemClock.uptimeMillis() < due) {
            wake()
            return
        }
        if (!hasChanges()) {
            schedule.saved()
            return
        }
        if (penDown()) {
            main.postDelayed(check, PEN_UP_RETRY_MS)
            return
        }
        // Done once asked: a failed write leaves a recovery copy, and the next edit asks again.
        schedule.saved()
        save()
    }

    private companion object {
        /** How soon it looks again when the pen was down. */
        const val PEN_UP_RETRY_MS = 1_000L
    }
}
