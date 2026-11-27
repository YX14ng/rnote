package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The hand holding the S Pen rests on the screen: its touches don't navigate or undo. */
class PalmGuardTest {
    @Test
    fun withoutAPenFingersNavigate() {
        assertTrue(PalmGuard().fingersAllowed(at = 10_000))
    }

    @Test
    fun whileThePenHoversFingersAreThePalm() {
        val guard = PalmGuard()
        guard.penHover(inRange = true, at = 1_000)

        assertFalse(guard.fingersAllowed(at = 1_000 + PalmGuard.HOVER_STALE_MS - 1))
    }

    @Test
    fun aHoverThatNeverSaidItEndedExpires() {
        val guard = PalmGuard()
        guard.penHover(inRange = true, at = 1_000)

        assertTrue(guard.fingersAllowed(at = 1_000 + PalmGuard.HOVER_STALE_MS + 1))
    }

    @Test
    fun aMomentAfterThePenLeavesFingersNavigateAgain() {
        val guard = PalmGuard()
        guard.penHover(inRange = true, at = 1_000)
        guard.penHover(inRange = false, at = 2_000)

        assertFalse(guard.fingersAllowed(at = 2_000 + PalmGuard.GRACE_MS - 1))
        assertTrue(guard.fingersAllowed(at = 2_000 + PalmGuard.GRACE_MS + 1))
    }

    @Test
    fun liftingThePenKeepsThePalmIgnoredForAMoment() {
        val guard = PalmGuard()
        guard.penTouched(at = 5_000)

        assertFalse(guard.fingersAllowed(at = 5_100))
    }
}
