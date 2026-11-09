package io.github.yx14ng.rnote.viewer

import android.content.Context
import java.io.File
import java.io.IOException

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

    /** Stores [bytes]; safe to call off the main thread. */
    fun write(bytes: ByteArray, documentName: String) {
        // Write next to it and rename, so a crash mid-write never leaves a broken copy.
        val partial = File(file.parentFile, "$FILE_NAME.partial")
        partial.writeBytes(bytes)
        if (!partial.renameTo(file)) {
            throw IOException("Moving the recovery copy into place failed")
        }
        preferences.edit().putString(KEY_DOCUMENT_NAME, documentName).apply()
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
    }
}
