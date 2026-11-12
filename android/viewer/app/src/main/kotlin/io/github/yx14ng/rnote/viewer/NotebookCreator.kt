package io.github.yx14ng.rnote.viewer

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/**
 * Makes new notebooks in the library: the file is written once, with its whole contents, so
 * the library never shows half a notebook. Everything runs on the library's thread; the
 * results come back on the main thread with the new notebook's path.
 */
internal class NotebookCreator(private val context: Context, private val store: LibraryStore) {
    private val defaultsFile = File(context.filesDir, DEFAULTS_FILE)

    /** The setup the next notebook of [folder] starts with. */
    fun defaults(folder: String?, done: (Result<NotebookSetup>) -> Unit) =
        store.run({ defaultsOf(folder) }, done)

    /**
     * A notebook named [title] (or "Untitled") in [folder], set up as [setup], with the
     * tags [tags]; [setup] is remembered for the next notebook of [folder].
     */
    fun create(folder: String?, title: String, setup: NotebookSetup, tags: List<String>, done: (Result<String>) -> Unit) =
        store.run({
            val path = write(folder, title) { name -> NativeBridge.newNotebook(setup.toJson(), name) }
            describe(path, setup.cover, tags)
            NativeBridge.rememberNotebookDefaults(defaultsFile.path, folder, setup.toJson())
            path
        }, done)

    /** An "Untitled" notebook in [folder] as the last one there, with [layout] if given. */
    fun quick(folder: String?, layout: SetupLayout? = null, done: (Result<String>) -> Unit) =
        store.run({
            val last = defaultsOf(folder)
            val setup = if (layout == null) last else last.copy(layout = layout)
            val path = write(folder, "") { name -> NativeBridge.newNotebook(setup.toJson(), name) }
            describe(path, setup.cover, emptyList())
            path
        }, done)

    /** A notebook named [name] in [folder], as the last one there, with [image] on its first page. */
    fun fromImage(folder: String?, name: String, image: () -> ByteArray, done: (Result<String>) -> Unit) =
        store.run({
            val setup = defaultsOf(folder)
            val path = write(folder, name) { fileName -> NativeBridge.imageNotebook(setup.toJson(), image(), fileName) }
            describe(path, setup.cover, emptyList())
            path
        }, done)

    /** A notebook named [name] in [folder] with the `.rnote` [bytes] made elsewhere, and [cover]. */
    fun save(folder: String?, name: String, cover: Int?, bytes: ByteArray, done: (Result<String>) -> Unit) =
        store.run({
            val path = write(folder, name) { bytes }
            describe(path, cover, emptyList())
            path
        }, done)

    /**
     * Writes a notebook named [title] (or "Untitled") in [folder] with the bytes [make]
     * gives for its file name; returns its path. Runs on the library's thread.
     */
    fun write(folder: String?, title: String, make: (String) -> ByteArray): String {
        val stem = title.trim().ifEmpty { context.getString(R.string.untitled) }
        val request = LibraryStore.request("new_notebook_path", "folder" to folder, "stem" to stem)
        store.root.mkdirs()
        val path = JSONObject(NativeBridge.libraryCall(store.root.path, request.toString())).getString("path")
        val bytes = make(store.file(path).name)
        // Whole or not at all, so a sync never sees half of it; never replaces a notebook,
        // even one that appeared since the path was chosen.
        NativeBridge.libraryCreateNotebook(store.root.path, path, bytes)
        return path
    }

    private fun describe(path: String, cover: Int?, tags: List<String>) {
        if (cover != null) {
            call(LibraryStore.request("set_cover", "path" to path, "color" to NotebookSetup.unsigned(cover)))
        }
        if (tags.isNotEmpty()) call(LibraryStore.request("set_tags", "path" to path, "tags" to JSONArray(tags)))
    }

    private fun call(request: JSONObject) {
        NativeBridge.libraryCall(store.root.path, request.toString())
    }

    private fun defaultsOf(folder: String?): NotebookSetup =
        NotebookSetup.fromJson(NativeBridge.notebookDefaults(defaultsFile.path, folder))

    private companion object {
        /** Outside the library, so it's never synced. */
        const val DEFAULTS_FILE = "notebook_defaults.json"
    }
}
