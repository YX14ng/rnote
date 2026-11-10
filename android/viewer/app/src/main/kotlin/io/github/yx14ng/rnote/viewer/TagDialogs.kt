package io.github.yx14ng.rnote.viewer

import android.app.AlertDialog
import android.content.Context

/** Dialogs to create tags and to choose the tags of notebooks. */
internal class TagDialogs(
    private val context: Context,
    private val store: LibraryStore,
    private val onChanged: () -> Unit,
    private val onError: (Throwable) -> Unit,
) {
    /** Asks for a name, then a color, and creates the tag; [onCreated] gets its id. */
    fun create(onCreated: (String) -> Unit = {}) {
        askName(R.string.library_new_tag, "") { name ->
            ColorPicker.show(context, R.string.library_tag_color, DEFAULT_COLOR) { color ->
                store.call(LibraryStore.request("create_tag", "name" to name, "color" to unsigned(color))) { result ->
                    result.onSuccess { onChanged(); onCreated(it.getString("id")) }.onFailure(onError)
                }
            }
        }
    }

    fun rename(tag: LibraryTag) = askName(R.string.library_rename, tag.name) { name -> update(tag, name, tag.color) }

    fun recolor(tag: LibraryTag) =
        ColorPicker.show(context, R.string.library_tag_color, tag.color) { color -> update(tag, tag.name, color) }

    fun delete(tag: LibraryTag) {
        AlertDialog.Builder(context)
            .setMessage(context.getString(R.string.library_delete_tag_message, tag.name))
            .setPositiveButton(R.string.library_delete) { _, _ ->
                store.call(LibraryStore.request("delete_tag", "id" to tag.id)) { result ->
                    result.onSuccess { onChanged() }.onFailure(onError)
                }
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    /**
     * Chooses the tags of the notebooks at [paths]: checked are the ones all of them have.
     * The chosen set replaces their tags.
     */
    fun assign(paths: List<String>, snapshot: LibrarySnapshot) {
        val tags = snapshot.tags
        val notebooks = snapshot.notebooks.filter { it.path in paths }
        val checked = BooleanArray(tags.size) { i -> notebooks.isNotEmpty() && notebooks.all { tags[i].id in it.tags } }
        AlertDialog.Builder(context)
            .setTitle(R.string.library_tags)
            .setMultiChoiceItems(tags.map { it.name }.toTypedArray(), checked) { _, which, isChecked ->
                checked[which] = isChecked
            }
            .setPositiveButton(R.string.apply) { _, _ ->
                val chosen = tags.filterIndexed { i, _ -> checked[i] }.map { it.id }
                val requests = paths.map { LibraryStore.request("set_tags", "path" to it, "tags" to org.json.JSONArray(chosen)) }
                store.callAll(requests) { result -> result.onSuccess { onChanged() }.onFailure(onError) }
            }
            .setNeutralButton(R.string.library_new_tag) { _, _ -> create() }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    /** Asks for a name, starting with [initial]; [onName] gets it trimmed and not empty. */
    fun askName(title: Int, initial: String, onName: (String) -> Unit) {
        val form = FormDialog(context, title)
        val field = form.addText(R.string.library_name, initial)
        form.show { if (field.text.isNotEmpty()) onName(field.text) }
    }

    private fun update(tag: LibraryTag, name: String, color: Int) {
        val request = LibraryStore.request("update_tag", "id" to tag.id, "name" to name, "color" to unsigned(color))
        store.call(request) { result -> result.onSuccess { onChanged() }.onFailure(onError) }
    }

    private companion object {
        val DEFAULT_COLOR = Graphite.ACCENT

        /** Colors travel as unsigned ARGB, as the library stores them. */
        fun unsigned(color: Int): Long = color.toLong() and 0xFFFFFFFFL
    }
}
