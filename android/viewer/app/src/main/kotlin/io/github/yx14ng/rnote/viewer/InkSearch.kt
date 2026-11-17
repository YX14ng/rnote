package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.Log
import org.json.JSONArray
import java.io.File

/**
 * Searching the open notebook: the search field in the editor's bar, the results marked on
 * the page and moving between them. The writing is searched in what [ink] has read so far;
 * the text boxes always.
 */
internal class InkSearch(
    private val context: Context,
    private val canvas: NoteCanvasView,
    private val renderer: DocumentRenderer,
    private val ink: InkIndexer,
    private val topBar: EditorTopBar,
) {
    private val bar = SearchBar(
        context,
        onQuery = ::search,
        onPrevious = { move(MatchCursor::previous) },
        onNext = { move(MatchCursor::next) },
        onLanguage = ink::setLanguage,
        onClose = ::close,
    )
    private val main = Handler(Looper.getMainLooper())
    private val refresh = Runnable { search(query, quiet = true) }
    private val cursor = MatchCursor()
    private var matches: List<List<DoubleArray>> = emptyList()
    private var query = ""
    private var showing = false

    init {
        ink.onChanged = {
            bar.setLanguage(ink.language)
            // A reading run ended: what was found may have changed.
            if (showing && ink.pending == 0) search(query, quiet = true)
            showStatus()
        }
    }

    /**
     * The notebook changed: the results are looked for again once editing stops (erased
     * words, edited text boxes), without moving the view.
     */
    fun onEdited() {
        if (!showing || query.isBlank()) return
        main.removeCallbacks(refresh)
        main.postDelayed(refresh, EDIT_PAUSE_MS)
    }

    /** Shows the search field in the editor's bar. */
    fun open() {
        showing = true
        topBar.showSearch(bar)
        bar.setLanguage(ink.language)
        ink.retry()
        showStatus()
        bar.focus()
    }

    /** Puts the field away and clears the marks. */
    fun close() {
        main.removeCallbacks(refresh)
        if (!showing) return
        showing = false
        bar.clear()
        topBar.showSearch(null)
        query = ""
        matches = emptyList()
        cursor.reset(0)
        canvas.showMatches(matches, null)
    }

    /**
     * Looks for [text]. A [quiet] search (the notebook changed, not the query) keeps the
     * result shown and doesn't move the view.
     */
    private fun search(text: String, quiet: Boolean = false) {
        query = text
        if (text.isBlank()) {
            show(emptyList(), quiet)
            return
        }
        // Without an index (a notebook with no file yet), only the text boxes.
        val index = (ink.indexFile ?: File(File(context.cacheDir, "ink"), "none.json")).path
        renderer.query({ document -> parse(document.inkSearch(index, text)) }) { result ->
            if (text != query || !showing) return@query
            result
                .onSuccess { found -> show(found, quiet) }
                .onFailure { error -> Log.w(TAG, "Searching failed", error) }
        }
    }

    private fun show(found: List<List<DoubleArray>>, quiet: Boolean) {
        matches = found
        if (quiet) {
            cursor.resize(found.size)
            canvas.showMatches(matches, cursor.current)
            showStatus()
        } else {
            cursor.reset(found.size)
            showCurrent()
        }
    }

    private fun move(step: (MatchCursor) -> Unit) {
        step(cursor)
        showCurrent()
    }

    private fun showCurrent() {
        canvas.showMatches(matches, cursor.current)
        cursor.current?.let { canvas.revealMatch(matches[it]) }
        showStatus()
    }

    /** Where the results stand and, while the writing can't all be searched, why. */
    private fun showStatus() {
        val (at, total) = cursor.position
        val count = when {
            query.isBlank() -> null
            total == 0 -> context.getString(R.string.search_none)
            else -> context.getString(R.string.search_count, at, total)
        }
        val language = context.getString(ink.language.label)
        val reading = if (ink.indexFile == null) null else when (val state = ink.modelState) {
            ModelState.Checking -> null
            ModelState.Downloading -> context.getString(R.string.ink_downloading, language)
            ModelState.Offline -> context.getString(R.string.ink_offline)
            is ModelState.Failed -> context.getString(R.string.ink_failed, state.message)
            ModelState.Ready -> if (ink.pending > 0) context.resources.getQuantityString(R.plurals.ink_pending, ink.pending, ink.pending) else null
        }
        bar.setStatus(listOfNotNull(count, reading).joinToString(" · "))
    }

    private fun parse(json: String): List<List<DoubleArray>> {
        val list = JSONArray(json)
        return (0 until list.length()).map { i ->
            val rects = list.getJSONObject(i).getJSONArray("rects")
            (0 until rects.length()).map { j ->
                val r = rects.getJSONArray(j)
                DoubleArray(4) { r.getDouble(it) }
            }
        }
    }

    private companion object {
        const val TAG = "RnoteSearch"
        const val EDIT_PAUSE_MS = 700L
    }
}
