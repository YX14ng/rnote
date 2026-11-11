package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.Executors

/** What a sync did; the paths are relative to the library. */
internal data class SyncReport(
    val uploaded: Int,
    val downloaded: Int,
    val moved: Int,
    val deleted: Int,
    val conflicts: List<String>,
    val changedHere: List<String>,
    val skipped: List<String>,
) {
    companion object {
        val EMPTY = SyncReport(0, 0, 0, 0, emptyList(), emptyList(), emptyList())

        fun parse(json: JSONObject) = SyncReport(
            uploaded = json.getInt("uploaded"),
            downloaded = json.getInt("downloaded"),
            moved = json.getInt("moved"),
            deleted = json.getInt("deleted"),
            conflicts = json.getJSONArray("conflicts").strings(),
            changedHere = json.getJSONArray("changed_here").strings(),
            skipped = json.getJSONArray("skipped").strings(),
        )

        private fun JSONArray.strings(): List<String> = (0 until length()).map(::getString)
    }
}

/** Where the sync stands, as the sync button shows it. */
internal sealed interface SyncStatus {
    /** This build can't sign in to Google: it isn't signed with the stable key. */
    data object Unavailable : SyncStatus

    data object SignedOut : SyncStatus

    /** Signed in before, but Google needs the user again (e.g. access removed). */
    data object SignInNeeded : SyncStatus

    data object Running : SyncStatus

    data class Done(val atMs: Long, val report: SyncReport) : SyncStatus

    /** Couldn't reach Google: the next trigger tries again. Empty [message] before the first sync. */
    data class Pending(val message: String) : SyncStatus

    data class Failed(val message: String) : SyncStatus
}

/**
 * When and how the library syncs with Google Drive: one sync at a time on its own thread,
 * with a token asked without showing anything. Nothing here shows UI; everything runs and
 * calls back on the main thread.
 */
internal object SyncManager {
    private const val PREFERENCES = "sync"
    private const val KEY_ENABLED = "enabled"
    private const val KEY_LAST_DONE_MS = "last_done_ms"

    /** How long after the last save [syncSoon] waits. */
    private const val SOON_DELAY_MS = 5_000L

    private val main = Handler(Looper.getMainLooper())
    private val worker = Executors.newSingleThreadExecutor()
    private val listeners = mutableListOf<(SyncStatus) -> Unit>()
    private lateinit var app: Context

    /** Whether a trigger came while a sync ran: one more runs when it ends. */
    private var again = false
    private val soon = Runnable { syncNow(app) }

    var status: SyncStatus = SyncStatus.SignedOut
        private set

    /** Adds [listener], which gets the current status right away. */
    fun addListener(context: Context, listener: (SyncStatus) -> Unit) {
        init(context)
        listeners += listener
        listener(status)
    }

    fun removeListener(listener: (SyncStatus) -> Unit) {
        listeners -= listener
    }

    /** Syncs now, or right after the sync running; nothing if not signed in. */
    fun syncNow(context: Context) {
        init(context)
        when (status) {
            SyncStatus.Unavailable, SyncStatus.SignedOut -> return
            SyncStatus.Running -> again = true
            else -> {
                setStatus(SyncStatus.Running)
                fetchToken(retried = false)
            }
        }
    }

    /** Syncs a few seconds after the last call: after saving a notebook. */
    fun syncSoon(context: Context) {
        init(context)
        main.removeCallbacks(soon)
        main.postDelayed(soon, SOON_DELAY_MS)
    }

    /** The user signed in from the sync button and Google gave [token]: syncs with it. */
    fun signedIn(context: Context, token: String) {
        init(context)
        preferences().edit().putBoolean(KEY_ENABLED, true).apply()
        if (status == SyncStatus.Running) {
            again = true
        } else {
            setStatus(SyncStatus.Running)
            runSync(token, retried = false)
        }
    }

    /**
     * Signs out: Google forgets the app's access and the device its token. The local
     * sign-out happens even if Google can't be told, which [done] then reports.
     */
    fun signOut(context: Context, done: (Result<Unit>) -> Unit) {
        init(context)
        DriveAuth.token(app) { result ->
            val token = (result as? TokenResult.Token)?.value
            worker.execute {
                val revoked = token?.let { runCatching { NativeBridge.syncRevoke(it) } } ?: Result.success(Unit)
                main.post {
                    val finish = {
                        preferences().edit().putBoolean(KEY_ENABLED, false).apply()
                        main.removeCallbacks(soon)
                        setStatus(SyncStatus.SignedOut)
                        done(revoked)
                    }
                    if (token != null) DriveAuth.clearToken(app, token, finish) else finish()
                }
            }
        }
    }

    private fun init(context: Context) {
        if (::app.isInitialized) return
        app = context.applicationContext
        status = when {
            !BuildConfig.DRIVE_SYNC -> SyncStatus.Unavailable
            !enabled() -> SyncStatus.SignedOut
            else -> {
                val last = preferences().getLong(KEY_LAST_DONE_MS, 0)
                if (last > 0) SyncStatus.Done(last, SyncReport.EMPTY) else SyncStatus.Pending("")
            }
        }
    }

    private fun fetchToken(retried: Boolean) {
        DriveAuth.token(app) { result ->
            when (result) {
                is TokenResult.Token -> runSync(result.value, retried)
                is TokenResult.NeedsUser -> finish(SyncStatus.SignInNeeded)
                TokenResult.NotRegistered -> finish(SyncStatus.Unavailable)
                is TokenResult.Failed -> finish(SyncStatus.Failed(messageOf(result.error)))
            }
        }
    }

    private fun runSync(token: String, retried: Boolean) {
        val library = LibraryStore.forApp(app).root
        val state = File(app.filesDir, "sync/state.json")
        worker.execute {
            val answer = runCatching {
                library.mkdirs()
                JSONObject(NativeBridge.syncRun(library.path, state.path, Build.MODEL, token))
            }
            main.post { handle(answer, token, retried) }
        }
    }

    private fun handle(answer: Result<JSONObject>, token: String, retried: Boolean) {
        val json = answer.getOrElse { error ->
            finish(SyncStatus.Failed(messageOf(error)))
            return
        }
        when (json.getString("status")) {
            "ok" -> {
                val now = System.currentTimeMillis()
                preferences().edit().putLong(KEY_LAST_DONE_MS, now).apply()
                finish(SyncStatus.Done(now, SyncReport.parse(json)))
            }
            // The token expired: a fresh one, once per sync.
            "unauthorized" ->
                if (retried) {
                    finish(SyncStatus.SignInNeeded)
                } else {
                    DriveAuth.clearToken(app, token) { fetchToken(retried = true) }
                }
            "offline" -> finish(SyncStatus.Pending(json.optString("message")))
            // Only another process could hold the lock; the next trigger tries again.
            "busy" -> finish(SyncStatus.Pending(""))
            else -> finish(SyncStatus.Failed(json.optString("message")))
        }
    }

    private fun finish(result: SyncStatus) {
        // Signed out while it ran: whatever it did, the button shows signed out.
        setStatus(if (enabled() || result == SyncStatus.Unavailable) result else SyncStatus.SignedOut)
        if (again) {
            again = false
            syncNow(app)
        }
    }

    private fun setStatus(status: SyncStatus) {
        this.status = status
        for (listener in listeners.toList()) listener(status)
    }

    private fun enabled() = preferences().getBoolean(KEY_ENABLED, false)

    private fun preferences() = app.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)

    private fun messageOf(error: Throwable) = error.message ?: error.toString()
}
