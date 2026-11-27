package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.util.Log

/**
 * Takes saved notebooks to Drive once there is a connection, even if the app is closed or
 * killed right after saving. [SyncManager.syncSoon] schedules it beside its own short timer,
 * and a sync that ends done cancels it; without a connection it waits for one.
 */
@SuppressLint("SpecifyJobSchedulerIdRange")
class SyncJob : JobService() {
    private var watching: ((SyncStatus) -> Unit)? = null

    override fun onStartJob(params: JobParameters): Boolean {
        SyncManager.syncNow(this)
        var starting = true
        var endedRightAway = false
        lateinit var watch: (SyncStatus) -> Unit
        watch = { status ->
            val retry = outcome(status)
            if (retry != null) {
                SyncManager.removeListener(watch)
                watching = null
                if (starting) endedRightAway = true else jobFinished(params, retry)
            }
        }
        watching = watch
        // It gets the current status at once: a sync that couldn't start ends the job now.
        SyncManager.addListener(this, watch)
        starting = false
        return !endedRightAway
    }

    override fun onStopJob(params: JobParameters): Boolean {
        watching?.let(SyncManager::removeListener)
        watching = null
        // Stopped by the system (the connection went away): try again later.
        return true
    }

    companion object {
        private const val TAG = "RnoteViewer"
        /**
         * Far from the ids the WorkManager inside ML Kit and Play Services hands out, which
         * count up from 0; this app doesn't use WorkManager itself.
         */
        private const val JOB_ID = 0x524E_4F54

        /**
         * What a sync that reached [status] means for the job: null while it runs, true to
         * run again later (no connection), false when it's over.
         */
        internal fun outcome(status: SyncStatus): Boolean? = when (status) {
            SyncStatus.Running -> null
            is SyncStatus.Pending -> true
            else -> false
        }

        /** Runs a sync once there is a connection, at least [delayMs] from now. */
        internal fun schedule(context: Context, delayMs: Long) {
            val job = JobInfo.Builder(JOB_ID, ComponentName(context, SyncJob::class.java))
                .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                .setMinimumLatency(delayMs)
                .build()
            val scheduled = context.getSystemService(JobScheduler::class.java)?.schedule(job)
            if (scheduled != JobScheduler.RESULT_SUCCESS) Log.w(TAG, "Scheduling the sync job failed")
        }

        /** The saved notebooks reached Drive: nothing left for the job. */
        internal fun cancel(context: Context) {
            context.getSystemService(JobScheduler::class.java)?.cancel(JOB_ID)
        }
    }
}
