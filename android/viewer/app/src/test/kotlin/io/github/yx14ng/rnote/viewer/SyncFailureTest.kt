package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Why a sync failed, said in the user's language rather than as Rust's error chain. */
class SyncFailureTest {
    @Test
    fun eachKindHasItsReason() {
        assertEquals(R.string.sync_reason_drive_full, SyncFailure.reason("drive_full"))
        assertEquals(R.string.sync_reason_drive_refused, SyncFailure.reason("drive_refused"))
        assertEquals(R.string.sync_reason_changed, SyncFailure.reason("changed_meanwhile"))
        assertEquals(R.string.sync_reason_local, SyncFailure.reason("local_files"))
    }

    @Test
    fun unknownKindsAreUnexpected() {
        assertEquals(R.string.error_unexpected, SyncFailure.reason("unexpected"))
        assertEquals(R.string.error_unexpected, SyncFailure.reason(""))
    }

    @Test
    fun onlyAFileChangedMeanwhileIsTriedAgainSoon() {
        assertTrue(SyncFailure.retrySoon("changed_meanwhile"))
        assertFalse(SyncFailure.retrySoon("drive_full"))
    }

    @Test
    fun skippedEntriesAreShownByTheirNotebook() {
        assertEquals(listOf("Orga/a.rnote", "b.rnote"), SyncFailure.skippedNotebooks(listOf("Orga/a.rnote: Writing failed", "b.rnote: x: y")))
    }
}
