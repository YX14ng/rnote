package io.github.yx14ng.rnote.viewer

/**
 * How long ago the last sync was, in the steps the sync menu says it: "just now" under a
 * minute, then minutes, then hours, then the date. Worded with the app's own strings, so it
 * is in the language of the rest of the app, not the system's.
 */
internal sealed interface SyncAgo {
    data object JustNow : SyncAgo

    data class Minutes(val count: Int) : SyncAgo

    data class Hours(val count: Int) : SyncAgo

    /** A day or more: the date says it better. */
    data object OnDate : SyncAgo

    companion object {
        private const val MINUTE_MS = 60_000L
        private const val HOUR_MS = 60 * MINUTE_MS
        private const val DAY_MS = 24 * HOUR_MS

        /** For a sync [elapsedMs] ago; a clock that went back counts as just now. */
        fun of(elapsedMs: Long): SyncAgo = when {
            elapsedMs < MINUTE_MS -> JustNow
            elapsedMs < HOUR_MS -> Minutes((elapsedMs / MINUTE_MS).toInt())
            elapsedMs < DAY_MS -> Hours((elapsedMs / HOUR_MS).toInt())
            else -> OnDate
        }
    }
}
