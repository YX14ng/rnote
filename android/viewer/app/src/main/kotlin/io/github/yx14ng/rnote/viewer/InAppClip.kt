package io.github.yx14ng.rnote.viewer

import java.util.UUID

/**
 * Keeps a copy too big for the system clipboard, which travels through a binder call limited to
 * about 1 MB (a page with a PDF or images is more). The system clipboard gets a small marker
 * instead; pasting it gives the copy back, while copying anything else, anywhere, replaces the
 * marker and with it the copy.
 */
internal class InAppClip {
    private var content: ClipContent? = null
    private var marker: String? = null

    /** Keeps [copy]; returns the marker to put on the system clipboard. */
    fun keep(copy: ClipContent): String {
        val token = UUID.randomUUID().toString()
        content = copy
        marker = token
        return token
    }

    /**
     * The copy behind the system clipboard's item, if it is this app's marker ([isMarker],
     * the item's MIME type) with the latest token ([text]).
     */
    fun lookup(isMarker: Boolean, text: String?): ClipContent? =
        if (isMarker && text != null && text == marker) content else null

    companion object {
        /** The marker's MIME type. */
        const val MARKER_MIME_TYPE = "application/x-rnote-in-app-clip"

        /**
         * Copies longer than this stay in the app. A parcel holds two bytes per character, and the
         * binder's 1 MB is shared with the rest of the call.
         */
        const val SYSTEM_LIMIT_CHARS = 200_000

        /** One for the whole app, so a copy survives the editor being recreated. */
        val shared = InAppClip()

        fun tooBig(copy: ClipContent): Boolean = copy.data.length > SYSTEM_LIMIT_CHARS
    }
}
