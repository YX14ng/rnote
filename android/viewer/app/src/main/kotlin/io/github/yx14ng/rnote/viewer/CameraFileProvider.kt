package io.github.yx14ng.rnote.viewer

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import java.io.File
import java.io.FileNotFoundException

/**
 * Lends the system camera the one file it writes a photo to, through a `content://` URI the
 * app grants it while taking the picture. Without AndroidX there is no `FileProvider`, and
 * nothing else is shared: any other URI is refused.
 */
class CameraFileProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        val file = fileFor(uri) ?: throw FileNotFoundException(uri.toString())
        file.parentFile?.mkdirs()
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.parseMode(mode))
    }

    override fun getType(uri: Uri): String? = if (fileFor(uri) != null) MIME_TYPE else null

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, args: Array<out String>?, sort: String?): Cursor? {
        val file = fileFor(uri) ?: return null
        val columns = projection ?: arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE)
        return MatrixCursor(columns, 1).apply {
            addRow(columns.map { column ->
                when (column) {
                    OpenableColumns.DISPLAY_NAME -> file.name
                    OpenableColumns.SIZE -> file.length()
                    else -> null
                }
            })
        }
    }

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null

    override fun update(uri: Uri, values: ContentValues?, selection: String?, args: Array<out String>?): Int = 0

    override fun delete(uri: Uri, selection: String?, args: Array<out String>?): Int = 0

    private fun fileFor(uri: Uri): File? {
        val context = context ?: return null
        return if (uri.authority == authority(context.packageName) && uri.path == "/$PHOTO_NAME") photoFile(context) else null
    }

    companion object {
        private const val PHOTO_NAME = "photo.jpg"
        private const val MIME_TYPE = "image/jpeg"

        fun authority(packageName: String) = "$packageName.camera"

        /** Where the camera writes the photo, in the app's cache. */
        fun photoFile(context: android.content.Context) = File(File(context.cacheDir, "camera"), PHOTO_NAME)

        /** The URI the camera writes [photoFile] through. */
        fun photoUri(context: android.content.Context): Uri =
            Uri.Builder().scheme("content").authority(authority(context.packageName)).path(PHOTO_NAME).build()
    }
}
