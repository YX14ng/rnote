package io.github.yx14ng.rnote.viewer

/**
 * Why a sync failed or skipped notebooks, said in the user's language. The native side
 * tells the kind of failure (`rnote_sync::FailureKind`), never the technical chain.
 */
internal object SyncFailure {
    /** The reason to show for a failure of [kind]. */
    fun reason(kind: String): Int = when (kind) {
        "drive_full" -> R.string.sync_reason_drive_full
        "drive_refused" -> R.string.sync_reason_drive_refused
        "changed_meanwhile" -> R.string.sync_reason_changed
        "local_files" -> R.string.sync_reason_local
        else -> R.string.error_unexpected
    }

    /** Whether a failure of [kind] goes away by itself, so the sync tries again soon. */
    fun retrySoon(kind: String): Boolean = kind == "changed_meanwhile"

    /** The notebooks of the report's skipped entries, which read `path: reason`. */
    fun skippedNotebooks(skipped: List<String>): List<String> = skipped.map { it.substringBefore(": ") }
}
