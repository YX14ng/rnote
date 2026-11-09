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
    private lateinit var recovery: Recovery

    private var settings = EditorSettings()
    private var documentOpen = false
    private var currentUri: Uri? = null
    private var documentName = ""
    private var editState = EditState.CLEAN

    // Set once the user agreed to drop the changes, so going to the background doesn't save them.
    private var changesDiscarded = false

    private var backCallback: OnBackInvokedCallback? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        recovery = Recovery(this)
        settings = EditorSettings.load(this)
        canvasView = NoteCanvasView(this).apply {
            renderer = this@MainActivity.renderer
            onError = ::showError
            onEditState = ::updateEditState
        }
        applySettingsToView()
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
        if (!handleIntent(intent) && recovery.exists) {
            restoreRecovery()
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    override fun onStop() {
        super.onStop()
        if (documentOpen && editState.modified && !changesDiscarded && !isChangingConfigurations) {
            autosave()
        }
    }

    override fun onCreateOptionsMenu(menu: Menu): Boolean {
        EditorMenu.create(menu)
        return true
    }

    override fun onPrepareOptionsMenu(menu: Menu): Boolean {
        EditorMenu.prepare(menu, MenuState(documentOpen, editState, settings), this)
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        val action = EditorMenu.actionFor(item) ?: return super.onOptionsItemSelected(item)
        when (action) {
            EditorMenu.Action.Undo -> canvasView.applyEdit(NativeDocument::undo)
            EditorMenu.Action.Redo -> canvasView.applyEdit(NativeDocument::redo)
            is EditorMenu.Action.SelectTool -> {
                changeSettings(settings.copy(tool = action.tool))
                canvasView.applyEdit { it.setTool(action.tool) }
            }
            is EditorMenu.Action.SelectColor ->
                changeBrush(settings.brush.copy(color = action.color), colorChosen = true)
            is EditorMenu.Action.SelectWidth ->
                changeBrush(settings.brush.copy(width = action.width), settings.colorChosen)
            is EditorMenu.Action.SelectShape -> {
                // Picking a shape means drawing it next.
                changeSettings(settings.copy(shape = action.shape, tool = Tool.SHAPER))
                canvasView.applyEdit { it.setShape(action.shape); it.setTool(Tool.SHAPER) }
            }
            is EditorMenu.Action.SelectBrushStyle -> {
                changeSettings(settings.copy(brushStyle = action.style))
                canvasView.applyEdit { it.setBrushStyle(action.style) }
            }
            is EditorMenu.Action.SelectEraserStyle -> {
                changeSettings(settings.copy(eraserStyle = action.style))
                canvasView.applyEdit { it.setEraserStyle(action.style) }
            }
            is EditorMenu.Action.Selection -> {
                if (action.action == SelectionAction.SELECT_ALL) {
                    // The engine switches to the selector to show the selection.
                    changeSettings(settings.copy(tool = Tool.SELECTOR))
                }
                canvasView.applyEdit { it.selection(action.action) }
            }
            EditorMenu.Action.ToggleFingerDrawing ->
                changeSettings(settings.copy(fingerDrawing = !settings.fingerDrawing))
            EditorMenu.Action.New -> confirmDiscard(::newDocument)
            EditorMenu.Action.Open ->
                confirmDiscard { startActivityForResult(DocumentFiles.openIntent(), REQUEST_OPEN) }
            EditorMenu.Action.Save -> save()
            EditorMenu.Action.SaveAs -> saveAs()
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

    /** Opens the document an ACTION_VIEW intent points to. Returns whether there was one. */
    private fun handleIntent(intent: Intent): Boolean {
        val uri = intent.data
        if (intent.action != Intent.ACTION_VIEW || uri == null) {
            return false
        }
        confirmDiscard { openDocument(uri) }
        return true
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
                    recovery.clear()
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
                    recovery.clear()
                    showOpenedDocument(info)
                }
                .onFailure(::showError)
        }
    }

    /** Reopens the unsaved changes kept by [autosave], still marked as unsaved. */
    private fun restoreRecovery() {
        showMessage(getString(R.string.loading))
        renderer.open({ recovery.read() }, brush = null) { result ->
            result
                .onSuccess { info ->
                    currentUri = null
                    documentName = recovery.documentName ?: getString(R.string.untitled_file_name)
                    showOpenedDocument(info)
                    canvasView.applyEdit(NativeDocument::markModified)
                    Toast.makeText(this, R.string.recovered, Toast.LENGTH_LONG).show()
                }
                .onFailure { error ->
                    Log.e(TAG, "Restoring the recovery copy failed", error)
                    recovery.clear()
                    showMessage(getString(R.string.empty_hint))
                }
        }
    }

    private fun showOpenedDocument(info: DocumentInfo) {
        documentOpen = true
        changesDiscarded = false
        messageView.visibility = View.GONE
        if (!settings.colorChosen) {
            val contrasting = BrushPalette.contrastingColor(info.backgroundColor)
            changeSettings(settings.copy(brush = settings.brush.copy(color = contrasting)))
        }
        canvasView.showDocument(info.bounds)
        // A new document starts with the engine defaults: bring it in line with the settings.
        // Queued before any pen input, so the first stroke already uses them.
        renderer.setBrush(settings.brush)
        canvasView.applyEdit { document ->
            document.setShape(settings.shape)
            document.setBrushStyle(settings.brushStyle)
            document.setEraserStyle(settings.eraserStyle)
            document.setTool(settings.tool)
        }
        updateEditState(info.editState)
    }

    private fun changeSettings(newSettings: EditorSettings) {
        settings = newSettings
        settings.save(this)
        applySettingsToView()
    }

    private fun applySettingsToView() {
        canvasView.tool = settings.tool
        canvasView.brush = settings.brush
        canvasView.fingerDrawing = settings.fingerDrawing
    }

    private fun changeBrush(brush: Brush, colorChosen: Boolean) {
        changeSettings(settings.copy(brush = brush, colorChosen = colorChosen))
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

    /**
     * Saves to [uri]. When [quiet], failures fall back to a recovery copy instead of asking
     * the user, since this runs while the app goes to the background.
     */
    private fun writeTo(uri: Uri, quiet: Boolean = false) {
        renderer.save(documentName, { bytes -> DocumentFiles.write(contentResolver, uri, bytes) }) { result ->
            result
                .onSuccess { state ->
                    recovery.clear()
                    updateEditState(state)
                    if (!quiet) {
                        Toast.makeText(this, R.string.saved, Toast.LENGTH_SHORT).show()
                    }
                }
                .onFailure { error ->
                    Log.e(TAG, "Saving to $uri failed", error)
                    when {
                        quiet -> saveRecovery()
                        error is SecurityException -> {
                            // Opened read-only, e.g. from another app: ask where to save instead.
                            Toast.makeText(this, R.string.save_read_only, Toast.LENGTH_LONG).show()
                            saveAs()
                        }
                        else -> {
                            val message = error.message ?: error.toString()
                            Toast.makeText(this, getString(R.string.save_failed, message), Toast.LENGTH_LONG).show()
                        }
                    }
                }
        }
    }

    /** Saves to the open file, or keeps a recovery copy if there is none. */
    private fun autosave() {
        val uri = currentUri
        if (uri != null) {
            writeTo(uri, quiet = true)
        } else {
            saveRecovery()
        }
    }

    private fun saveRecovery() {
        val name = documentName
        renderer.saveCopy(name, { bytes -> recovery.write(bytes, name) }) { result ->
            result.onFailure { error -> Log.e(TAG, "Writing the recovery copy failed", error) }
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
            .setPositiveButton(R.string.discard) { _, _ ->
                changesDiscarded = true
                recovery.clear()
                action()
            }
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
    }
}
