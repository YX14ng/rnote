package io.github.yx14ng.rnote.viewer

import android.content.Context
import java.io.File

/**
 * A copy of unsaved changes that have no file to go to, e.g. a new document, written when the
 * app goes to the background so they survive Android killing it.
 */
internal class Recovery(context: Context) {
    private val file = File(context.filesDir, FILE_NAME)
    private val preferences = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)

    val exists: Boolean
        get() = file.isFile

    /** The name of the recovered document, if one was recorded. */
    val documentName: String?
        get() = preferences.getString(KEY_DOCUMENT_NAME, null)

    /** The library notebook the changes belong to, if they belong to one. */
    val libraryPath: String?
        get() = preferences.getString(KEY_LIBRARY_PATH, null)

    /** The SHA-256 of that notebook's file as the editor last read or wrote it. */
    val knownSha256: String?
        get() = preferences.getString(KEY_KNOWN_SHA256, null)

    /**
     * Stores [bytes], the unsaved changes of [documentName], which is the library notebook
     * at [libraryPath] last known as [knownSha256], if any; safe to call off the main thread.
     */
    fun write(bytes: ByteArray, documentName: String, libraryPath: String?, knownSha256: String?) {
        // Written next to it and renamed, so a crash mid-write never leaves a broken copy.
        DurableFiles.replace(file, bytes, "$FILE_NAME.partial")
        preferences.edit()
            .putString(KEY_DOCUMENT_NAME, documentName)
            .putString(KEY_LIBRARY_PATH, libraryPath)
            .putString(KEY_KNOWN_SHA256, knownSha256)
            .apply()
    }

    fun read(): ByteArray = file.readBytes()

    fun clear() {
        file.delete()
        preferences.edit().clear().apply()
    }

    private companion object {
        const val FILE_NAME = "recovery.rnote"
        const val PREFERENCES = "recovery"
        const val KEY_DOCUMENT_NAME = "document_name"
        const val KEY_LIBRARY_PATH = "library_path"
        const val KEY_KNOWN_SHA256 = "known_sha256"
    }
}
