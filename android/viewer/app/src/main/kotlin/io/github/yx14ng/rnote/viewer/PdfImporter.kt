package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.AlertDialog
import android.content.pm.ActivityInfo
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import io.github.yx14ng.rnote.viewer.NewNotebookOptions.dp
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Imports a PDF picked with the system picker as a new notebook: asks for its password if it
 * has one, shows [ImportPdfScreen], then imports page by page on a thread of its own with
 * progress and Cancel. The notebook is written to the library only once every page is in,
 * so cancelling leaves nothing behind. [onCreated] gets the new notebook's path.
 */
internal class PdfImporter(
    private val activity: Activity,
    private val creator: NotebookCreator,
    private val onCreated: (String) -> Unit,
    private val onError: (Throwable) -> Unit,
) {
    private val main = Handler(Looper.getMainLooper())

    fun start(uri: Uri, folders: List<String>, folder: String?) {
        val fileName = DocumentFiles.displayName(activity.contentResolver, uri) ?: uri.lastPathSegment.orEmpty()
        PdfPassword(activity, onError).open({ DocumentFiles.read(activity.contentResolver, uri) }) { bytes, password, pages ->
            creator.defaults(folder) { defaults ->
                defaults.onFailure(onError).onSuccess { setup ->
                    val size = setup.pageSize ?: PageSize.A4
                    ImportPdfScreen(activity, fileName, bytes.size.toLong(), pages, folders, folder, size) { choice ->
                        import(bytes, password, setup.withPageSize(choice.pageSize, landscape = false), choice)
                    }.show()
                }
            }
        }
    }

    private fun import(bytes: ByteArray, password: String?, setup: NotebookSetup, choice: PdfChoice) {
        val total = choice.endPage - choice.firstPage
        val cancelled = AtomicBoolean(false)
        val bar = ProgressBar(activity, null, android.R.attr.progressBarStyleHorizontal).apply { max = total }
        val label = TextView(activity).apply { textSize = 15f }
        val box = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(activity, 24f)
            setPadding(pad, dp(activity, 8f), pad, 0)
            addView(label)
            addView(bar)
            addView(TextView(activity).apply {
                setText(R.string.import_pdf_cancel_hint)
                textSize = 14f
                setPadding(0, dp(activity, 10f), 0, 0)
            })
        }
        val progress = AlertDialog.Builder(activity)
            .setTitle(R.string.import_pdf_progress)
            .setView(box)
            .setCancelable(false)
            .setNegativeButton(android.R.string.cancel) { _, _ -> cancelled.set(true) }
            .show()
        val show = { done: Int ->
            bar.progress = done
            label.text = activity.getString(R.string.import_pdf_progress_page, (done + 1).coerceAtMost(total), total)
        }
        show(0)
        // Turning the tablet would recreate the screen in the middle of a long import.
        val orientation = activity.requestedOrientation
        activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LOCKED
        Thread({
            val result = runCatching {
                val handle = NativeBridge.pdfImportBegin(bytes, password, setup.toJson(), choice.firstPage, choice.endPage, choice.vector)
                try {
                    var done = 0
                    while (!cancelled.get() && NativeBridge.pdfImportNext(handle)) {
                        done += 1
                        val shown = done
                        main.post { show(shown) }
                    }
                    if (cancelled.get()) null else NativeBridge.pdfImportFinish(handle, "${choice.name}.rnote")
                } finally {
                    NativeBridge.pdfImportClose(handle)
                }
            }
            main.post {
                val alive = !activity.isDestroyed
                if (alive) {
                    progress.dismiss()
                    activity.requestedOrientation = orientation
                }
                val notebook = result.getOrElse { error -> if (alive) onError(error); return@post }
                // Saved even if the screen was recreated meanwhile: the work is done, and the
                // notebook shows up in the library; only opening it needs the screen.
                if (notebook != null && !cancelled.get()) {
                    creator.save(choice.folder, choice.name, setup.cover, notebook) { saved ->
                        if (!activity.isDestroyed) saved.onSuccess(onCreated).onFailure(onError)
                    }
                }
            }
        }, "rnote-pdf-import").start()
    }
}
