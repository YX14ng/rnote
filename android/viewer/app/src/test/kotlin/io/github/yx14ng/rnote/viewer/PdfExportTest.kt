package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Test

/** The name an exported PDF gets. */
class PdfExportTest {
    @Test
    fun theNotebooksNameWithPdfInsteadOfRnote() {
        assertEquals("Algebra 2.pdf", PdfExport.fileName("Algebra 2.rnote"))
        assertEquals("Algebra 2.pdf", PdfExport.fileName("Algebra 2"))
    }

    @Test
    fun charactersFileSystemsRejectBecomeDashes() {
        assertEquals("TP 1-2 - parte A.pdf", PdfExport.fileName("TP 1/2 : parte A"))
    }

    @Test
    fun aNameWithNothingLeftGetsTheAppsName() {
        assertEquals("Rnote.pdf", PdfExport.fileName("  .rnote"))
    }
}
