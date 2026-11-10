package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.util.LruCache
import android.widget.ImageView
import java.io.File
import java.util.concurrent.Executors

/**
 * Thumbnails of the library's notebooks: rendered by the engine in the background, kept in
 * memory and in the app's cache folder under the notebook's key, which changes when the
 * notebook does. A notebook that can't be rendered keeps the plain cover.
 */
internal class ThumbnailCache(context: Context, private val store: LibraryStore) {
    private val dir = File(context.cacheDir, "thumbnails")
    private val memory = object : LruCache<String, Bitmap>(MEMORY_BYTES) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount
    }
    private val workers = Executors.newFixedThreadPool(WORKERS)
    private val main = Handler(Looper.getMainLooper())

    /** Rendering failed for these keys; they aren't tried again until the notebook changes. */
    private val failed = mutableSetOf<String>()

    /** Shows [notebook]'s thumbnail in [into] when it is ready; until then, nothing. */
    fun load(notebook: Notebook, into: ImageView) {
        val key = notebook.key
        into.tag = key
        into.setImageDrawable(null)
        if (notebook.isXopp || key in failed) {
            return
        }
        memory.get(key)?.let { into.setImageBitmap(it); return }
        workers.execute {
            val bitmap = runCatching { readOrRender(notebook) }
                .onFailure { Log.w(TAG, "No thumbnail for ${notebook.path}", it) }
                .getOrNull()
            main.post {
                if (bitmap == null) {
                    failed += key
                } else {
                    memory.put(key, bitmap)
                }
                // The view may show another notebook by now.
                if (into.tag == key && bitmap != null) {
                    into.setImageBitmap(bitmap)
                }
            }
        }
    }

    private fun readOrRender(notebook: Notebook): Bitmap? {
        val file = File(dir, "${notebook.key}.png")
        if (!file.exists()) {
            val png = NativeBridge.libraryThumbnail(store.file(notebook.path).readBytes(), SIZE_PX) ?: return null
            dir.mkdirs()
            val temporary = File(dir, "${notebook.key}.tmp")
            temporary.writeBytes(png)
            temporary.renameTo(file)
        }
        return BitmapFactory.decodeFile(file.path)
    }

    private companion object {
        const val TAG = "RnoteViewer"
        const val SIZE_PX = 360
        const val WORKERS = 2
        const val MEMORY_BYTES = 32 * 1024 * 1024
    }
}
