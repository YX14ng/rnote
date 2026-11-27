package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Which sheets a PDF export takes, as the dialog chose them (0-based, both ends included). */
class PdfOptionsTest {
    @Test
    fun allSheetsIsNoRange() {
        assertNull(PdfOptions.sheets(PdfOptions.Sheets.ALL, current = 2, from = 0, to = 4, count = 5))
    }

    @Test
    fun theCurrentSheetIsJustThatOne() {
        assertEquals(2..2, PdfOptions.sheets(PdfOptions.Sheets.CURRENT, current = 2, from = 0, to = 4, count = 5))
    }

    @Test
    fun aChosenRangeIsKeptInOrderAndInsideTheNotebook() {
        assertEquals(1..3, PdfOptions.sheets(PdfOptions.Sheets.RANGE, current = 0, from = 1, to = 3, count = 5))
        // Picked backwards: the same sheets.
        assertEquals(1..3, PdfOptions.sheets(PdfOptions.Sheets.RANGE, current = 0, from = 3, to = 1, count = 5))
        assertEquals(3..4, PdfOptions.sheets(PdfOptions.Sheets.RANGE, current = 0, from = 3, to = 9, count = 5))
    }
}
