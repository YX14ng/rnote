package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** When the open notebook saves itself while it is being edited. */
class AutosaveScheduleTest {
    @Test
    fun itSavesAShortWhileAfterTheLastEdit() {
        val schedule = AutosaveSchedule()
        schedule.edited(1_000)
        schedule.edited(2_000)

        assertEquals(2_000 + AutosaveSchedule.IDLE_MS, schedule.dueAt())
    }

    @Test
    fun writingWithoutPauseStillSavesNowAndThen() {
        val schedule = AutosaveSchedule()
        var now = 0L
        while (now < 10 * AutosaveSchedule.MAX_UNSAVED_MS) {
            schedule.edited(now)
            now += 500
        }

        assertEquals(AutosaveSchedule.MAX_UNSAVED_MS, schedule.dueAt())
    }

    @Test
    fun nothingIsDueOnceSavedUntilTheNextEdit() {
        val schedule = AutosaveSchedule()
        schedule.edited(1_000)

        schedule.saved()

        assertNull(schedule.dueAt())
        schedule.edited(9_000)
        assertEquals(9_000 + AutosaveSchedule.IDLE_MS, schedule.dueAt())
    }
}
