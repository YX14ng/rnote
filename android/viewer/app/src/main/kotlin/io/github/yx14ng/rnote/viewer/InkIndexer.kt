package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.Executors

/**
 * Reads the writing of the open notebook in the background, a fragment at a time, and keeps
 * what was read in the notebook's index: right after opening it and a few seconds after the
 * last edit (see [InkSchedule]). Only the fragments that changed are read again.
 *
 * Everything here runs on the main thread; the document is read on the renderer's thread
 * and the index is written on a thread of its own.
 */
internal class InkIndexer(
    context: Context,
    private val recognizer: InkRecognizer,
    private val renderer: DocumentRenderer,
) {
    private val main = Handler(Looper.getMainLooper())
    private val writer = Executors.newSingleThreadExecutor()
    // The application's, so a model download or a network callback never keeps the editor.
    private val app = context.applicationContext
    private val preferences = app.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
    private val connectivity = app.getSystemService(ConnectivityManager::class.java)
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private var schedule = InkSchedule()

    // Bumped on every notebook and language, so the answers of an older run are dropped.
    private var generation = 0

    /** The index of the open notebook; null for one with no file yet, searched by text only. */
    var indexFile: File? = null
        private set

    var language: InkLanguage = InkLanguage.fromTag(preferences.getString(KEY_LANGUAGE, null))
        private set

    var modelState: ModelState = ModelState.Checking
        private set

    /** How many fragments are waiting to be read. */
    var pending: Int = 0
        private set

    /** How many fragments the last run couldn't read; they are tried again later. */
    var unread: Int = 0
        private set
    private var runUnread = 0
    // The best reading of each fragment read in this run, for the context of the next one.
    private val runReadings = HashMap<String, String>()

    // Readings not written to the index yet: it's rewritten whole, so they go in batches.
    private var batch: Batch? = null

    private val tickLater = Runnable { tick() }

    /** Called whenever [modelState], [pending], [unread] or [language] change. */
    var onChanged: (() -> Unit)? = null

    /** Starts reading the notebook whose index is [file]. */
    fun open(file: File?) {
        stop()
        indexFile = file
        schedule = InkSchedule()
        if (file == null) return
        // A notebook not read yet takes the language chosen last.
        val chosen = InkLanguage.fromTag(preferences.getString(KEY_LANGUAGE, null))
        language = chosen
        // The index of a long notebook is big: its language is read off the main thread.
        Thread({
            val read = InkLanguage.fromTag(NativeBridge.inkLanguage(file.path, chosen.tag))
            main.post {
                // Another notebook may be open by now.
                if (indexFile != file) return@post
                language = read
                start()
                onChanged?.invoke()
            }
        }, "rnote-ink-language").start()
    }

    private fun start() {
        // Another notebook may be in a language whose model isn't on the device.
        modelState = ModelState.Checking
        schedule = InkSchedule()
        schedule.opened(SystemClock.uptimeMillis())
        prepareModel()
    }

    /** The notebook changed: read it again once writing stops. */
    fun onEdited() {
        if (indexFile == null) return
        schedule.edited(SystemClock.uptimeMillis())
        // One wake-up for the last edit, not one per batch of pen samples.
        main.removeCallbacks(tickLater)
        main.postDelayed(tickLater, InkSchedule.DELAY_MS + TICK_SLACK_MS)
    }

    /** Reads the notebook in [newLanguage] from now on, all of it again. */
    fun setLanguage(newLanguage: InkLanguage) {
        if (newLanguage == language) return
        language = newLanguage
        preferences.edit().putString(KEY_LANGUAGE, newLanguage.tag).apply()
        stop()
        // The index forgets the readings in the other language the next time it's read.
        if (indexFile != null) start()
        onChanged?.invoke()
    }

    /** Tries the model again if it couldn't be had, e.g. when the search opens. */
    fun retry() {
        val state = modelState
        if (indexFile != null && (state == ModelState.Offline || state is ModelState.Failed)) prepareModel()
    }

    /** Stops reading for good: the editor is gone. */
    fun close() {
        stop()
        writer.shutdown()
        recognizer.close()
    }

    private fun stop() {
        generation++
        schedule.close()
        main.removeCallbacksAndMessages(null)
        unwatchNetwork()
        // Still the readings of that notebook (the index drops them if its language changed).
        flush()
        pending = 0
        unread = 0
    }

    private fun prepareModel() {
        val run = generation
        recognizer.ensureModel(language) { state ->
            if (run != generation) return@ensureModel
            modelState = state
            onChanged?.invoke()
            if (state == ModelState.Ready) tick()
            // The model comes down as soon as there's a connection again.
            if (state == ModelState.Offline) watchNetwork()
        }
    }

    private fun watchNetwork() {
        if (networkCallback != null) return
        // Android tells about a network right away, even one this app may not use (data
        // saver, a firewall): only a connection the app has counts, or it would ask again
        // and again.
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                main.post(::retryIfOnline)
            }

            override fun onBlockedStatusChanged(network: Network, blocked: Boolean) {
                if (!blocked) main.post(::retryIfOnline)
            }
        }
        if (runCatching { connectivity.registerDefaultNetworkCallback(callback) }.isSuccess) networkCallback = callback
    }

    private fun retryIfOnline() {
        if (networkCallback == null || connectivity.activeNetwork == null) return
        unwatchNetwork()
        if (indexFile != null && modelState == ModelState.Offline) prepareModel()
    }

    private fun unwatchNetwork() {
        networkCallback?.let { callback -> runCatching { connectivity.unregisterNetworkCallback(callback) } }
        networkCallback = null
    }

    private fun tick() {
        val file = indexFile ?: return
        if (modelState != ModelState.Ready || !schedule.due(SystemClock.uptimeMillis())) return
        schedule.started()
        runUnread = 0
        runReadings.clear()
        val run = generation
        val tag = language.tag
        // Only taking the strokes out needs the document's thread, which also draws the
        // frames; finding what changed, the index and the JSON go to the writer's thread.
        renderer.query({ document -> document.inkStrokes() }) { taken ->
            val strokes = taken.getOrElse { error ->
                if (run == generation) failedListing(error, run)
                return@query
            }
            val queued = runCatching {
                writer.execute {
                    val result = runCatching { parse(NativeBridge.inkPendingOf(strokes, file.path, tag)) }
                    main.post {
                        if (run != generation) return@post
                        result
                            .onSuccess { fragments -> readNext(fragments, 0, run) }
                            .onFailure { error -> failedListing(error, run) }
                    }
                }
            }
            // Closed meanwhile: the strokes go unread and unfreed, a small loss at the end.
            queued.onFailure { error -> if (run == generation) failedListing(error, run) }
        }
    }

    private fun failedListing(error: Throwable, run: Int) {
        Log.w(TAG, "Listing the writing to read failed", error)
        finishRun(run)
    }

    private fun readNext(fragments: List<InkFragment>, at: Int, run: Int) {
        if (run != generation) return
        if (at == fragments.size) {
            finishRun(run)
            return
        }
        pending = fragments.size - at
        onChanged?.invoke()
        val fragment = fragments[at]
        val file = indexFile ?: return
        val tag = language.tag
        val before = fragment.before ?: fragment.previous?.let(runReadings::get)
        recognizer.recognize(fragment, language, preContext(before, language)) { result ->
            if (run != generation) return@recognize
            result
                .onSuccess { candidates ->
                    candidates.firstOrNull()?.let { runReadings[fragment.hash] = it }
                    val reading = JSONObject()
                        .put("hash", fragment.hash)
                        .put("rect", JSONArray(fragment.rect.toList()))
                        .put("candidates", JSONArray(candidates))
                    val current = batch ?: Batch(file, tag).also { batch = it }
                    current.items.put(reading)
                    if (current.items.length() >= BATCH_SIZE) flush()
                }
                .onFailure { error ->
                    Log.w(TAG, "Reading a fragment failed", error)
                    runUnread++
                }
            readNext(fragments, at + 1, run)
        }
    }

    private fun finishRun(run: Int) {
        val now = SystemClock.uptimeMillis()
        pending = 0
        unread = runUnread
        schedule.finished(now, unread)
        // Told once the readings are in the index, so a search that follows finds them.
        flush { if (run == generation) onChanged?.invoke() }
        // Edits during the run asked for another one; fragments it couldn't read, for a
        // later one.
        schedule.dueIn(now)?.let { wait -> main.postDelayed(tickLater, wait + TICK_SLACK_MS) }
    }

    /** Writes the batch to the index, then calls [then] on the main thread. */
    private fun flush(then: (() -> Unit)? = null) {
        val readings = batch
        batch = null
        if (readings == null && then == null) return
        writer.execute {
            if (readings != null) {
                runCatching { NativeBridge.inkAddAll(readings.file.path, readings.tag, readings.items.toString()) }
                    .onFailure { error -> Log.w(TAG, "Keeping the readings failed", error) }
            }
            if (then != null) main.post(then)
        }
    }

    private fun JSONObject.optStringOrNull(key: String): String? = if (isNull(key)) null else getString(key)

    private class Batch(val file: File, val tag: String) {
        val items = JSONArray()
    }

    private fun parse(json: String): List<InkFragment> {
        val list = JSONArray(json)
        return (0 until list.length()).map { i ->
            val item = list.getJSONObject(i)
            val rect = item.getJSONArray("rect").let { r -> DoubleArray(4) { r.getDouble(it) } }
            val strokes = item.getJSONArray("strokes").let { s ->
                (0 until s.length()).map { j ->
                    val points = s.getJSONArray(j)
                    (0 until points.length()).map { k ->
                        val p = points.getJSONArray(k)
                        doubleArrayOf(p.getDouble(0), p.getDouble(1), p.getDouble(2))
                    }
                }
            }
            InkFragment(item.getString("hash"), rect, strokes, item.optStringOrNull("before"), item.optStringOrNull("previous"))
        }
    }

    companion object {
        private const val TAG = "RnoteInk"
        private const val PREFERENCES = "ink"
        private const val KEY_LANGUAGE = "language"
        private const val TICK_SLACK_MS = 50L
        private const val BATCH_SIZE = 25
        // ML Kit looks at up to 20 characters before the writing.
        private const val PRE_CONTEXT_CHARS = 20

        /**
         * What goes before a fragment for the recognizer: the end of [before], and a space in
         * a [language] that writes its words apart.
         */
        fun preContext(before: String?, language: InkLanguage): String = when {
            before.isNullOrBlank() -> ""
            language.spaced -> before.takeLast(PRE_CONTEXT_CHARS - 1) + " "
            else -> before.takeLast(PRE_CONTEXT_CHARS)
        }

        /** The index file of the notebook known as [key] (its library path or its URI). */
        fun indexFileFor(context: Context, key: String): File = InkIndexFiles.fileFor(context, key)
    }
}
