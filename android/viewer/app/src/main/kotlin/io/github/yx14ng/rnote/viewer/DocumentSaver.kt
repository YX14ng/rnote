package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.net.Uri
import android.os.Build
import android.util.Log
import android.widget.Toast
import org.json.JSONObject
import java.security.MessageDigest

/**
 * Where the open document is saved, and how: to its file, through "Save as", or to a
 * recovery copy when the app goes to the background and there is no writable file. A
 * library notebook is only written over if it's still the file the editor read: if a sync
 * changed it meanwhile, the document goes to a conflict copy beside it.
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

    /** The open notebook's path in the library, if it is one of its notebooks. */
    var libraryPath: String? = null

    /** The SHA-256 of the library notebook as the editor last read or wrote it. */
    var knownSha256: String? = null

    /** Runs when the document went to the conflict copy at this library path instead. */
    var onConflictCopy: (String) -> Unit = {}

    private val library = LibraryStore.forApp(context)

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
        val path = libraryPath
        val expected = knownSha256
        // Written on the renderer's thread, read back on the main one.
        var written: JSONObject? = null
        val write = { bytes: ByteArray ->
            if (path == null) {
                DocumentFiles.write(context.contentResolver, uri, bytes)
            } else {
                written = JSONObject(NativeBridge.libraryWriteNotebook(library.root.path, path, expected, Build.MODEL, bytes))
            }
            Unit
        }
        renderer.save(documentName, write) { result ->
            result
                .onSuccess { state ->
                    written?.let(::applyLibraryWrite)
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

    /** Keeps what writing a library notebook did; a conflict copy becomes the open notebook. */
    private fun applyLibraryWrite(written: JSONObject) {
        knownSha256 = written.getString("sha256")
        if (written.getBoolean("conflict")) {
            val path = written.getString("path")
            val file = library.file(path)
            libraryPath = path
            currentUri = Uri.fromFile(file)
            documentName = file.name
            onConflictCopy(path)
        }
    }

    private fun saveRecovery() {
        val name = documentName
        renderer.saveCopy(name, { bytes -> recovery.write(bytes, name) }) { result ->
            result.onFailure { error -> Log.e(TAG, "Writing the recovery copy failed", error) }
        }
    }

    companion object {
        private const val TAG = "RnoteViewer"

        /** The SHA-256 of [bytes] in lower-case hex, as `rnote-sync` writes it. */
        fun sha256Hex(bytes: ByteArray): String =
            MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
    }
}
