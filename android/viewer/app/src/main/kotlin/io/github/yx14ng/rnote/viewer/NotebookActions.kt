package io.github.yx14ng.rnote.viewer

import android.app.AlertDialog
import android.content.Context
import android.view.View
import android.widget.PopupMenu
import org.json.JSONObject

/**
 * What can be done to notebooks, folders and the trash in the library: the menus of each,
 * and the dialogs they open. Every change ends with [onChanged], so the screen reloads.
 */
internal class NotebookActions(
    private val context: Context,
    private val store: LibraryStore,
    private val snapshot: () -> LibrarySnapshot,
    private val onChanged: () -> Unit,
    private val onError: (Throwable) -> Unit,
) {
    val tags = TagDialogs(context, store, onChanged, onError)

    /** The menu of a notebook's card; in the trash, restore or delete for good. */
    fun notebookMenu(notebook: Notebook, anchor: View, inTrash: Boolean) {
        val menu = PopupMenu(context, anchor)
        if (inTrash) {
            menu.menu.add(R.string.library_restore).setOnMenuItemClickListener { restore(listOf(notebook)); true }
            menu.menu.add(R.string.library_delete_forever).setOnMenuItemClickListener { deleteForever(listOf(notebook)); true }
        } else {
            menu.menu.add(R.string.library_rename).setOnMenuItemClickListener { rename(notebook); true }
            menu.menu.add(R.string.library_move).setOnMenuItemClickListener { moveTo(listOf(notebook.path)); true }
            menu.menu.add(R.string.library_tags).setOnMenuItemClickListener { tags.assign(listOf(notebook.path), snapshot()); true }
            menu.menu.add(R.string.library_cover).setOnMenuItemClickListener { chooseCover(notebook); true }
            menu.menu.add(R.string.library_duplicate).setOnMenuItemClickListener {
                run(LibraryStore.request("duplicate", "path" to notebook.path)); true
            }
            menu.menu.add(R.string.library_to_trash).setOnMenuItemClickListener { trash(listOf(notebook.path)); true }
        }
        menu.show()
    }

    /** Chooses the cover color of [notebook], or no cover. */
    fun chooseCover(notebook: Notebook) {
        val colors = CoverColor.entries
        val labels = colors.map { context.getString(it.label) } + context.getString(R.string.library_no_cover)
        AlertDialog.Builder(context)
            .setTitle(R.string.library_cover)
            .setItems(labels.toTypedArray()) { _, which ->
                val color = colors.getOrNull(which)?.let { NotebookSetup.unsigned(it.argb) }
                run(LibraryStore.request("set_cover", "path" to notebook.path, "color" to color))
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    fun folderMenu(folder: String, anchor: View) {
        val menu = PopupMenu(context, anchor)
        menu.menu.add(R.string.library_rename).setOnMenuItemClickListener {
            tags.askName(R.string.library_rename, folder) { name ->
                run(LibraryStore.request("rename_folder", "folder" to folder, "new_name" to name))
            }
            true
        }
        menu.menu.add(R.string.library_delete).setOnMenuItemClickListener { deleteFolder(folder); true }
        menu.show()
    }

    fun tagMenu(tag: LibraryTag, anchor: View) {
        val menu = PopupMenu(context, anchor)
        menu.menu.add(R.string.library_rename).setOnMenuItemClickListener { tags.rename(tag); true }
        menu.menu.add(R.string.library_tag_color).setOnMenuItemClickListener { tags.recolor(tag); true }
        menu.menu.add(R.string.library_delete).setOnMenuItemClickListener { tags.delete(tag); true }
        menu.show()
    }

    fun createFolder() =
        tags.askName(R.string.library_new_folder, "") { name -> run(LibraryStore.request("create_folder", "name" to name)) }

    fun rename(notebook: Notebook) =
        tags.askName(R.string.library_rename, notebook.name) { name ->
            run(LibraryStore.request("rename", "path" to notebook.path, "new_name" to name))
        }

    /** Moves the notebooks at [paths] to a folder chosen from a list, or out of folders. */
    fun moveTo(paths: List<String>) {
        val folders = snapshot().folders.sortedWith(String.CASE_INSENSITIVE_ORDER)
        val choices = listOf(context.getString(R.string.library_uncategorized)) + folders
        AlertDialog.Builder(context)
            .setTitle(R.string.library_move)
            .setItems(choices.toTypedArray()) { _, which ->
                val folder = if (which == 0) null else folders[which - 1]
                runAll(paths.map { LibraryStore.request("move_to", "path" to it, "folder" to folder) })
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    fun trash(paths: List<String>) = runAll(paths.map { LibraryStore.request("trash", "path" to it) })

    fun restore(notebooks: List<Notebook>) = runAll(notebooks.map { LibraryStore.request("restore", "id" to trashId(it)) })

    fun deleteForever(notebooks: List<Notebook>) = confirm(
        context.resources.getQuantityString(R.plurals.library_delete_forever_message, notebooks.size, notebooks.size),
    ) { runAll(notebooks.map { LibraryStore.request("delete_forever", "id" to trashId(it)) }) }

    fun emptyTrash() = confirm(context.getString(R.string.library_empty_trash_message)) {
        run(LibraryStore.request("empty_trash"))
    }

    private fun deleteFolder(folder: String) {
        val count = snapshot().notebooks.count { it.folder == folder }
        val delete = { run(LibraryStore.request("delete_folder", "folder" to folder)) }
        if (count == 0) {
            delete()
        } else {
            confirm(context.resources.getQuantityString(R.plurals.library_delete_folder_message, count, folder, count)) { delete() }
        }
    }

    private fun confirm(message: String, action: () -> Unit) {
        AlertDialog.Builder(context)
            .setMessage(message)
            .setPositiveButton(R.string.library_delete) { _, _ -> action() }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun run(request: JSONObject) = store.call(request) { result -> result.onSuccess { onChanged() }.onFailure(::fail) }

    private fun runAll(requests: List<JSONObject>) = store.callAll(requests) { result ->
        result.onSuccess { onChanged() }.onFailure(::fail)
    }

    /** Part of a batch may have happened before the failure: reload either way. */
    private fun fail(error: Throwable) {
        onChanged()
        onError(error)
    }

    private fun trashId(notebook: Notebook) = notebook.path.removePrefix("$TRASH_PATH/")
}
