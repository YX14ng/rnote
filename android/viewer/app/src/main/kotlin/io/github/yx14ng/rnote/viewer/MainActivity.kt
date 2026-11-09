package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.view.HapticFeedbackConstants
import android.view.KeyEvent
import android.view.Menu
import android.view.MenuItem
import android.view.View
import android.widget.Toast

/** Opens, creates, edits and saves `.rnote` documents. */
class MainActivity : Activity() {
    private val renderer = DocumentRenderer()
    private lateinit var canvasView: NoteCanvasView
    private lateinit var layout: EditorLayout
    private lateinit var recovery: Recovery
    private lateinit var saver: DocumentSaver
    private lateinit var guard: UnsavedChangesGuard
    private lateinit var clipboard: ClipboardActions
    private lateinit var toolSidebar: ToolSidebar

    private var settings = EditorSettings()
    private var documentOpen = false
    private var editState = EditState.CLEAN
    private var documentSettings: DocumentSettings? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        recovery = Recovery(this)
        guard = UnsavedChangesGuard(this, onDiscard = recovery::clear)
        saver = DocumentSaver(this, renderer, recovery, onSaved = ::updateEditState, requestSaveAs = ::saveAs)
        settings = EditorSettings.load(this)
        canvasView = NoteCanvasView(this).apply {
            renderer = this@MainActivity.renderer
            onError = ::showError
            onEditState = ::updateEditState
            onTwoFingerTap = ::undoByGesture
        }
        clipboard = ClipboardActions(this, canvasView, renderer, onToolChanged = { tool ->
            // The engine already switched to the tool that shows what was pasted.
            changeSettings(settings.copy(tool = tool))
            invalidateOptionsMenu()
        }, onError = ::showError)
        toolSidebar = ToolSidebar(this).apply {
            onToolSelected = ::selectTool
            visibility = View.GONE
        }
        applySettingsToView()
        layout = EditorLayout(this, canvasView, toolSidebar)
        setContentView(layout.root)
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
        // Also while the configuration changes: one the manifest doesn't list recreates the
        // activity, and the native document goes with the old one.
        if (documentOpen && editState.modified && !guard.changesDiscarded) {
            saver.autosave()
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

    // Ctrl+Y redoes too, as on the desktop; the menu shortcuts cover the rest.
    override fun onKeyShortcut(keyCode: Int, event: KeyEvent): Boolean {
        if (keyCode == KeyEvent.KEYCODE_Y && event.isCtrlPressed && documentOpen && editState.canRedo) {
            canvasView.applyEdit(NativeDocument::redo)
            return true
        }
        return super.onKeyShortcut(keyCode, event)
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        val action = EditorMenu.actionFor(item) ?: return super.onOptionsItemSelected(item)
        when (action) {
            EditorMenu.Action.Undo -> canvasView.applyEdit(NativeDocument::undo)
            EditorMenu.Action.Redo -> canvasView.applyEdit(NativeDocument::redo)
            is EditorMenu.Action.SelectColor ->
                changeBrush(settings.brush.copy(color = action.color), colorChosen = true)
            EditorMenu.Action.CustomColor -> FormatDialogs.showCustomColor(this, settings.brush.color) { color ->
                changeBrush(settings.brush.copy(color = color), colorChosen = true)
                invalidateOptionsMenu()
            }
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
            EditorMenu.Action.Copy -> clipboard.copy()
            EditorMenu.Action.Cut -> clipboard.cut()
            EditorMenu.Action.Paste -> clipboard.paste()
            is EditorMenu.Action.Emphasis -> canvasView.applyEdit { it.toggleTextEmphasis(action.emphasis) }
            EditorMenu.Action.ColorSelectedText -> canvasView.applyEdit { it.colorSelectedText(settings.brush.color) }
            EditorMenu.Action.SelectAllText ->
                canvasView.applyEdit { it.handleCharKey('a'.code, shift = false, ctrl = true) }
            EditorMenu.Action.FinishShape -> canvasView.finishShape()
            EditorMenu.Action.TextFormat -> FormatDialogs.showTextFormat(this, settings.textFormat) { format ->
                changeSettings(settings.copy(textFormat = format))
                canvasView.applyEdit { it.setTextFormat(format) }
            }
            EditorMenu.Action.ShapeStyle -> FormatDialogs.showShapeStyle(this, settings.shapeStyle) { style ->
                changeSettings(settings.copy(shapeStyle = style))
                canvasView.applyEdit { it.setShapeStyle(style) }
            }
            EditorMenu.Action.Document -> documentSettings?.let { current ->
                FormatDialogs.showDocument(this, current) { change ->
                    canvasView.applyEdit(change)
                    // Runs after the edit, so the view already has the new bounds. Only a new
                    // page width or layout moves the view; a new background keeps it.
                    refreshDocumentSettings(onRefreshed = { updated ->
                        if (updated.pageWidth != current.pageWidth || updated.layout != current.layout) {
                            canvasView.refit(updated)
                        }
                    })
                }
            }
            EditorMenu.Action.ToggleFingerDrawing ->
                changeSettings(settings.copy(fingerDrawing = !settings.fingerDrawing))
            EditorMenu.Action.ToggleShapeRecognition -> {
                changeSettings(settings.copy(shapeRecognition = !settings.shapeRecognition))
                applyShapeRecognition()
            }
            EditorMenu.Action.New -> guard.confirmDiscard(::newDocument)
            EditorMenu.Action.Open ->
                guard.confirmDiscard { startActivityForResult(DocumentFiles.openIntent(saver.currentUri), REQUEST_OPEN) }
            EditorMenu.Action.Save -> saver.save()
            EditorMenu.Action.SaveAs -> saveAs()
        }
        invalidateOptionsMenu()
        return true
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data
        if (resultCode != RESULT_OK || uri == null) {
            // Backed out of the picker: the open document stays, and so does its autosave.
            guard.keepChanges()
            return
        }
        when (requestCode) {
            REQUEST_OPEN -> openDocument(uri)
            REQUEST_SAVE_AS -> saver.saveAs(uri)
        }
    }

    // Only reached before Android 13: from 13 on the manifest opts into OnBackInvokedCallback,
    // which UnsavedChangesGuard registers. No AndroidX here, so no OnBackPressedDispatcher.
    @SuppressLint("GestureBackNavigation")
    @Deprecated("Only used before Android 13; later versions use OnBackInvokedCallback.")
    override fun onBackPressed() {
        if (editState.modified) {
            guard.confirmDiscard { finish() }
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
        guard.confirmDiscard { openDocument(uri) }
        return true
    }

    private fun openDocument(uri: Uri) {
        startLoading()
        renderer.open({ DocumentFiles.read(contentResolver, uri) }, brush = null) { result ->
            result
                .onSuccess { info ->
                    saver.currentUri = uri
                    saver.documentName = DocumentFiles.displayName(contentResolver, uri)
                        ?: uri.lastPathSegment
                        ?: getString(R.string.untitled_file_name)
                    recovery.clear()
                    showOpenedDocument(info)
                }
                .onFailure { error ->
                    // The current document stays open: keep autosaving it.
                    guard.keepChanges()
                    endLoading()
                    showError(error)
                }
        }
    }

    private fun newDocument() {
        canvasView.loading = true
        renderer.createEmpty(brush = null) { result ->
            result
                .onSuccess { info ->
                    saver.currentUri = null
                    saver.documentName = getString(R.string.untitled_file_name)
                    recovery.clear()
                    showOpenedDocument(info)
                }
                .onFailure { error ->
                    endLoading()
                    showError(error)
                }
        }
    }

    /** Reopens the unsaved changes kept by [DocumentSaver.autosave], still marked as unsaved. */
    private fun restoreRecovery() {
        startLoading()
        renderer.open({ recovery.read() }, brush = null) { result ->
            result
                .onSuccess { info ->
                    saver.currentUri = null
                    saver.documentName = recovery.documentName ?: getString(R.string.untitled_file_name)
                    showOpenedDocument(info)
                    canvasView.applyEdit(NativeDocument::markModified)
                    Toast.makeText(this, R.string.recovered, Toast.LENGTH_LONG).show()
                }
                .onFailure { error ->
                    Log.e(TAG, "Restoring the recovery copy failed", error)
                    recovery.clear()
                    endLoading()
                    if (!documentOpen) {
                        layout.showMessage(getString(R.string.empty_hint))
                    }
                }
        }
    }

    /** Shows that a document is loading, and keeps input away from the current one. */
    private fun startLoading() {
        canvasView.loading = true
        layout.showMessage(getString(R.string.loading))
    }

    /** Undoes [startLoading] after a failure: the current document, if any, stays usable. */
    private fun endLoading() {
        canvasView.loading = false
        if (documentOpen) {
            layout.hideMessage()
        }
    }

    private fun showOpenedDocument(info: DocumentInfo) {
        documentOpen = true
        documentSettings = info.settings
        guard.changesDiscarded = false
        layout.hideMessage()
        toolSidebar.visibility = View.VISIBLE
        if (!settings.colorChosen) {
            val contrasting = BrushPalette.contrastingColor(info.backgroundColor)
            changeSettings(settings.copy(brush = settings.brush.copy(color = contrasting)))
        }
        canvasView.showDocument(info.bounds, info.settings)
        // A new document starts with the engine defaults: bring it in line with the settings.
        // Queued before any pen input, so the first stroke already uses them.
        renderer.setBrush(settings.brush)
        canvasView.applyEdit { document ->
            document.setShape(settings.shape)
            document.setBrushStyle(settings.brushStyle)
            document.setEraserStyle(settings.eraserStyle)
            document.setShapeStyle(settings.shapeStyle)
            // Only new text: nothing is being edited yet.
            document.setTextFormat(settings.textFormat)
            document.setTool(settings.tool)
            document.setShapeRecognition(settings.shapeRecognition, shapeMinSizePx())
        }
        updateEditState(info.editState)
    }

    /** Reads the document settings back after the document dialog changed them. */
    private fun refreshDocumentSettings(onRefreshed: (DocumentSettings) -> Unit) {
        renderer.query({ it.settings }) { result ->
            result
                .onSuccess { settings ->
                    documentSettings = settings
                    canvasView.pageColor = settings.backgroundColor
                    if (!this.settings.colorChosen) {
                        // Keep the automatic color readable on the new background.
                        val contrasting = BrushPalette.contrastingColor(settings.backgroundColor)
                        changeBrush(this.settings.brush.copy(color = contrasting), colorChosen = false)
                    }
                    onRefreshed(settings)
                }
                .onFailure(::showError)
        }
    }

    private fun changeSettings(newSettings: EditorSettings) {
        settings = newSettings
        settings.save(this)
        applySettingsToView()
    }

    private fun applyShapeRecognition() {
        val enabled = settings.shapeRecognition
        val minSizePx = shapeMinSizePx()
        canvasView.applyEdit { it.setShapeRecognition(enabled, minSizePx) }
    }

    /** Strokes smaller than this on screen stay as drawn, even when the pen holds still. */
    private fun shapeMinSizePx(): Double = SHAPE_MIN_SIZE_DP * resources.displayMetrics.density.toDouble()

    /** A two-finger tap undoes, like in other note-taking apps. */
    private fun undoByGesture() {
        if (documentOpen && editState.canUndo) {
            canvasView.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
            canvasView.applyEdit(NativeDocument::undo)
        }
    }

    private fun selectTool(tool: Tool) {
        changeSettings(settings.copy(tool = tool))
        canvasView.applyEdit { it.setTool(tool) }
        invalidateOptionsMenu()
    }

    private fun applySettingsToView() {
        toolSidebar.selectedTool = settings.tool
        canvasView.tool = settings.tool
        canvasView.brush = settings.brush
        canvasView.fingerDrawing = settings.fingerDrawing
        canvasView.holdToSnap = settings.shapeRecognition
    }

    private fun changeBrush(brush: Brush, colorChosen: Boolean) {
        changeSettings(settings.copy(brush = brush, colorChosen = colorChosen))
        if (documentOpen) {
            renderer.setBrush(brush)
        }
    }

    private fun saveAs() {
        startActivityForResult(DocumentFiles.createIntent(saver.documentName, saver.currentUri), REQUEST_SAVE_AS)
    }

    private fun updateEditState(state: EditState) {
        editState = state
        title = if (state.modified) "• ${saver.documentName}" else saver.documentName
        invalidateOptionsMenu()
        guard.modified = state.modified
    }

    private fun showError(error: Throwable) {
        Log.e(TAG, "Document operation failed", error)
        val message = getString(R.string.open_failed, error.message ?: error.toString())
        if (documentOpen) {
            Toast.makeText(this, message, Toast.LENGTH_LONG).show()
        } else {
            layout.showMessage(message)
        }
    }

    private companion object {
        const val TAG = "RnoteViewer"
        // Holding the pen still is what asks for a shape, so small ones are fine too.
        const val SHAPE_MIN_SIZE_DP = 24.0

        const val REQUEST_OPEN = 1
        const val REQUEST_SAVE_AS = 2
    }
}
