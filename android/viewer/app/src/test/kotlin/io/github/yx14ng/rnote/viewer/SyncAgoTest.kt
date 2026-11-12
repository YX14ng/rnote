package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** "Synced …" says how long ago in words of the app's language, never "0 minutes ago". */
class SyncAgoTest {
    @Test
    fun underAMinuteIsJustNow() {
        assertEquals(SyncAgo.JustNow, SyncAgo.of(0))
        assertEquals(SyncAgo.JustNow, SyncAgo.of(59_999))
    }

    @Test
    fun aClockThatWentBackIsJustNow() {
        assertEquals(SyncAgo.JustNow, SyncAgo.of(-5_000))
    }

    @Test
    fun underAnHourCountsMinutes() {
        assertEquals(SyncAgo.Minutes(1), SyncAgo.of(60_000))
        assertEquals(SyncAgo.Minutes(59), SyncAgo.of(3_599_999))
    }

    @Test
    fun underADayCountsHours() {
        assertEquals(SyncAgo.Hours(1), SyncAgo.of(3_600_000))
        assertEquals(SyncAgo.Hours(23), SyncAgo.of(86_399_999))
    }

    @Test
    fun laterItGivesTheDate() {
        assertEquals(SyncAgo.OnDate, SyncAgo.of(86_400_000))
    }
}
