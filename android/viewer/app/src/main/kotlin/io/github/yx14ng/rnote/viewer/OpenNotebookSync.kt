package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.net.Uri

/**
 * Keeps the editor's notebook in step with what a sync does to it in the library: a move
 * on another device moves it here, and a newer version reopens it unless it has unsaved
 * changes, where saving makes a conflict copy instead.
 */
internal class OpenNotebookSync(
    private val context: Context,
    private val saver: DocumentSaver,
    private val modified: () -> Boolean,
    /** The notebook is at another path now: the bar shows its name. */
    private val onMoved: () -> Unit,
    /** Opens the library notebook at the path again, where the view was. */
    private val reopen: (String) -> Unit,
    /** The notebook is gone: back to the library. */
    private val close: () -> Unit,
) {
    /** A sync ended: see whether it moved or wrote the open notebook. */
    fun onStatus(status: SyncStatus) {
        val path = saver.libraryPath ?: return
        if (status !is SyncStatus.Done) return
        val moved = status.report.movedTo(path)
        when {
            moved != null -> follow(path, moved)
            path in status.report.changedHere -> check(path)
        }
    }

    /** The editor goes on with the notebook at its new place, so saving never recreates the old one. */
    private fun follow(from: String, to: String) {
        val file = LibraryStore.forApp(context).notebookFile(to) ?: return
        InkIndexFiles.move(context, from, to)
        saver.libraryPath = to
        saver.currentUri = Uri.fromFile(file)
        saver.documentName = file.name
        onMoved()
        // Its content may have changed too.
        check(to)
    }

    /**
     * If the file at [path] isn't what the editor read or wrote: without unsaved changes it
     * reopens, or closes if it's gone; with them, saving decides (a conflict copy).
     */
    private fun check(path: String) {
        val known = saver.knownSha256
        val store = LibraryStore.forApp(context)
        store.run({ store.notebookFile(path)?.takeIf { it.exists() }?.readBytes()?.let(DocumentSaver::sha256Hex) }) done@{ result ->
            val current = result.getOrElse { return@done }
            if (saver.libraryPath != path || saver.knownSha256 != known || modified() || current == known) return@done
            if (current != null) reopen(path) else close()
        }
    }
}
