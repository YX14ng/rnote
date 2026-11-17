package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** When the writing is read: on opening, a few seconds after writing stops, one run at a time. */
class InkScheduleTest {
    @Test
    fun openingIndexesRightAway() {
        val schedule = InkSchedule()
        schedule.opened(now = 1_000)

        assertTrue(schedule.due(now = 1_000))
    }

    @Test
    fun indexingWaitsThreeSecondsAfterTheLastEdit() {
        val schedule = InkSchedule()
        schedule.edited(now = 0)
        schedule.edited(now = 1_000)

        assertFalse(schedule.due(now = 3_999))
        assertTrue(schedule.due(now = 4_000))
    }

    @Test
    fun indexerRunsOneAtATimeAndStopsOnClose() {
        val schedule = InkSchedule()
        schedule.opened(now = 0)
        schedule.started()
        schedule.edited(now = 10)

        assertFalse(schedule.due(now = 10_000))
        schedule.finished(now = 20, unread = 0)
        assertTrue(schedule.due(now = 10_000))

        schedule.close()
        assertFalse(schedule.due(now = 20_000))
    }

    @Test
    fun aRunUsesUpTheRequest() {
        val schedule = InkSchedule()
        schedule.opened(now = 0)
        schedule.started()
        schedule.finished(now = 20, unread = 0)

        assertFalse(schedule.due(now = 10_000))
    }

    @Test
    fun aRunThatCouldntReadEverythingTriesAgainLater() {
        val schedule = InkSchedule()
        schedule.opened(now = 0)
        schedule.started()
        schedule.finished(now = 1_000, unread = 2)

        assertEquals(30_000L, schedule.dueIn(now = 1_000))
        assertFalse(schedule.due(now = 30_999))
        assertTrue(schedule.due(now = 31_000))
    }

    @Test
    fun retriesWaitLongerEachTimeUpToFiveMinutes() {
        val schedule = InkSchedule()
        var now = 0L
        val waits = (1..6).map {
            schedule.started()
            schedule.finished(now, unread = 1)
            val from = now
            while (!schedule.due(now) && now - from < 1_000_000) now += 1_000
            now - from
        }

        assertEquals(listOf(30_000L, 60_000L, 120_000L, 240_000L, 300_000L, 300_000L), waits)
    }

    @Test
    fun aRunThatReadEverythingStopsRetrying() {
        val schedule = InkSchedule()
        schedule.started()
        schedule.finished(now = 0, unread = 1)
        schedule.started()
        schedule.finished(now = 30_000, unread = 0)

        assertFalse(schedule.due(now = 1_000_000))
    }

    @Test
    fun anEditDuringARunGoesBeforeTheRetry() {
        val schedule = InkSchedule()
        schedule.opened(now = 0)
        schedule.started()
        schedule.edited(now = 500)
        schedule.finished(now = 1_000, unread = 1)

        assertTrue(schedule.due(now = 3_500))
    }
}
