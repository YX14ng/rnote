package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.text.format.DateUtils
import android.widget.ImageButton
import android.widget.PopupMenu
import android.widget.Toast

/**
 * The cloud in the library's top bar: signs in to Google Drive, shows where the sync stands
 * and, signed in, offers to sync now or sign out. [attach] and [detach] follow the screen's
 * resume and pause.
 */
internal class SyncButton(private val activity: Activity) {
    val view: ImageButton = Graphite.iconButton(activity, R.drawable.ic_cloud_off, R.string.sync_button) { onTap() }

    /** Runs when a sync changed the library while the screen shows. */
    var onLibraryChanged: () -> Unit = {}

    /** The status last shown; null until [attach] gives the current one. */
    private var shown: SyncStatus? = null
    private val listener: (SyncStatus) -> Unit = ::show

    /** The failure last told in a toast, until a sync works again. */
    private var toastedFailure: String? = null

    fun attach() {
        shown = null
        SyncManager.addListener(activity, listener)
    }

    fun detach() {
        SyncManager.removeListener(listener)
    }

    /** Takes the result of Google's sign-in screens; returns whether it was theirs. */
    fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?): Boolean {
        if (requestCode != REQUEST_SIGN_IN) return false
        if (resultCode == Activity.RESULT_OK) {
            onToken(DriveAuth.finishSignIn(activity, data))
        }
        return true
    }

    private fun show(status: SyncStatus) {
        val previous = shown
        shown = status
        val icon = when (status) {
            SyncStatus.Unavailable, SyncStatus.SignedOut, SyncStatus.SignInNeeded -> R.drawable.ic_cloud_off
            SyncStatus.Running -> R.drawable.ic_cloud_sync
            is SyncStatus.Done, is SyncStatus.Pending, is SyncStatus.Failed -> R.drawable.ic_cloud
        }
        view.setImageResource(icon)
        view.alpha = if (status is SyncStatus.Pending || status is SyncStatus.Failed) DIMMED_ALPHA else 1f
        // The status the screen found when it came back was handled by its own reload.
        if (previous == null) return
        when (status) {
            is SyncStatus.Done -> {
                toastedFailure = null
                val report = status.report
                if (report.didAnything) onLibraryChanged()
                if (report.conflicts.isNotEmpty()) {
                    val text = activity.resources.getQuantityString(R.plurals.sync_conflicts, report.conflicts.size, report.conflicts.size)
                    Toast.makeText(activity, text, Toast.LENGTH_LONG).show()
                }
            }
            // Once per failure, not once per attempt.
            is SyncStatus.Failed -> if (status.message != toastedFailure) {
                toastedFailure = status.message
                Toast.makeText(activity, activity.getString(R.string.sync_status_failed, status.message), Toast.LENGTH_LONG).show()
            }
            else -> Unit
        }
    }

    private fun onTap() {
        when (SyncManager.status) {
            SyncStatus.Unavailable -> showUnavailable()
            SyncStatus.SignedOut, SyncStatus.SignInNeeded ->
                DriveAuth.startSignIn(activity, REQUEST_SIGN_IN, ::onToken)
            else -> showMenu()
        }
    }

    private fun onToken(result: TokenResult) {
        when (result) {
            is TokenResult.Token -> SyncManager.signedIn(activity, result.value)
            TokenResult.NotRegistered -> showUnavailable()
            is TokenResult.Failed -> signInFailed(result.error.message ?: result.error.toString())
            is TokenResult.Offline -> signInFailed(result.message)
            // Google's screens never ask for more screens.
            is TokenResult.NeedsUser -> signInFailed(result.toString())
        }
    }

    private fun signInFailed(message: String) {
        Toast.makeText(activity, activity.getString(R.string.sync_sign_in_failed, message), Toast.LENGTH_LONG).show()
    }

    private fun showUnavailable() {
        AlertDialog.Builder(activity)
            .setTitle(R.string.sync_button)
            .setMessage(R.string.sync_unavailable)
            .setPositiveButton(android.R.string.ok, null)
            .show()
    }

    private fun showMenu() {
        val menu = PopupMenu(activity, view)
        menu.menu.add(statusText()).isEnabled = false
        menu.menu.add(R.string.sync_now).setOnMenuItemClickListener { SyncManager.syncNow(activity); true }
        menu.menu.add(R.string.sync_sign_out).setOnMenuItemClickListener { confirmSignOut(); true }
        menu.show()
    }

    private fun statusText(): String = when (val status = SyncManager.status) {
        is SyncStatus.Done -> activity.getString(R.string.sync_status_done, ago(status.atMs))
        // Pending always means Google couldn't be reached; the error itself says nothing more.
        is SyncStatus.Pending ->
            activity.getString(if (status.message.isEmpty()) R.string.sync_status_waiting else R.string.sync_status_offline)
        is SyncStatus.Failed -> activity.getString(R.string.sync_status_failed, status.message)
        else -> activity.getString(R.string.sync_status_running)
    }

    private fun ago(atMs: Long): String {
        val res = activity.resources
        return when (val ago = SyncAgo.of(System.currentTimeMillis() - atMs)) {
            SyncAgo.JustNow -> res.getString(R.string.sync_ago_just_now)
            is SyncAgo.Minutes -> res.getQuantityString(R.plurals.sync_ago_minutes, ago.count, ago.count)
            is SyncAgo.Hours -> res.getQuantityString(R.plurals.sync_ago_hours, ago.count, ago.count)
            SyncAgo.OnDate -> res.getString(
                R.string.sync_ago_on,
                DateUtils.formatDateTime(activity, atMs, DateUtils.FORMAT_SHOW_DATE or DateUtils.FORMAT_SHOW_TIME),
            )
        }
    }

    private fun confirmSignOut() {
        AlertDialog.Builder(activity)
            .setTitle(R.string.sync_sign_out)
            .setMessage(R.string.sync_sign_out_message)
            .setPositiveButton(R.string.sync_sign_out) { _, _ ->
                SyncManager.signOut(activity) { result ->
                    result.onFailure {
                        Toast.makeText(activity, R.string.sync_signed_out_offline, Toast.LENGTH_LONG).show()
                    }
                }
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    companion object {
        /** The request code of Google's sign-in screens; other screens' codes are below 100. */
        const val REQUEST_SIGN_IN = 100

        private const val DIMMED_ALPHA = 0.6f
    }
}
