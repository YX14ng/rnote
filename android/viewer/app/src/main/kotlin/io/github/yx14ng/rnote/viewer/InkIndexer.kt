package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import org.json.JSONArray
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

    /** Called whenever [modelState], [pending] or [language] change. */
    var onChanged: (() -> Unit)? = null

    /** Starts reading the notebook whose index is [file]. */
    fun open(file: File?) {
        stop()
        indexFile = file
        schedule = InkSchedule()
        if (file == null) return
        // A notebook not read yet takes the language chosen last.
        val chosen = InkLanguage.fromTag(preferences.getString(KEY_LANGUAGE, null))
        language = InkLanguage.fromTag(NativeBridge.inkLanguage(file.path, chosen.tag))
        start()
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
        main.postDelayed(::tick, InkSchedule.DELAY_MS + TICK_SLACK_MS)
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
        pending = 0
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
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                main.post {
                    unwatchNetwork()
                    if (indexFile != null && modelState == ModelState.Offline) prepareModel()
                }
            }
        }
        if (runCatching { connectivity.registerDefaultNetworkCallback(callback) }.isSuccess) networkCallback = callback
    }

    private fun unwatchNetwork() {
        networkCallback?.let { callback -> runCatching { connectivity.unregisterNetworkCallback(callback) } }
        networkCallback = null
    }

    private fun tick() {
        val file = indexFile ?: return
        if (modelState != ModelState.Ready || !schedule.due(SystemClock.uptimeMillis())) return
        schedule.started()
        val run = generation
        val tag = language.tag
        // Read and parsed on the renderer's thread: a big notebook has a lot of points.
        renderer.query({ document -> parse(document.inkPending(file.path, tag)) }) { result ->
            if (run != generation) return@query
            result
                .onSuccess { fragments -> readNext(fragments, 0, run) }
                .onFailure { error ->
                    Log.w(TAG, "Listing the writing to read failed", error)
                    finishRun()
                }
        }
    }

    private fun readNext(fragments: List<InkFragment>, at: Int, run: Int) {
        if (run != generation) return
        pending = fragments.size - at
        onChanged?.invoke()
        if (at == fragments.size) {
            finishRun()
            return
        }
        val fragment = fragments[at]
        val file = indexFile ?: return
        val tag = language.tag
        recognizer.recognize(fragment, language, before = "") { result ->
            if (run != generation) return@recognize
            result
                .onSuccess { candidates ->
                    val json = JSONArray(candidates).toString()
                    writer.execute {
                        runCatching { NativeBridge.inkAdd(file.path, tag, fragment.hash, fragment.rect, json) }
                            .onFailure { error -> Log.w(TAG, "Keeping a reading failed", error) }
                    }
                }
                .onFailure { error -> Log.w(TAG, "Reading a fragment failed", error) }
            readNext(fragments, at + 1, run)
        }
    }

    private fun finishRun() {
        schedule.finished()
        // Edits during the run asked for another one.
        tick()
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
            InkFragment(item.getString("hash"), rect, strokes)
        }
    }

    companion object {
        private const val TAG = "RnoteInk"
        private const val PREFERENCES = "ink"
        private const val KEY_LANGUAGE = "language"
        private const val TICK_SLACK_MS = 50L

        /** The index file of the notebook known as [key] (its library path or its URI). */
        fun indexFileFor(context: Context, key: String): File =
            File(File(context.cacheDir, "ink"), DocumentSaver.sha256Hex(key.toByteArray()).take(16) + ".json")
    }
}
