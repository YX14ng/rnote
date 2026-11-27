package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The library reloads after any sync that did something, so it never shows what is gone. */
class SyncReportTest {
    @Test
    fun anEmptySyncChangesNothing() {
        assertFalse(SyncReport.EMPTY.didAnything)
    }

    @Test
    fun anyWorkCounts() {
        assertTrue(SyncReport.EMPTY.copy(uploaded = 1).didAnything)
        assertTrue(SyncReport.EMPTY.copy(downloaded = 1).didAnything)
        assertTrue(SyncReport.EMPTY.copy(moved = 1).didAnything)
        assertTrue(SyncReport.EMPTY.copy(deleted = 1).didAnything)
        assertTrue(SyncReport.EMPTY.copy(conflicts = listOf("a.rnote")).didAnything)
        assertTrue(SyncReport.EMPTY.copy(changedHere = listOf("a.rnote")).didAnything)
    }

    @Test
    fun aNotebookMovedHereIsFollowedToItsNewPlace() {
        val report = SyncReport.EMPTY.copy(movedHere = mapOf("Orga/a.rnote" to "b.rnote", "b.rnote" to "F/c.rnote"))

        assertEquals("F/c.rnote", report.movedTo("Orga/a.rnote"))
        assertNull(report.movedTo("x.rnote"))
    }

    @Test
    fun movesInACircleEnd() {
        val report = SyncReport.EMPTY.copy(movedHere = mapOf("a.rnote" to "b.rnote", "b.rnote" to "a.rnote"))

        assertEquals("b.rnote", report.movedTo("a.rnote"))
    }
}
