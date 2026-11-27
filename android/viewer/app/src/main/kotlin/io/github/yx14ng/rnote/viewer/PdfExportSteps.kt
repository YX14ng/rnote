package io.github.yx14ng.rnote.viewer

/** Drawing a PDF export a sheet per step, telling how far it got, and stopping when asked. */
internal object PdfExportSteps {
    /** An export being drawn; see `PdfExportJob` in the engine. */
    interface Job {
        val total: Int

        /** Draws the next sheet; returns how many are drawn. */
        fun drawNext(): Int

        /** The PDF, once every sheet is drawn; closes the job. */
        fun finish(): ByteArray

        /** Drops the export; closes the job. */
        fun cancel()
    }

    /**
     * Draws [job] to the end and returns its PDF, calling [progress] with the sheets drawn
     * and the total before the first and after each one; or returns null, dropping it, once
     * [isCancelled]. A failure drops it too, and is thrown.
     */
    fun run(job: Job, isCancelled: () -> Boolean, progress: (Int, Int) -> Unit): ByteArray? {
        val total = job.total
        try {
            var done = 0
            progress(done, total)
            while (done < total) {
                if (isCancelled()) {
                    job.cancel()
                    return null
                }
                done = job.drawNext()
                progress(done, total)
            }
        } catch (error: Throwable) {
            job.cancel()
            throw error
        }
        return job.finish()
    }
}

/** A [PdfExportSteps.Job] of the engine; [handle] comes from `pdfExportBegin`. */
internal class NativePdfExport(private val handle: Long) : PdfExportSteps.Job {
    override val total: Int = NativeBridge.pdfExportTotal(handle)

    override fun drawNext(): Int = NativeBridge.pdfExportDrawNext(handle)

    override fun finish(): ByteArray = NativeBridge.pdfExportFinish(handle)

    override fun cancel() = NativeBridge.pdfExportCancel(handle)
}
