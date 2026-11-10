package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.net.Uri
import org.json.JSONObject
import java.util.Locale

/**
 * Copies a notebook picked with the system picker into the library, in [folder], with a
 * suffix if its name is taken. A Xournal++ notebook is also converted to `.rnote`.
 */
internal class LibraryImport(private val context: Context, private val store: LibraryStore, private val folder: String?) {
    fun import(uri: Uri, done: (Result<Unit>) -> Unit) {
        val resolver = context.contentResolver
        val fileName = DocumentFiles.displayName(resolver, uri) ?: uri.lastPathSegment.orEmpty()
        val extension = fileName.substringAfterLast('.', "").lowercase(Locale.ROOT)
        if (extension != "rnote" && extension != "xopp") {
            done(Result.failure(IllegalArgumentException(context.getString(R.string.library_import_unsupported))))
            return
        }
        store.run({
            val bytes = DocumentFiles.read(resolver, uri)
            val request = LibraryStore.request("unique_name", "folder" to folder, "file_name" to fileName)
            val path = JSONObject(NativeBridge.libraryCall(store.root.path, request.toString())).getString("path")
            store.file(path).writeBytes(bytes)
            if (extension == "xopp") {
                NativeBridge.libraryCall(store.root.path, LibraryStore.request("import_xopp", "path" to path).toString())
            }
            Unit
        }, done)
    }
}
