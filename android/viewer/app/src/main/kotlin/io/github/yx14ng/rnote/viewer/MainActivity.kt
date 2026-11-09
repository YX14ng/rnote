package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.Gravity
import android.view.Menu
import android.view.MenuItem
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.WindowInsets
import android.widget.FrameLayout
import android.widget.TextView
import android.widget.Toast
import android.window.OnBackInvokedCallback
import android.window.OnBackInvokedDispatcher

/** Opens, creates, edits and saves `.rnote` documents. */
class MainActivity : Activity() {
    private val renderer = DocumentRenderer()
    private lateinit var canvasView: NoteCanvasView
    private lateinit var messageView: TextView

    private var documentOpen = false
    private var currentUri: Uri? = null
    private var documentName = ""
    private var editState = EditState.CLEAN

    // Until the user picks a color, it follows the document background.
    private var colorChosen = false

    private var backCallback: OnBackInvokedCallback? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        canvasView = NoteCanvasView(this).apply {
            renderer = this@MainActivity.renderer
            onError = ::showError
            onEditState = ::updateEditState
        }
        messageView = TextView(this).apply {
            gravity = Gravity.CENTER
            textSize = MESSAGE_TEXT_SIZE_SP
            setText(R.string.empty_hint)
        }
        val root = FrameLayout(this).apply {
            addView(canvasView, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
            addView(messageView, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Edge-to-edge is enforced from Android 15 on: keep content out of the bars.
            root.setOnApplyWindowInsetsListener { view, insets ->
                val bars = insets.getInsets(WindowInsets.Type.systemBars())
                view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
                insets
            }
        }
        setContentView(root)
        handleIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    override fun onCreateOptionsMenu(menu: Menu): Boolean {
        menu.add(Menu.NONE, MENU_UNDO, 0, R.string.action_undo)
            .setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
        menu.add(Menu.NONE, MENU_REDO, 1, R.string.action_redo)
            .setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
        menu.add(GROUP_TOOLS, MENU_PEN, 2, R.string.action_pen)
            .setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
        menu.add(GROUP_TOOLS, MENU_ERASER, 3, R.string.action_eraser)
            .setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
        menu.setGroupCheckable(GROUP_TOOLS, true, true)

        val colors = menu.addSubMenu(Menu.NONE, MENU_COLOR, 4, R.string.action_color)
        BrushPalette.colors.forEachIndexed { index, option ->
            colors.add(GROUP_COLORS, MENU_COLOR_FIRST + index, index, option.label)
        }
        colors.setGroupCheckable(GROUP_COLORS, true, true)
        val widths = menu.addSubMenu(Menu.NONE, MENU_WIDTH, 5, R.string.action_width)
        BrushPalette.widths.forEachIndexed { index, option ->
            widths.add(GROUP_WIDTHS, MENU_WIDTH_FIRST + index, index, option.label)
        }
        widths.setGroupCheckable(GROUP_WIDTHS, true, true)

        menu.add(Menu.NONE, MENU_NEW, 6, R.string.action_new)
        menu.add(Menu.NONE, MENU_OPEN, 7, R.string.action_open)
        menu.add(Menu.NONE, MENU_SAVE, 8, R.string.action_save)
        menu.add(Menu.NONE, MENU_SAVE_AS, 9, R.string.action_save_as)
        return true
    }

    override fun onPrepareOptionsMenu(menu: Menu): Boolean {
        menu.findItem(MENU_UNDO).isEnabled = documentOpen && editState.canUndo
        menu.findItem(MENU_REDO).isEnabled = documentOpen && editState.canRedo
        for (id in listOf(MENU_PEN, MENU_ERASER, MENU_COLOR, MENU_WIDTH, MENU_SAVE, MENU_SAVE_AS)) {
            menu.findItem(id).isEnabled = documentOpen
        }
        // Action bar items don't show check marks, so mark the active tool in its title.
        for ((id, tool, label) in listOf(
            Triple(MENU_PEN, Tool.BRUSH, R.string.action_pen),
            Triple(MENU_ERASER, Tool.ERASER, R.string.action_eraser),
        )) {
            val active = canvasView.tool == tool
            menu.findItem(id).isChecked = active
            menu.findItem(id).title = if (active) "✓ ${getString(label)}" else getString(label)
        }
        BrushPalette.colors.forEachIndexed { index, option ->
            menu.findItem(MENU_COLOR_FIRST + index).isChecked = option.value == canvasView.brush.color
        }
        BrushPalette.widths.forEachIndexed { index, option ->
            menu.findItem(MENU_WIDTH_FIRST + index).isChecked = option.value == canvasView.brush.width
        }
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        val id = item.itemId
        when {
            id == MENU_UNDO -> canvasView.applyEdit(NativeDocument::undo)
            id == MENU_REDO -> canvasView.applyEdit(NativeDocument::redo)
            id == MENU_PEN -> canvasView.tool = Tool.BRUSH
            id == MENU_ERASER -> canvasView.tool = Tool.ERASER
            id in MENU_COLOR_FIRST until MENU_COLOR_FIRST + BrushPalette.colors.size -> {
                colorChosen = true
                applyBrush(canvasView.brush.copy(color = BrushPalette.colors[id - MENU_COLOR_FIRST].value))
            }
            id in MENU_WIDTH_FIRST until MENU_WIDTH_FIRST + BrushPalette.widths.size ->
                applyBrush(canvasView.brush.copy(width = BrushPalette.widths[id - MENU_WIDTH_FIRST].value))
            id == MENU_NEW -> confirmDiscard(::newDocument)
            id == MENU_OPEN -> confirmDiscard { startActivityForResult(DocumentFiles.openIntent(), REQUEST_OPEN) }
            id == MENU_SAVE -> save()
            id == MENU_SAVE_AS -> saveAs()
            else -> return super.onOptionsItemSelected(item)
        }
        invalidateOptionsMenu()
        return true
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data
        if (resultCode != RESULT_OK || uri == null) {
            return
        }
        when (requestCode) {
            REQUEST_OPEN -> openDocument(uri)
            REQUEST_SAVE_AS -> {
                currentUri = uri
                documentName = DocumentFiles.displayName(contentResolver, uri) ?: documentName
                writeTo(uri)
            }
        }
    }

    // Only reached before Android 13: from 13 on the manifest opts into OnBackInvokedCallback,
    // which updateBackHandling() registers. No AndroidX here, so no OnBackPressedDispatcher.
    @SuppressLint("GestureBackNavigation")
    @Deprecated("Only used before Android 13; later versions use OnBackInvokedCallback.")
    override fun onBackPressed() {
        if (editState.modified) {
            confirmDiscard { finish() }
        } else {
            @Suppress("DEPRECATION")
            super.onBackPressed()
        }
    }

    override fun onDestroy() {
        renderer.close()
        super.onDestroy()
    }

    private fun handleIntent(intent: Intent) {
        val uri = intent.data
        if (intent.action == Intent.ACTION_VIEW && uri != null) {
            confirmDiscard { openDocument(uri) }
        }
    }

    private fun openDocument(uri: Uri) {
        showMessage(getString(R.string.loading))
        renderer.open({ DocumentFiles.read(contentResolver, uri) }, brush = null) { result ->
            result
                .onSuccess { info ->
                    currentUri = uri
                    documentName = DocumentFiles.displayName(contentResolver, uri)
                        ?: uri.lastPathSegment
                        ?: getString(R.string.untitled_file_name)
                    showOpenedDocument(info)
                }
                .onFailure(::showError)
        }
    }

    private fun newDocument() {
        renderer.createEmpty(brush = null) { result ->
            result
                .onSuccess { info ->
                    currentUri = null
                    documentName = getString(R.string.untitled_file_name)
                    showOpenedDocument(info)
                }
                .onFailure(::showError)
        }
    }

    private fun showOpenedDocument(info: DocumentInfo) {
        documentOpen = true
        messageView.visibility = View.GONE
        if (!colorChosen) {
            canvasView.brush = canvasView.brush.copy(color = BrushPalette.contrastingColor(info.backgroundColor))
        }
        // Queued before any pen input, so the first stroke already uses it.
        renderer.setBrush(canvasView.brush)
        canvasView.showDocument(info.bounds)
        updateEditState(info.editState)
    }

    private fun applyBrush(brush: Brush) {
        canvasView.brush = brush
        if (documentOpen) {
            renderer.setBrush(brush)
        }
    }

    private fun save() {
        val uri = currentUri
        if (uri == null) {
            saveAs()
        } else {
            writeTo(uri)
        }
    }

    private fun saveAs() {
        startActivityForResult(DocumentFiles.createIntent(documentName), REQUEST_SAVE_AS)
    }

    private fun writeTo(uri: Uri) {
        renderer.save(documentName, { bytes -> DocumentFiles.write(contentResolver, uri, bytes) }) { result ->
            result
                .onSuccess { state ->
                    updateEditState(state)
                    Toast.makeText(this, R.string.saved, Toast.LENGTH_SHORT).show()
                }
                .onFailure { error ->
                    Log.e(TAG, "Saving to $uri failed", error)
                    if (error is SecurityException) {
                        // Opened read-only, e.g. from another app: ask where to save instead.
                        Toast.makeText(this, R.string.save_read_only, Toast.LENGTH_LONG).show()
                        saveAs()
                    } else {
                        val message = error.message ?: error.toString()
                        Toast.makeText(this, getString(R.string.save_failed, message), Toast.LENGTH_LONG).show()
                    }
                }
        }
    }

    private fun updateEditState(state: EditState) {
        editState = state
        title = if (state.modified) "• $documentName" else documentName
        invalidateOptionsMenu()
        updateBackHandling()
    }

    /** Asks before throwing away unsaved changes; runs [action] right away if there are none. */
    private fun confirmDiscard(action: () -> Unit) {
        if (!editState.modified) {
            action()
            return
        }
        AlertDialog.Builder(this)
            .setMessage(R.string.discard_message)
            .setPositiveButton(R.string.discard) { _, _ -> action() }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    /** From Android 13 on, intercept back only while there are unsaved changes. */
    private fun updateBackHandling() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
            return
        }
        val registered = backCallback
        if (editState.modified && registered == null) {
            val callback = OnBackInvokedCallback { confirmDiscard(::finish) }
            onBackInvokedDispatcher.registerOnBackInvokedCallback(
                OnBackInvokedDispatcher.PRIORITY_DEFAULT,
                callback,
            )
            backCallback = callback
        } else if (!editState.modified && registered != null) {
            onBackInvokedDispatcher.unregisterOnBackInvokedCallback(registered)
            backCallback = null
        }
    }

    private fun showMessage(text: String) {
        messageView.visibility = View.VISIBLE
        messageView.text = text
    }

    private fun showError(error: Throwable) {
        Log.e(TAG, "Document operation failed", error)
        val message = getString(R.string.open_failed, error.message ?: error.toString())
        if (documentOpen) {
            Toast.makeText(this, message, Toast.LENGTH_LONG).show()
        } else {
            showMessage(message)
        }
    }

    private companion object {
        const val TAG = "RnoteViewer"
        const val MESSAGE_TEXT_SIZE_SP = 18f

        const val REQUEST_OPEN = 1
        const val REQUEST_SAVE_AS = 2

        const val GROUP_TOOLS = 1
        const val GROUP_COLORS = 2
        const val GROUP_WIDTHS = 3

        const val MENU_UNDO = 1
        const val MENU_REDO = 2
        const val MENU_PEN = 3
        const val MENU_ERASER = 4
        const val MENU_COLOR = 5
        const val MENU_WIDTH = 6
        const val MENU_NEW = 7
        const val MENU_OPEN = 8
        const val MENU_SAVE = 9
        const val MENU_SAVE_AS = 10
        const val MENU_COLOR_FIRST = 100
        const val MENU_WIDTH_FIRST = 200
    }
}
