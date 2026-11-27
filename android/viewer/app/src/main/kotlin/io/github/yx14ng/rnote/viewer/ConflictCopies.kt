package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.AlertDialog

/**
 * The conflict copies a sync made, when a notebook changed on two devices: told by name in
 * a dialog, where tapping one opens it to compare it with the other version.
 */
internal object ConflictCopies {
    /** The name the library shows for the notebook at [path]. */
    fun name(path: String): String = path.substringAfterLast('/').substringBeforeLast('.')

    /** Tells about the copies at [paths]; [open] opens the one tapped. */
    fun show(activity: Activity, paths: List<String>, open: (String) -> Unit) {
        val title = activity.resources.getQuantityString(R.plurals.sync_conflicts, paths.size, paths.size)
        AlertDialog.Builder(activity)
            .setTitle(title)
            .setItems(paths.map(::name).toTypedArray()) { _, which -> open(paths[which]) }
            .setPositiveButton(android.R.string.ok, null)
            .show()
    }
}
