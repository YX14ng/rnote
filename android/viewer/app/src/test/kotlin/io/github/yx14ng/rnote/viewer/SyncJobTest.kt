package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The background job that takes saved notebooks to Drive even if the app is closed. */
class SyncJobTest {
    @Test
    fun aSyncStillRunningIsWaitedFor() {
        assertNull(SyncJob.outcome(SyncStatus.Running))
    }

    @Test
    fun withoutAConnectionTheJobRunsAgainWhenThereIsOne() {
        assertEquals(true, SyncJob.outcome(SyncStatus.Pending("offline")))
    }

    @Test
    fun anyOtherEndFinishesTheJob() {
        val ends = listOf(
            SyncStatus.Done(1, SyncReport.EMPTY),
            SyncStatus.Failed("x"),
            SyncStatus.SignInNeeded,
            SyncStatus.SignedOut,
            SyncStatus.Unavailable,
        )
        for (end in ends) assertEquals(false, SyncJob.outcome(end))
        assertFalse(SyncJob.outcome(SyncStatus.Done(1, SyncReport.EMPTY))!!)
        assertTrue(SyncJob.outcome(SyncStatus.Pending(""))!!)
    }
}
