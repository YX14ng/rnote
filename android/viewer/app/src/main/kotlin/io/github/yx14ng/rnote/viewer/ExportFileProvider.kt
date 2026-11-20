package io.github.yx14ng.rnote.viewer

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import java.io.File
import java.io.FileNotFoundException

/**
 * Lends other apps, read only, the PDF the user chose to share, through a `content://` URI
 * granted with the share. Without AndroidX there is no `FileProvider`; only files of the
 * exports folder in the app's cache are served, by plain name.
 */
class ExportFileProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        val file = fileOf(uri) ?: throw FileNotFoundException(uri.toString())
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun getType(uri: Uri): String? = if (fileOf(uri) != null) PdfExport.MIME_TYPE else null

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, args: Array<out String>?, sort: String?): Cursor? {
        val file = fileOf(uri) ?: return null
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

    /** The exported file `uri` names, if it is one: a plain name in the exports folder. */
    private fun fileOf(uri: Uri): File? {
        val context = context ?: return null
        val name = uri.lastPathSegment ?: return null
        if (uri.authority != authority(context.packageName) || uri.pathSegments.size != 1 ||
            name.contains('/') || name.startsWith('.')
        ) {
            return null
        }
        return fileFor(context, name).takeIf { it.isFile }
    }

    companion object {
        fun authority(packageName: String) = "$packageName.exports"

        /** Where the PDF named [name] is written to be shared, in the app's cache. */
        fun fileFor(context: Context, name: String) = File(File(context.cacheDir, "exports"), name)

        fun uriFor(context: Context, name: String): Uri =
            Uri.Builder().scheme("content").authority(authority(context.packageName)).appendPath(name).build()
    }
}
