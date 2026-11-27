package io.github.yx14ng.rnote.viewer

/**
 * Tells the palm from fingers that navigate: while the S Pen hovers near the screen, and for
 * a moment after it leaves or lifts, the hand holding it rests on the screen, so touches then
 * neither pan, zoom nor undo. Times are `MotionEvent.getEventTime()` values.
 */
internal class PalmGuard {
    private var hovering = false
    private var lastPenAt = NEVER

    /** The pen came into hover range ([inRange]) or left it, at [at]. */
    fun penHover(inRange: Boolean, at: Long) {
        hovering = inRange
        lastPenAt = at
    }

    /** The pen touched, moved on or left the screen at [at]. */
    fun penTouched(at: Long) {
        lastPenAt = at
    }

    /**
     * Whether fingers landing at [at] are fingers, not the palm. A hover with no news for a
     * while counts as over: not every device says when the pen leaves.
     */
    fun fingersAllowed(at: Long): Boolean =
        at - lastPenAt > if (hovering) HOVER_STALE_MS else GRACE_MS

    companion object {
        /** How long after the pen leaves the palm may still be resting. */
        const val GRACE_MS = 600L

        /** How long a hover lasts without hover events: the pen sends them while in range. */
        const val HOVER_STALE_MS = 3_000L

        private const val NEVER = Long.MIN_VALUE / 2
    }
}
