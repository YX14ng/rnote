package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.AlertDialog
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import io.github.yx14ng.rnote.viewer.NewNotebookOptions.dp
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Inserts the pages of a PDF into the open notebook, after the sheet the user chose: asks
 * which pages and how to keep them, reads them on a thread of its own with progress and
 * Cancel, and inserts them all at once, as one change that undo takes back.
 */
internal class PageInserter(
    private val activity: Activity,
    private val renderer: DocumentRenderer,
    private val canvas: NoteCanvasView,
    private val onError: (Throwable) -> Unit,
) {
    private val main = Handler(Looper.getMainLooper())

    /** Inserts pages of the PDF at [uri] before sheet [at]. */
    fun start(uri: Uri, at: Int) {
        PdfPassword(activity, onError).open({ DocumentFiles.read(activity.contentResolver, uri) }) { bytes, password, pages ->
            askWhich(pages) { first, end, vector -> read(bytes, password, first, end, vector, at) }
        }
    }

    /** Asks for the pages (1-based in the form) and how to keep them. */
    private fun askWhich(pages: Int, onChosen: (first: Int, end: Int, vector: Boolean) -> Unit) {
        val form = FormDialog(activity, R.string.insert_pdf_title)
        val from = form.addNumber(R.string.insert_pdf_from, 1.0)
        val to = form.addNumber(R.string.insert_pdf_to, pages.toDouble())
        val kind = form.addChoice(
            R.string.import_pdf_kind,
            form.labels(listOf(R.string.import_pdf_vector, R.string.import_pdf_bitmap)),
            selected = 0,
        )
        form.show {
            val first = (from.text.replace(",", ".").toDoubleOrNull()?.toInt() ?: 1).coerceIn(1, pages)
            val last = (to.text.replace(",", ".").toDoubleOrNull()?.toInt() ?: pages).coerceIn(first, pages)
            onChosen(first - 1, last, kind() == 0)
        }
    }

    private fun read(bytes: ByteArray, password: String?, first: Int, end: Int, vector: Boolean, at: Int) {
        val total = end - first
        val cancelled = AtomicBoolean(false)
        val bar = ProgressBar(activity, null, android.R.attr.progressBarStyleHorizontal).apply { max = total }
        val label = TextView(activity).apply { textSize = 15f }
        val box = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(activity, 24f)
            setPadding(pad, dp(activity, 8f), pad, 0)
            addView(label)
            addView(bar)
        }
        val progress = AlertDialog.Builder(activity)
            .setTitle(R.string.insert_pdf_progress)
            .setView(box)
            .setCancelable(false)
            .setNegativeButton(android.R.string.cancel) { _, _ -> cancelled.set(true) }
            .show()
        val show = { done: Int ->
            bar.progress = done
            label.text = activity.getString(R.string.import_pdf_progress_page, (done + 1).coerceAtMost(total), total)
        }
        show(0)
        // Opened on the document's thread, which knows the size of the sheets.
        renderer.query({ document -> document.pdfPagesBegin(bytes, password, first, end, vector) }) { begun ->
            val pages = begun.getOrElse { error ->
                progress.dismiss()
                onError(error)
                return@query
            }
            Thread({
                val result = runCatching {
                    var done = 0
                    while (!cancelled.get() && NativeBridge.pdfPagesNext(pages)) {
                        done += 1
                        val shown = done
                        main.post { show(shown) }
                    }
                }
                main.post {
                    if (!activity.isDestroyed) progress.dismiss()
                    if (result.isFailure || cancelled.get() || activity.isDestroyed) {
                        NativeBridge.pdfPagesClose(pages)
                        result.exceptionOrNull()?.let(onError)
                        return@post
                    }
                    canvas.applyEdit { document -> document.insertPdfPages(pages, at) }
                }
            }, "rnote-pdf-insert").start()
        }
    }
}
