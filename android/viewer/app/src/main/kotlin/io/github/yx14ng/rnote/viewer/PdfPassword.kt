package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.AlertDialog
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.widget.EditText
import android.widget.LinearLayout
import io.github.yx14ng.rnote.viewer.NewNotebookOptions.dp

/**
 * Opens a PDF picked by the user: counts its pages on a thread of its own, asking for the
 * password until it's right when it has one. Used to import a PDF as a notebook and to insert
 * one into a notebook.
 */
internal class PdfPassword(private val activity: Activity, private val onError: (Throwable) -> Unit) {
    private val main = Handler(Looper.getMainLooper())

    /** Reads [read] and counts the pages; [onOpen] gets the bytes, the password and the count. */
    fun open(read: () -> ByteArray, onOpen: (bytes: ByteArray, password: String?, pages: Int) -> Unit) {
        background(read) { bytes -> count(bytes, password = null, wrong = false, onOpen) }
    }

    private fun count(bytes: ByteArray, password: String?, wrong: Boolean, onOpen: (ByteArray, String?, Int) -> Unit) {
        background({ NativeBridge.pdfPageCount(bytes, password) }) { count ->
            if (count >= 0) {
                onOpen(bytes, password, count)
            } else {
                ask(wrong) { typed -> count(bytes, typed, wrong = true, onOpen) }
            }
        }
    }

    private fun ask(wrong: Boolean, onPassword: (String) -> Unit) {
        val field = EditText(activity).apply {
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            isSingleLine = true
        }
        val box = LinearLayout(activity).apply {
            val pad = dp(activity, 24f)
            setPadding(pad, dp(activity, 8f), pad, 0)
            addView(field, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT))
        }
        AlertDialog.Builder(activity)
            .setTitle(R.string.import_pdf_password)
            .setMessage(if (wrong) R.string.import_pdf_wrong_password else R.string.import_pdf_password_message)
            .setView(box)
            .setPositiveButton(android.R.string.ok) { _, _ -> onPassword(field.text.toString()) }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    /** Runs [work] on a thread of its own and hands its result to [done] on the main thread. */
    private fun <T> background(work: () -> T, done: (T) -> Unit) {
        Thread({
            val result = runCatching(work)
            main.post {
                if (activity.isDestroyed) return@post
                result.onSuccess(done).onFailure(onError)
            }
        }, "rnote-pdf-open").start()
    }
}
