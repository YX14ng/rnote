package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.AlertDialog
import android.os.Build
import android.window.OnBackInvokedCallback
import android.window.OnBackInvokedDispatcher

/**
 * Keeps unsaved changes from being lost by accident: asks before dropping them and, from
 * Android 13 on, intercepts back while there are any.
 *
 * [onDiscard] runs once the user agreed to drop them, before the action that drops them.
 */
internal class UnsavedChangesGuard(
    private val activity: Activity,
    private val onDiscard: () -> Unit,
    /** What back does while there are unsaved changes: by default, ask before dropping them. */
    private val onBack: (() -> Unit)? = null,
) {
    /** Whether the open document has unsaved changes. */
    var modified = false
        set(value) {
            field = value
            updateBackHandling()
        }

    /** Set once the user agreed to drop the changes, so going to the background doesn't save them. */
    var changesDiscarded = false

    /**
     * Takes back an agreement to drop the changes, when what was to replace the document
     * didn't (a cancelled picker, a file that failed to open): autosave covers them again.
     */
    fun keepChanges() {
        changesDiscarded = false
    }

    private var backCallback: OnBackInvokedCallback? = null

    /** Asks before throwing away unsaved changes; runs [action] right away if there are none. */
    fun confirmDiscard(action: () -> Unit) {
        if (!modified) {
            action()
            return
        }
        AlertDialog.Builder(activity)
            .setMessage(R.string.discard_message)
            .setPositiveButton(R.string.discard) { _, _ ->
                changesDiscarded = true
                onDiscard()
                action()
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun updateBackHandling() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            return
        }
        val registered = backCallback
        if (modified && registered == null) {
            val callback = OnBackInvokedCallback { onBack?.invoke() ?: confirmDiscard(activity::finish) }
            activity.onBackInvokedDispatcher.registerOnBackInvokedCallback(
                OnBackInvokedDispatcher.PRIORITY_DEFAULT,
                callback,
            )
            backCallback = callback
        } else if (!modified && registered != null) {
            activity.onBackInvokedDispatcher.unregisterOnBackInvokedCallback(registered)
            backCallback = null
        }
    }
}
