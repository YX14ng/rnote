package io.github.yx14ng.rnote.viewer

import android.content.ContentResolver
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import java.io.IOException

/** Reading and writing documents through the Storage Access Framework. */
internal object DocumentFiles {
    /**
     * Rnote files have no registered MIME type on Android; a generic one keeps document
     * providers from renaming the file or appending another extension.
     */
    private const val MIME_TYPE = "application/octet-stream"

    /** The shared Documents folder on the device's main storage. */
    private val documentsFolder: Uri =
        DocumentsContract.buildDocumentUri("com.android.externalstorage.documents", "primary:Documents")

    /**
     * Picks a document to open. The picker starts next to [current], the open file, or in the
     * Documents folder.
     */
    fun openIntent(current: Uri?): Intent =
        Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("*/*")
            .putExtra(DocumentsContract.EXTRA_INITIAL_URI, current ?: documentsFolder)

    /** Picks where to save; starts like [openIntent]. */
    fun createIntent(suggestedName: String, current: Uri?): Intent =
        Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType(MIME_TYPE)
            .putExtra(Intent.EXTRA_TITLE, suggestedName)
            .putExtra(DocumentsContract.EXTRA_INITIAL_URI, current ?: documentsFolder)

    /** The name the provider shows for [uri], if it has one. */
    fun displayName(resolver: ContentResolver, uri: Uri): String? =
        runCatching {
            resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            }
        }.getOrNull()

    fun read(resolver: ContentResolver, uri: Uri): ByteArray =
        resolver.openInputStream(uri)?.use { it.readBytes() } ?: throw IOException("Cannot read $uri")

    /** Replaces the contents of [uri] with [bytes]. */
    fun write(resolver: ContentResolver, uri: Uri, bytes: ByteArray) {
        val output = resolver.openOutputStream(uri, "wt") ?: throw IOException("Cannot write $uri")
        output.use { it.write(bytes) }
    }
}
