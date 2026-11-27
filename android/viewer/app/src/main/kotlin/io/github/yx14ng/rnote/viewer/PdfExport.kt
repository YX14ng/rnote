package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.AlertDialog
import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Exports the open notebook as a PDF, a page per sheet: saved where the user chooses, or
 * handed to another app through Android's share sheet. The document's thread only takes
 * what to draw; the sheets are drawn, and the PDF written, on a thread of its own.
 */
internal class PdfExport(
    private val activity: Activity,
    private val renderer: DocumentRenderer,
    private val onError: (Throwable) -> Unit,
) {
    private val main = Handler(Looper.getMainLooper())

    /** Writes the PDF of the notebook named [title] to [uri], picked by the user. */
    fun saveTo(uri: Uri, title: String) {
        export(title) { pdf ->
            activity.contentResolver.openOutputStream(uri, "wt")?.use { it.write(pdf) }
                ?: error("The chosen file can't be written")
            main.post { Toast.makeText(activity, R.string.pdf_saved, Toast.LENGTH_SHORT).show() }
        }
    }

    /** Opens the share sheet with the PDF of the notebook named [title]. */
    fun share(title: String) {
        export(title) { pdf ->
            val file = ExportFileProvider.fileFor(activity, fileName(title))
            file.parentFile?.let { dir ->
                dir.mkdirs()
                // Only the latest export is lent out; older ones are left from earlier shares.
                dir.listFiles()?.filter { it != file }?.forEach(File::delete)
            }
            file.writeBytes(pdf)
            val uri = ExportFileProvider.uriFor(activity, file.name)
            val send = Intent(Intent.ACTION_SEND)
                .setType(MIME_TYPE)
                .putExtra(Intent.EXTRA_STREAM, uri)
                .putExtra(Intent.EXTRA_TITLE, file.name)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            // Some apps read the grant only from the clip data.
            send.clipData = ClipData.newRawUri(file.name, uri)
            main.post {
                if (!activity.isDestroyed) {
                    activity.startActivity(Intent.createChooser(send, activity.getString(R.string.share_pdf_title)))
                }
            }
        }
    }

    /**
     * Exports the notebook named [title] a sheet at a time behind a dialog that shows how
     * far it got and can stop it, then hands the PDF to [deliver] off the main thread.
     */
    private fun export(title: String, deliver: (ByteArray) -> Unit) {
        val cancelled = AtomicBoolean(false)
        val progress = ProgressBar(activity, null, android.R.attr.progressBarStyleHorizontal).apply { isIndeterminate = true }
        val pages = TextView(activity).apply { setText(R.string.pdf_exporting) }
        val padding = Graphite.dp(activity, 24f)
        val content = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(padding, padding / 2, padding, 0)
            addView(pages)
            addView(progress)
        }
        val dialog = AlertDialog.Builder(activity)
            .setTitle(R.string.pdf_exporting)
            .setView(content)
            .setNegativeButton(android.R.string.cancel) { _, _ -> cancelled.set(true) }
            .setCancelable(false)
            .show()
        val close = { if (!activity.isDestroyed && dialog.isShowing) dialog.dismiss() }
        renderer.query({ document -> document.beginPdfExport(title.removeSuffix(".rnote")) }) { begun ->
            begun
                .onSuccess { job ->
                    Thread({
                        runCatching {
                            val pdf = PdfExportSteps.run(job, cancelled::get) { done, total ->
                                main.post { showProgress(progress, pages, done, total) }
                            }
                            main.post { close() }
                            if (pdf != null) deliver(pdf)
                        }.onFailure { error ->
                            main.post {
                                close()
                                onError(error)
                            }
                        }
                    }, "rnote-pdf-export").start()
                }
                .onFailure { error ->
                    close()
                    onError(error)
                }
        }
    }

    private fun showProgress(progress: ProgressBar, pages: TextView, done: Int, total: Int) {
        progress.isIndeterminate = false
        progress.max = total.coerceAtLeast(1)
        progress.progress = done
        pages.text = activity.getString(R.string.pdf_export_progress, done, total)
    }

    companion object {
        const val MIME_TYPE = "application/pdf"

        /** Characters file systems and other apps reject in names. */
        private val UNSAFE = Regex("""[/\\:*?"<>|\u0000-\u001f]""")

        /** The PDF's file name for a notebook named [title]: `.rnote` becomes `.pdf`. */
        fun fileName(title: String): String {
            val name = title.removeSuffix(".rnote").replace(UNSAFE, "-").trim().trim('.')
            return "${name.ifEmpty { "Rnote" }}.pdf"
        }
    }
}
