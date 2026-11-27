package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.util.Log
import androidx.annotation.StringRes
import java.io.FileNotFoundException
import java.net.ConnectException
import java.net.SocketTimeoutException
import java.net.UnknownHostException

/**
 * What went wrong, in the terms the user is told it. Errors come from Java and from the
 * engine, whose messages are English and technical: those stay in the log.
 */
internal enum class ErrorKind(@StringRes val reason: Int) {
    DAMAGED(R.string.error_damaged),
    NO_SPACE(R.string.error_no_space),
    NOT_FOUND(R.string.error_not_found),
    NO_ACCESS(R.string.error_no_access),
    OFFLINE(R.string.error_offline),
    TOO_BIG(R.string.error_too_big),
    UNEXPECTED(R.string.error_unexpected);

    companion object {
        /** The kind of [error], judged by it and its causes. */
        fun of(error: Throwable): ErrorKind {
            val chain = generateSequence(error) { it.cause }.take(MAX_CAUSES).toList()
            val text = chain.joinToString(" ") { it.message.orEmpty() }
            return when {
                chain.any { it is OutOfMemoryError } -> TOO_BIG
                chain.any { it is UnknownHostException || it is SocketTimeoutException || it is ConnectException } ->
                    OFFLINE
                "ENOSPC" in text || "No space left" in text -> NO_SPACE
                chain.any { it is SecurityException } || "EACCES" in text || "Permission denied" in text -> NO_ACCESS
                chain.any { it is FileNotFoundException } || "ENOENT" in text -> NOT_FOUND
                DAMAGED_MARKERS.any { it in text } -> DAMAGED
                else -> UNEXPECTED
            }
        }

        private const val MAX_CAUSES = 8

        /** What the engine says when a file isn't a notebook it can read. */
        private val DAMAGED_MARKERS = listOf(
            "Loading the .rnote file failed",
            "loading RnoteFile from bytes failed",
            "decompressing bytes failed",
            "Importing the Xournal++ file failed",
            "invalid PDF",
        )
    }
}

/**
 * A refusal of the library that the user can fix, as the engine sends it:
 * `library-error:<code>:<argument>` (see `coded_message` in `library_bridge.rs`).
 */
internal data class LibraryErrorCode(val code: String, val argument: String) {
    /** The sentence for it, or null for a code this version doesn't know. */
    fun text(context: Context): String? {
        val resource = when (code) {
            "empty_name" -> R.string.library_error_empty_name
            "name_starts_with_dot" -> R.string.library_error_name_starts_with_dot
            "name_has_character" -> R.string.library_error_name_has_character
            "name_taken" -> R.string.library_error_name_taken
            "folder_has_folders" -> R.string.library_error_folder_has_folders
            "no_such_folder" -> R.string.library_error_no_such_folder
            "no_such_notebook" -> R.string.library_error_no_such_notebook
            else -> return null
        }
        return context.getString(resource, argument)
    }

    companion object {
        private const val PREFIX = "library-error:"

        fun of(error: Throwable): LibraryErrorCode? {
            val message = error.message ?: return null
            val rest = message.substringAfter(PREFIX, missingDelimiterValue = "").ifEmpty { return null }
            return LibraryErrorCode(rest.substringBefore(':'), rest.substringAfter(':', missingDelimiterValue = ""))
        }
    }
}

/** The text the user gets for an error. */
internal object ErrorText {
    /** Why it failed, e.g. "The file is damaged…"; an unexpected error keeps its message. */
    fun reason(context: Context, error: Throwable): String {
        LibraryErrorCode.of(error)?.text(context)?.let { return it }
        val kind = ErrorKind.of(error)
        val reason = context.getString(kind.reason)
        return if (kind == ErrorKind.UNEXPECTED) "$reason (${error.message ?: error.javaClass.simpleName})" else reason
    }

    /** "[action]: why", e.g. "Could not export the PDF: there is no space left"; logs [error]. */
    fun of(context: Context, @StringRes action: Int, error: Throwable): String {
        Log.e(TAG, context.getString(action), error)
        return context.getString(R.string.error_with_reason, context.getString(action), reason(context, error))
    }

    private const val TAG = "RnoteViewer"
}
