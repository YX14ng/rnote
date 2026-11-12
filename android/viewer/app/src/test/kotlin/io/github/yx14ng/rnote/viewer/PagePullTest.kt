package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Dragging past the end of a document of pages for a second adds a page, like StarNote. */
class PagePullTest {
    @Test
    fun pullingPastTheEndForASecondAddsAPage() {
        val pull = PagePull()
        assertFalse(pull.onDrag(dx = 0f, dy = 12f, atEnd = true, now = 0))
        assertEquals(0f, pull.progress!!, 1e-6f)
        assertFalse(pull.onTick(atEnd = true, now = 500))
        assertEquals(0.5f, pull.progress!!, 1e-6f)
        assertTrue(pull.onTick(atEnd = true, now = 1000))
        assertNull(pull.progress)
    }

    @Test
    fun aTouchAddsOnePageAtMost() {
        val pull = PagePull()
        pull.onDrag(dx = 0f, dy = 12f, atEnd = true, now = 0)
        assertTrue(pull.onTick(atEnd = true, now = 1000))
        assertFalse(pull.onDrag(dx = 0f, dy = 12f, atEnd = true, now = 1100))
        assertFalse(pull.onTick(atEnd = true, now = 3000))
        assertNull(pull.progress)

        pull.onLift()
        pull.onDrag(dx = 0f, dy = 12f, atEnd = true, now = 4000)
        assertTrue(pull.onTick(atEnd = true, now = 5000))
    }

    @Test
    fun liftingEarlyAddsNothing() {
        val pull = PagePull()
        pull.onDrag(dx = 0f, dy = 12f, atEnd = true, now = 0)
        pull.onLift()
        assertFalse(pull.onTick(atEnd = true, now = 1500))
        assertNull(pull.progress)
    }

    @Test
    fun leavingTheEndCancelsThePull() {
        val pull = PagePull()
        pull.onDrag(dx = 0f, dy = 12f, atEnd = true, now = 0)
        assertFalse(pull.onDrag(dx = 0f, dy = -30f, atEnd = false, now = 400))
        assertNull(pull.progress)
        assertFalse(pull.onTick(atEnd = true, now = 1200))
    }

    @Test
    fun onlyAPullTowardsTheEndStartsIt() {
        val pull = PagePull()
        pull.onDrag(dx = 0f, dy = -12f, atEnd = true, now = 0)
        assertNull(pull.progress)
        pull.onDrag(dx = 30f, dy = 4f, atEnd = true, now = 0)
        assertNull(pull.progress)
        pull.onDrag(dx = 0f, dy = 12f, atEnd = false, now = 0)
        assertNull(pull.progress)
    }

    @Test
    fun cancellingStopsThePullButAnotherDragRestartsIt() {
        val pull = PagePull()
        pull.onDrag(dx = 0f, dy = 12f, atEnd = true, now = 0)
        pull.cancel()
        assertFalse(pull.onTick(atEnd = true, now = 1000))
        pull.onDrag(dx = 0f, dy = 12f, atEnd = true, now = 1000)
        assertTrue(pull.onTick(atEnd = true, now = 2000))
    }
}
