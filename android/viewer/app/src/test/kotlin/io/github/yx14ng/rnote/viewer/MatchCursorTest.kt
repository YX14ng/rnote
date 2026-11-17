package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Moving between the results of a search: "2 of 7", wrapping at both ends. */
class MatchCursorTest {
    @Test
    fun aNewSearchStartsAtTheFirstResult() {
        val cursor = MatchCursor()
        cursor.reset(count = 7)

        assertEquals(0, cursor.current)
        assertEquals(1 to 7, cursor.position)
    }

    @Test
    fun nextAfterTheLastGoesBackToTheFirst() {
        val cursor = MatchCursor()
        cursor.reset(count = 3)
        cursor.next()
        cursor.next()

        assertEquals(3 to 3, cursor.position)
        cursor.next()
        assertEquals(1 to 3, cursor.position)
    }

    @Test
    fun previousBeforeTheFirstGoesToTheLast() {
        val cursor = MatchCursor()
        cursor.reset(count = 3)
        cursor.previous()

        assertEquals(3 to 3, cursor.position)
    }

    @Test
    fun noResultsIsZeroOfZero() {
        val cursor = MatchCursor()
        cursor.reset(count = 0)
        cursor.next()
        cursor.previous()

        assertNull(cursor.current)
        assertEquals(0 to 0, cursor.position)
    }

    @Test
    fun aQuietRefreshKeepsTheResultShown() {
        val cursor = MatchCursor()
        cursor.reset(count = 7)
        repeat(4) { cursor.next() }

        cursor.resize(count = 7)
        assertEquals(5 to 7, cursor.position)

        cursor.resize(count = 3)
        assertEquals(3 to 3, cursor.position)

        cursor.resize(count = 0)
        assertNull(cursor.current)

        cursor.resize(count = 2)
        assertEquals(1 to 2, cursor.position)
    }
}
