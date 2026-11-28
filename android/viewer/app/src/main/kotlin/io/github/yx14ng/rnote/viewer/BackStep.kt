package io.github.yx14ng.rnote.viewer

/** What back does in the editor: close what is open on top of the page, else leave it. */
internal enum class BackStep {
    OVERLAY,
    SEARCH,
    PAGES,
    SELECTION,
    LEAVE;

    companion object {
        /** The first thing back closes, the topmost first, given what is open. */
        fun first(overlay: Boolean, search: Boolean, pages: Boolean, selection: Boolean): BackStep = when {
            overlay -> OVERLAY
            search -> SEARCH
            pages -> PAGES
            selection -> SELECTION
            else -> LEAVE
        }
    }
}
