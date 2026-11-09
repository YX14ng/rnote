package io.github.yx14ng.rnote.viewer

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.net.Uri
import android.widget.Toast

/** Content copied from a document: Rnote stroke content (JSON) or plain text. */
data class ClipContent(val mimeType: String, val data: String) {
    companion object {
        /** `StrokeContent::MIME_TYPE` in rnote-engine. */
        const val STROKE_CONTENT_MIME_TYPE = "application/rnote-stroke-content"

        /** How serialized stroke content starts. */
        private const val STROKE_CONTENT_PREFIX = "{\"strokes\":"

        /**
         * Whether [text] is stroke content that lost its MIME type on the way, e.g. through a
         * clipboard synced with another system as plain text.
         */
        fun looksLikeStrokeContent(text: String): Boolean = text.startsWith(STROKE_CONTENT_PREFIX)

        /** Decodes the `"<mime type>\n<data>"` string from `jni_bridge/clipboard.rs`. */
        fun decode(encoded: String?): ClipContent? {
            val separator = encoded?.indexOf('\n') ?: return null
            if (separator < 0) {
                return null
            }
            return ClipContent(encoded.substring(0, separator), encoded.substring(separator + 1))
        }
    }
}

/**
 * Copy, cut and paste through the system clipboard.
 *
 * Strokes go on the clipboard as Rnote stroke content, which only this app and the desktop
 * format understand; text goes as plain text. Pasting also takes plain text and images from
 * other apps. Pasted content is centered in the view; [onToolChanged] gets the tool the
 * engine switched to for it.
 */
internal class ClipboardActions(
    private val context: Context,
    private val canvas: NoteCanvasView,
    private val renderer: DocumentRenderer,
    private val onToolChanged: (Tool) -> Unit,
    private val onError: (Throwable) -> Unit,
) {
    private val clipboard = context.getSystemService(ClipboardManager::class.java)

    fun copy() {
        renderer.query({ it.copy() }) { result -> result.onSuccess(::put).onFailure(onError) }
    }

    fun cut() {
        canvas.applyEdit { document ->
            val (content, state) = document.cut()
            canvas.post { put(content) }
            state
        }
    }

    fun paste() {
        val clip = clipboard.primaryClip
        if (clip == null || clip.itemCount == 0) {
            Toast.makeText(context, R.string.clipboard_empty, Toast.LENGTH_SHORT).show()
            return
        }
        val item = clip.getItemAt(0)
        val (x, y) = canvas.documentCenter()
        val uri = item.uri
        val itemText = item.text?.toString()
        when {
            itemText != null && (
                clip.description.hasMimeType(ClipContent.STROKE_CONTENT_MIME_TYPE) ||
                    ClipContent.looksLikeStrokeContent(itemText)
                ) -> pasteWith(Tool.SELECTOR) { it.pasteStrokeContent(itemText, x, y) }
            uri != null && isImage(uri) -> {
                val resolver = context.contentResolver
                // Read on the document thread, which also keeps the main thread free.
                pasteWith(Tool.SELECTOR) { document ->
                    val bytes = resolver.openInputStream(uri)?.use { it.readBytes() }
                        ?: error("Could not read the pasted image")
                    document.pasteImage(bytes, x, y)
                }
            }
            else -> {
                val text = item.coerceToText(context)?.toString().orEmpty()
                if (text.isEmpty()) {
                    Toast.makeText(context, R.string.clipboard_empty, Toast.LENGTH_SHORT).show()
                    return
                }
                pasteWith(Tool.TYPEWRITER) { it.pasteText(text, x, y) }
            }
        }
    }

    /** Runs [paste]; once it succeeded, reports the [tool] the engine switched to. */
    private fun pasteWith(tool: Tool, paste: (NativeDocument) -> EditState) {
        canvas.applyEdit { document ->
            val state = paste(document)
            canvas.post { onToolChanged(tool) }
            state
        }
    }

    private fun isImage(uri: Uri): Boolean = context.contentResolver.getType(uri)?.startsWith("image/") == true

    private fun put(content: ClipContent?) {
        if (content == null) {
            Toast.makeText(context, R.string.nothing_selected, Toast.LENGTH_SHORT).show()
            return
        }
        val label = context.getString(R.string.clipboard_label)
        val clip = if (content.mimeType == ClipContent.STROKE_CONTENT_MIME_TYPE) {
            ClipData(ClipDescription(label, arrayOf(content.mimeType)), ClipData.Item(content.data))
        } else {
            ClipData.newPlainText(label, content.data)
        }
        clipboard.setPrimaryClip(clip)
    }
}
