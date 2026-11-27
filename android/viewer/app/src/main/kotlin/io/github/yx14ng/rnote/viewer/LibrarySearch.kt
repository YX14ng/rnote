package io.github.yx14ng.rnote.viewer

import androidx.annotation.StringRes
import java.text.Normalizer
import java.util.Locale

/** Finding notebooks by name, what an empty library screen says, and its section as text. */
internal object LibrarySearch {
    private val MARKS = Regex("\\p{Mn}+")

    /** Whether [name] has every word of [query], ignoring case and accents. */
    fun matches(name: String, query: String): Boolean {
        val words = fold(query).split(' ').filter { it.isNotEmpty() }
        val folded = fold(name)
        return words.all { it in folded }
    }

    /** The notebooks out of the trash whose names match [query]. */
    fun find(snapshot: LibrarySnapshot, query: String): List<Notebook> =
        snapshot.notebooks.filter { matches(it.name, query) }

    /** What the screen says when it shows no notebook, or null when the "New" card is there. */
    @StringRes
    fun emptyMessage(section: LibrarySection, query: String): Int? = when {
        query.isNotBlank() -> R.string.library_empty_search
        section == LibrarySection.Trash -> R.string.library_trash_is_empty
        section is LibrarySection.Folder -> R.string.library_empty_folder
        section is LibrarySection.Tag -> R.string.library_empty_tag
        else -> null
    }

    /** [section] as text, to keep it while the screen is recreated; see [sectionFrom]. */
    fun sectionText(section: LibrarySection): String = when (section) {
        LibrarySection.All -> "all"
        LibrarySection.Uncategorized -> "uncategorized"
        is LibrarySection.Folder -> "folder:${section.name}"
        is LibrarySection.Tag -> "tag:${section.id}"
        LibrarySection.Trash -> "trash"
    }

    /** The section [text] from [sectionText] stands for; everything when there is none. */
    fun sectionFrom(text: String?): LibrarySection = when {
        text == null -> LibrarySection.All
        text == "uncategorized" -> LibrarySection.Uncategorized
        text == "trash" -> LibrarySection.Trash
        text.startsWith("folder:") -> LibrarySection.Folder(text.removePrefix("folder:"))
        text.startsWith("tag:") -> LibrarySection.Tag(text.removePrefix("tag:"))
        else -> LibrarySection.All
    }

    private fun fold(text: String): String =
        MARKS.replace(Normalizer.normalize(text.lowercase(Locale.ROOT), Normalizer.Form.NFD), "").trim()
}
