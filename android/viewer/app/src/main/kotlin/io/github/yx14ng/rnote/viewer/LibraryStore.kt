package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.os.Handler
import android.os.Looper
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.Executors

/** A notebook in the library; [path] is relative to the library root, with `/`. */
data class Notebook(
    val path: String,
    val name: String,
    val folder: String?,
    val isXopp: Boolean,
    val modifiedMs: Long,
    val size: Long,
    val tags: List<String>,
    /** Changes whenever the thumbnail must be rendered again. */
    val key: String,
)

data class LibraryTag(val id: String, val name: String, val color: Int)

data class TrashItem(val id: String, val name: String, val originalPath: String, val deletedAtMs: Long)

/** Everything the library screen shows. */
data class LibrarySnapshot(
    val notebooks: List<Notebook>,
    val folders: List<String>,
    val tags: List<LibraryTag>,
    val trash: List<TrashItem>,
)

/**
 * The notebook library, kept by `rnote-library` in a folder: requests run one at a time on a
 * background thread, and their results come back on the main thread.
 */
internal class LibraryStore(val root: File) {
    private val worker = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())

    /** Reads the whole library. */
    fun list(done: (Result<LibrarySnapshot>) -> Unit) {
        call(JSONObject().put("op", "list")) { result -> done(result.map(::parseSnapshot)) }
    }

    /** Runs [request] (an object with an `op` and its arguments). */
    fun call(request: JSONObject, done: (Result<JSONObject>) -> Unit) {
        worker.execute {
            val result = runCatching {
                root.mkdirs()
                JSONObject(NativeBridge.libraryCall(root.path, request.toString()))
            }
            main.post { done(result) }
        }
    }

    /** The file of the notebook at the relative [path]. */
    fun file(path: String): File = File(root, path)

    companion object {
        /** The app's own library, in its private storage. */
        fun forApp(context: Context): LibraryStore = LibraryStore(File(context.filesDir, "library"))

        /** Builds a request for [op] with [args]. */
        fun request(op: String, vararg args: Pair<String, Any?>): JSONObject =
            JSONObject().put("op", op).apply { for ((name, value) in args) put(name, value ?: JSONObject.NULL) }

        private fun parseSnapshot(json: JSONObject) = LibrarySnapshot(
            notebooks = json.getJSONArray("notebooks").objects().map { item ->
                Notebook(
                    path = item.getString("path"),
                    name = item.getString("name"),
                    folder = item.optStringOrNull("folder"),
                    isXopp = item.getString("kind") == "xopp",
                    modifiedMs = item.getLong("modified_ms"),
                    size = item.getLong("size"),
                    tags = item.getJSONArray("tags").strings(),
                    key = item.getString("key"),
                )
            },
            folders = json.getJSONArray("folders").strings(),
            tags = json.getJSONArray("tags").objects().map { item ->
                LibraryTag(item.getString("id"), item.getString("name"), item.getLong("color").toInt())
            },
            trash = json.getJSONArray("trash").objects().map { item ->
                TrashItem(item.getString("id"), item.getString("name"), item.getString("original_path"), item.getLong("deleted_at_ms"))
            },
        )

        private fun JSONArray.objects(): List<JSONObject> = (0 until length()).map(::getJSONObject)

        private fun JSONArray.strings(): List<String> = (0 until length()).map(::getString)

        private fun JSONObject.optStringOrNull(name: String): String? = if (isNull(name)) null else getString(name)
    }
}
