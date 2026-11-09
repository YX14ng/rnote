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
    private lateinit var controls: EditorControls

    private var documentOpen = false
    private var editState = EditState.CLEAN
    private var documentSettings: DocumentSettings? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        recovery = Recovery(this)
        guard = UnsavedChangesGuard(this, onDiscard = recovery::clear)
        saver = DocumentSaver(this, renderer, recovery, onSaved = ::updateEditState, requestSaveAs = ::saveAs)
        canvasView = NoteCanvasView(this).apply {
            renderer = this@MainActivity.renderer
            onError = ::showError
            onEditState = ::updateEditState
            onTwoFingerTap = ::undoByGesture
            onPenWheel = ::showPenWheel
        }
        toolSidebar = ToolSidebar(this).apply { visibility = View.GONE }
        controls = EditorControls(this, canvasView, toolSidebar, renderer, onChanged = ::invalidateOptionsMenu)
        toolSidebar.onToolSelected = controls::selectTool
        toolSidebar.onColorSelected = controls::selectPaletteColor
        toolSidebar.onColorEdit = controls::editPaletteColor
        clipboard = ClipboardActions(this, canvasView, renderer, onToolChanged = { tool ->
            // The engine already switched to the tool that shows what was pasted.
            controls.change(controls.settings.copy(tool = tool))
        }, onError = ::showError)
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
        EditorMenu.prepare(menu, MenuState(documentOpen, editState, controls.settings), this)
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
        val settings = controls.settings
        when (action) {
            EditorMenu.Action.Undo -> canvasView.applyEdit(NativeDocument::undo)
            EditorMenu.Action.Redo -> canvasView.applyEdit(NativeDocument::redo)
            is EditorMenu.Action.SelectColor ->
                controls.changeBrush(settings.brush.copy(color = action.color), colorChosen = true)
            EditorMenu.Action.CustomColor -> FormatDialogs.showCustomColor(this, settings.brush.color) { color ->
                controls.changeBrush(controls.settings.brush.copy(color = color), colorChosen = true)
                invalidateOptionsMenu()
            }
            is EditorMenu.Action.SelectWidth ->
                controls.changeBrush(settings.brush.copy(width = action.width), settings.colorChosen)
            is EditorMenu.Action.SelectShape -> {
                // Picking a shape means drawing it next.
                controls.change(settings.copy(shape = action.shape, tool = Tool.SHAPER))
                canvasView.applyEdit { it.setShape(action.shape); it.setTool(Tool.SHAPER) }
            }
            is EditorMenu.Action.SelectBrushStyle -> {
                controls.change(settings.copy(brushStyle = action.style))
                canvasView.applyEdit { it.setBrushStyle(action.style) }
            }
            is EditorMenu.Action.SelectEraserStyle -> {
                controls.change(settings.copy(eraserStyle = action.style))
                canvasView.applyEdit { it.setEraserStyle(action.style) }
            }
            is EditorMenu.Action.Selection -> {
                if (action.action == SelectionAction.SELECT_ALL) {
                    // The engine switches to the selector to show the selection.
                    controls.change(settings.copy(tool = Tool.SELECTOR))
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
                controls.change(controls.settings.copy(textFormat = format))
                canvasView.applyEdit { it.setTextFormat(format) }
            }
            EditorMenu.Action.ShapeStyle -> FormatDialogs.showShapeStyle(this, settings.shapeStyle) { style ->
                controls.change(controls.settings.copy(shapeStyle = style))
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
                controls.change(settings.copy(fingerDrawing = !settings.fingerDrawing))
            EditorMenu.Action.ToggleShapeRecognition -> controls.toggleShapeRecognition()
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
        controls.documentOpen = true
        documentSettings = info.settings
        guard.changesDiscarded = false
        layout.hideMessage()
        toolSidebar.visibility = View.VISIBLE
        controls.adaptToBackground(info.backgroundColor)
        canvasView.showDocument(info.bounds, info.settings)
        controls.applyToDocument()
        updateEditState(info.editState)
    }

    /** Reads the document settings back after the document dialog changed them. */
    private fun refreshDocumentSettings(onRefreshed: (DocumentSettings) -> Unit) {
        renderer.query({ it.settings }) { result ->
            result
                .onSuccess { settings ->
                    documentSettings = settings
                    canvasView.pageColor = settings.backgroundColor
                    controls.adaptToBackground(settings.backgroundColor)
                    onRefreshed(settings)
                }
                .onFailure(::showError)
        }
    }

    /** Opens the wheel to pick the brush color and width around the pen at ([x], [y]). */
    private fun showPenWheel(x: Float, y: Float) {
        if (!documentOpen) {
            return
        }
        val settings = controls.settings
        val wheel = PenWheel(
            this,
            colors = settings.palette,
            widths = BrushPalette.WHEEL_WIDTHS,
            color = settings.brush.color,
            brushWidth = settings.brush.width,
            onPick = controls::pickFromWheel,
            onClose = layout::hideOverlay,
        )
        wheel.centerAt(x, y)
        layout.showOverlay(wheel)
    }

    /** A two-finger tap undoes, like in other note-taking apps. */
    private fun undoByGesture() {
        if (documentOpen && editState.canUndo) {
            canvasView.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
            canvasView.applyEdit(NativeDocument::undo)
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

        const val REQUEST_OPEN = 1
        const val REQUEST_SAVE_AS = 2
    }
}
