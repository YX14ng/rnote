package io.github.yx14ng.rnote.viewer

import android.content.Context
import java.io.File

/**
 * Where the handwriting index of each notebook is kept, in the app's cache: one file per
 * notebook, named after its path (or the address of a file outside the library). A moved
 * notebook takes its index along, and those of notebooks gone are swept away.
 */
internal object InkIndexFiles {
    /** How long an index of a notebook not in the library is kept unused. */
    private const val MAX_UNUSED_MS = 30L * 24 * 60 * 60 * 1000

    /** The file name of the index of the notebook known as [key]. */
    fun nameFor(key: String): String = DocumentSaver.sha256Hex(key.toByteArray()).take(16) + ".json"

    fun fileFor(context: Context, key: String): File = File(dir(context), nameFor(key))

    /**
     * The [files] (name and last change) to delete: not the index of a notebook of the library
     * ([live] names) and unused for a month, as a file outside the library may still be opened.
     */
    fun stale(files: List<Pair<String, Long>>, live: Set<String>, now: Long): List<String> =
        files.filter { (name, changed) -> name !in live && now - changed > MAX_UNUSED_MS }.map { it.first }

    /** The notebook at [from] is now at [to]: its index goes along. Any thread. */
    fun move(context: Context, from: String, to: String) {
        val source = fileFor(context, from)
        val target = fileFor(context, to)
        if (source.exists() && !target.exists()) source.renameTo(target)
    }

    /** Deletes the stale indexes, given the [paths] of the library's notebooks. Off the main thread. */
    fun sweep(context: Context, paths: List<String>) {
        val files = dir(context).listFiles().orEmpty().map { it.name to it.lastModified() }
        val live = paths.map(::nameFor).toSet()
        for (name in stale(files, live, System.currentTimeMillis())) File(dir(context), name).delete()
    }

    private fun dir(context: Context) = File(context.cacheDir, "ink")
}
