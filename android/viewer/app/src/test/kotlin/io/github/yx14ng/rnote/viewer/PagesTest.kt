package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The arithmetic of the page column. */
class PagesTest {
    @Test
    fun theCurrentPageIsTheOneInTheMiddleOfTheView() {
        // Sheets 1000 tall; the view shows 800 of the document from y = 1700.
        assertEquals(2, Pages.current(viewTop = 1700.0, visibleHeight = 800.0, sheetHeight = 1000.0, count = 5))
        assertEquals(1, Pages.current(viewTop = 1000.0, visibleHeight = 800.0, sheetHeight = 1000.0, count = 5))
    }

    @Test
    fun theCurrentPageStaysWithinTheDocument() {
        assertEquals(0, Pages.current(viewTop = -900.0, visibleHeight = 800.0, sheetHeight = 1000.0, count = 3))
        assertEquals(2, Pages.current(viewTop = 9000.0, visibleHeight = 800.0, sheetHeight = 1000.0, count = 3))
    }

    @Test
    fun removingAsksFirstOnlyWhenSomethingIsWrittenOnThePages() {
        val withContent = booleanArrayOf(true, false, false)

        assertFalse(Pages.removingLosesContent(setOf(1, 2), withContent))
        assertTrue(Pages.removingLosesContent(setOf(0, 2), withContent))
    }

    @Test
    fun theTopOfAPageIsItsIndexTimesTheSheet() {
        assertEquals(3000.0, Pages.top(page = 3, sheetHeight = 1000.0), 1e-9)
    }

    @Test
    fun theSheetsOnScreenAreTheOnesTheViewCrosses() {
        // Sheets 1000 tall; the view shows 800 from y = 1700: the end of sheet 1 and sheet 2.
        assertEquals(1..2, Pages.visible(viewTop = 1700.0, visibleHeight = 800.0, sheetHeight = 1000.0, count = 5))
        assertEquals(0..0, Pages.visible(viewTop = 100.0, visibleHeight = 800.0, sheetHeight = 1000.0, count = 5))
        assertEquals(3..4, Pages.visible(viewTop = 3900.0, visibleHeight = 5000.0, sheetHeight = 1000.0, count = 5))
    }
}
