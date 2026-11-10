package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.net.Uri
import android.util.Log
import android.widget.Toast

/**
 * Where the open document is saved, and how: to its file, through "Save as", or to a
 * recovery copy when the app goes to the background and there is no writable file.
 */
internal class DocumentSaver(
    private val context: Context,
    private val renderer: DocumentRenderer,
    private val recovery: Recovery,
    private val onSaved: (EditState) -> Unit,
    private val requestSaveAs: () -> Unit,
) {
    /** The file the document was opened from or last saved to, if any. */
    var currentUri: Uri? = null

    /** The name shown in the title and stored in the file. */
    var documentName = ""

    /** Saves to the current file, or asks where to save it; [onDone] runs once saved. */
    fun save(onDone: (() -> Unit)? = null) {
        val uri = currentUri
        if (uri == null) {
            requestSaveAs()
        } else {
            writeTo(uri, onDone = onDone)
        }
    }

    /** Saves to the file the user picked with "Save as", which becomes the current one. */
    fun saveAs(uri: Uri) {
        currentUri = uri
        documentName = DocumentFiles.displayName(context.contentResolver, uri) ?: documentName
        writeTo(uri)
    }

    /** Saves to the current file, or keeps a recovery copy if there is none. */
    fun autosave() {
        val uri = currentUri
        if (uri != null) {
            writeTo(uri, quiet = true)
        } else {
            saveRecovery()
        }
    }

    /**
     * Saves to [uri]. When [quiet], failures fall back to a recovery copy instead of asking
     * the user, since this runs while the app goes to the background.
     */
    private fun writeTo(uri: Uri, quiet: Boolean = false, onDone: (() -> Unit)? = null) {
        renderer.save(documentName, { bytes -> DocumentFiles.write(context.contentResolver, uri, bytes) }) { result ->
            result
                .onSuccess { state ->
                    recovery.clear()
                    onSaved(state)
                    if (!quiet) {
                        Toast.makeText(context, R.string.saved, Toast.LENGTH_SHORT).show()
                    }
                    onDone?.invoke()
                }
                .onFailure { error ->
                    Log.e(TAG, "Saving to $uri failed", error)
                    when {
                        quiet -> saveRecovery()
                        error is SecurityException -> {
                            // Opened read-only, e.g. from another app: ask where to save instead.
                            Toast.makeText(context, R.string.save_read_only, Toast.LENGTH_LONG).show()
                            requestSaveAs()
                        }
                        else -> {
                            val message = error.message ?: error.toString()
                            val text = context.getString(R.string.save_failed, message)
                            Toast.makeText(context, text, Toast.LENGTH_LONG).show()
                        }
                    }
                }
        }
    }

    private fun saveRecovery() {
        val name = documentName
        renderer.saveCopy(name, { bytes -> recovery.write(bytes, name) }) { result ->
            result.onFailure { error -> Log.e(TAG, "Writing the recovery copy failed", error) }
        }
    }

    private companion object {
        const val TAG = "RnoteViewer"
    }
}
