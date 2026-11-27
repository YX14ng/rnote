package io.github.yx14ng.rnote.viewer

/** Selecting notebooks of the library by holding their cards. */
internal object CardSelection {
    /** The selection after holding the card of [path]; [selection] is null when not selecting. */
    fun held(selection: Set<String>?, path: String): Set<String> = when {
        selection == null -> setOf(path)
        path in selection -> selection - path
        else -> selection + path
    }
}
