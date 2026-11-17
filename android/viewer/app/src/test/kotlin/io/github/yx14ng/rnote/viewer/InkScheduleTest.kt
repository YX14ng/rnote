package io.github.yx14ng.rnote.viewer

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
        schedule.finished()
        assertTrue(schedule.due(now = 10_000))

        schedule.close()
        assertFalse(schedule.due(now = 20_000))
    }

    @Test
    fun aRunUsesUpTheRequest() {
        val schedule = InkSchedule()
        schedule.opened(now = 0)
        schedule.started()
        schedule.finished()

        assertFalse(schedule.due(now = 10_000))
    }
}
