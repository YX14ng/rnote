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
internal class ThumbnailCache private constructor(context: Context, private val store: LibraryStore) {
    private val dir = File(context.cacheDir, "thumbnails")
    private val memory = object : LruCache<String, Bitmap>(MEMORY_BYTES) {
        override fun sizeOf(key: String, value: Bitmap) = value.byteCount
    }
    private val workers = Executors.newFixedThreadPool(WORKERS)
    private val main = Handler(Looper.getMainLooper())

    /** Rendering failed for these keys; they aren't tried again until the notebook changes. */
    private val failed = mutableSetOf<String>()

    /** Deletes the thumbnails of notebooks other than [notebooks]: renamed, moved or trashed. */
    fun forgetOthers(notebooks: List<Notebook>) {
        val keys = notebooks.map { it.key }
        workers.execute {
            val names = dir.list()?.toList().orEmpty()
            for (name in ThumbnailFiles.orphans(names, keys)) File(dir, name).delete()
        }
    }

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
            // One per thread: two workers may render the same notebook at once.
            val temporary = File(dir, "${notebook.key}.${Thread.currentThread().id}.tmp")
            temporary.writeBytes(png)
            temporary.renameTo(file)
            // Each save of a notebook used to leave one more thumbnail behind.
            dir.list()?.let { names ->
                ThumbnailFiles.olderVersions(names.toList(), notebook.key).forEach { File(dir, it).delete() }
            }
        }
        return BitmapFactory.decodeFile(file.path)
    }

    companion object {
        private const val TAG = "RnoteViewer"
        private const val SIZE_PX = 360
        private const val WORKERS = 2
        private const val MEMORY_BYTES = 32 * 1024 * 1024

        private var shared: ThumbnailCache? = null

        /**
         * The app's one cache: the library screen is made again on every rotation, and a
         * cache each time left its threads behind and drew every thumbnail again.
         */
        fun forApp(context: Context): ThumbnailCache = shared
            ?: ThumbnailCache(context.applicationContext, LibraryStore.forApp(context)).also { shared = it }
    }
}

/** Names of the thumbnail files, `<path key>-<version key>.png` (see `thumbnail_key`). */
internal object ThumbnailFiles {
    /** The thumbnails among [names] of notebooks other than those with [liveKeys]. */
    fun orphans(names: List<String>, liveKeys: List<String>): List<String> {
        val notebooks = liveKeys.map { it.substringBefore('-') }.toSet()
        return names.filter { it.endsWith(".png") && it.substringBefore('-') !in notebooks }
    }

    /** The thumbnails among [names] of earlier versions of the notebook whose key is [current]. */
    fun olderVersions(names: List<String>, current: String): List<String> {
        val notebook = current.substringBefore('-', missingDelimiterValue = "").ifEmpty { return emptyList() }
        return names.filter { it.startsWith("$notebook-") && it.endsWith(".png") && it != "$current.png" }
    }
}
