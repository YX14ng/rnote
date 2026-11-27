package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** The export draws a sheet per step, tells each one, and stops when asked. */
class PdfExportStepsTest {
    private class FakeJob(override val total: Int, private val failAt: Int = -1) : PdfExportSteps.Job {
        var drawn = 0
        var finished = false
        var cancelled = false

        override fun drawNext(): Int {
            if (drawn == failAt) error("drawing failed")
            return ++drawn
        }

        override fun finish(): ByteArray {
            finished = true
            return byteArrayOf(1, 2, 3)
        }

        override fun cancel() {
            cancelled = true
        }
    }

    @Test
    fun itTellsEverySheetAndGivesThePdf() {
        val job = FakeJob(total = 3)
        val progress = mutableListOf<Pair<Int, Int>>()

        val pdf = PdfExportSteps.run(job, isCancelled = { false }) { done, total -> progress += done to total }

        assertArrayEquals(byteArrayOf(1, 2, 3), pdf)
        assertEquals(listOf(0 to 3, 1 to 3, 2 to 3, 3 to 3), progress)
        assertTrue(job.finished)
    }

    @Test
    fun cancellingStopsAtTheNextSheetAndDropsTheExport() {
        val job = FakeJob(total = 5)

        val pdf = PdfExportSteps.run(job, isCancelled = { job.drawn >= 2 }) { _, _ -> }

        assertNull(pdf)
        assertEquals(2, job.drawn)
        assertTrue(job.cancelled)
        assertTrue(!job.finished)
    }

    @Test
    fun aFailureDropsTheExportAndIsThrown() {
        val job = FakeJob(total = 3, failAt = 1)

        val thrown = runCatching { PdfExportSteps.run(job, isCancelled = { false }) { _, _ -> } }

        assertTrue(thrown.isFailure)
        assertTrue(job.cancelled)
    }
}
