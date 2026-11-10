package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.view.Gravity
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
    private lateinit var selectionBar: SelectionBar
    private var shownZoomPercent = 0

    private var documentOpen = false

    /** The open notebook's path in the library, if it is one of its notebooks. */
    private var libraryPath: String? = null
    private var editState = EditState.CLEAN
    private var documentSettings: DocumentSettings? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        recovery = Recovery(this)
        guard = UnsavedChangesGuard(this, onDiscard = recovery::clear, onBack = ::leave)
        saver = DocumentSaver(this, renderer, recovery, onSaved = ::updateEditState, requestSaveAs = ::saveAs)
        canvasView = NoteCanvasView(this).apply {
            renderer = this@MainActivity.renderer
            onError = ::showError
            onEditState = ::updateEditState
            onTwoFingerTap = ::undoByGesture
            onPenWheel = ::showPenWheel
            onViewportMoved = ::onViewportMoved
            // Moving the selection or drawing: the selection bar returns after the edit.
            onCanvasTouched = { selectionBar.hide() }
        }
        toolSidebar = ToolSidebar(this).apply { visibility = View.GONE }
        controls = EditorControls(this, canvasView, toolSidebar, renderer, onChanged = ::invalidateOptionsMenu)
        toolSidebar.onToolSelected = controls::selectTool
        toolSidebar.onColorSelected = controls::selectPaletteColor
        toolSidebar.onColorEdit = controls::editPaletteColor
        toolSidebar.onToolOptions = ::toggleToolOptions
        toolSidebar.onWidthTapped = { toggleToolOptions(Tool.BRUSH) }
        selectionBar = SelectionBar(this, palette = { controls.settings.palette }, onAction = ::perform)
        clipboard = ClipboardActions(this, canvasView, renderer, onToolChanged = { tool ->
            // The engine already switched to the tool that shows what was pasted.
            controls.change(controls.settings.copy(tool = tool))
        }, onError = ::showError)
        layout = EditorLayout(this, canvasView, toolSidebar)
        layout.selectionBar = selectionBar
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
        EditorMenu.prepare(menu, menuState())
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
        perform(action)
        return true
    }

    /** Runs [action], from the top bar, a panel, the selection bar or a shortcut. */
    private fun perform(action: EditorMenu.Action) {
        // Panels close once something is picked; switches and text emphasis keep them open.
        val keepsPanel = action is EditorMenu.Action.ToggleFingerDrawing ||
            action is EditorMenu.Action.ToggleShapeRecognition || action is EditorMenu.Action.Emphasis
        if (!keepsPanel) {
            layout.hideOverlay()
        }
        if (!controls.perform(action)) {
            when (action) {
                EditorMenu.Action.More -> showMenu()
                EditorMenu.Action.ZoomToFit -> canvasView.zoomToFit()
                EditorMenu.Action.Copy -> clipboard.copy()
                EditorMenu.Action.Cut -> clipboard.cut()
                EditorMenu.Action.Paste -> clipboard.paste()
                EditorMenu.Action.Document -> showDocumentDialog()
                EditorMenu.Action.New -> guard.confirmDiscard(::newDocument)
                EditorMenu.Action.Open ->
                    guard.confirmDiscard { startActivityForResult(DocumentFiles.openIntent(saver.currentUri), REQUEST_OPEN) }
                EditorMenu.Action.Save -> saver.save()
                EditorMenu.Action.SaveAs -> saveAs()
                else -> {}
            }
        }
        invalidateOptionsMenu()
    }

    private fun menuState() = MenuState(documentOpen, editState, controls.settings, canvasView.zoomPercent)

    private fun showMenu() {
        val panel = MenuPanel.build(this, menuState(), ::perform)
        layout.showOverlay(
            PanelOverlay(this, panel, MenuPanel.PANEL_WIDTH_DP, Gravity.END or Gravity.TOP, 8f, 8f, layout::hideOverlay),
        )
    }

    /** Opens the options of [tool] beside the sidebar, or closes them if open. */
    private fun toggleToolOptions(tool: Tool) {
        if (layout.overlayShown) {
            layout.hideOverlay()
            return
        }
        val panel = ToolOptionsPanel.build(this, tool, controls.settings, ::perform)
        layout.showOverlay(
            PanelOverlay(
                this,
                panel,
                ToolOptionsPanel.PANEL_WIDTH_DP,
                Gravity.END or Gravity.CENTER_VERTICAL,
                TOOL_OPTIONS_MARGIN_DP,
                0f,
                layout::hideOverlay,
            ),
        )
    }

    private fun showDocumentDialog() {
        val current = documentSettings ?: return
        FormatDialogs.showDocument(this, current) { change ->
            canvasView.applyEdit(change)
            // Runs after the edit, so the view already has the new bounds. Only a new page
            // width or layout moves the view; a new background keeps it.
            refreshDocumentSettings(onRefreshed = { updated ->
                if (updated.pageWidth != current.pageWidth || updated.layout != current.layout) {
                    canvasView.refit(updated)
                }
            })
        }
    }

    /** The view moved: the selection bar follows, and the zoom shown updates. */
    private fun onViewportMoved() {
        selectionBar.follow(canvasView.currentViewport)
        val percent = canvasView.zoomPercent
        if (percent != shownZoomPercent) {
            shownZoomPercent = percent
            invalidateOptionsMenu()
        }
    }

    /** Shows the selection bar over what the lasso selected, if anything. */
    private fun refreshSelectionBar() {
        if (!documentOpen || !editState.hasSelection || controls.settings.tool != Tool.SELECTOR) {
            selectionBar.hide()
            return
        }
        renderer.query({ it.selectionBounds() }) { result ->
            result.onSuccess { bounds -> selectionBar.show(bounds, canvasView.currentViewport) }
        }
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
            leave()
        } else {
            @Suppress("DEPRECATION")
            super.onBackPressed()
        }
    }

    /**
     * Back to the library. A library notebook is saved first, and the editor only closes once
     * that worked; a file opened from elsewhere asks before dropping unsaved changes.
     */
    private fun leave() {
        when {
            !editState.modified -> finish()
            libraryPath != null -> saver.save(onDone = ::finish)
            else -> guard.confirmDiscard { finish() }
        }
    }

    override fun onDestroy() {
        renderer.close()
        super.onDestroy()
    }

    /**
     * Opens the library notebook or the document an ACTION_VIEW intent points to. Returns
     * whether there was one.
     */
    private fun handleIntent(intent: Intent): Boolean {
        intent.getStringExtra(EXTRA_LIBRARY_PATH)?.let { path ->
            guard.confirmDiscard { openLibraryNotebook(path, isNew = intent.getBooleanExtra(EXTRA_NEW, false)) }
            return true
        }
        val uri = intent.data
        if (intent.action != Intent.ACTION_VIEW || uri == null) {
            return false
        }
        guard.confirmDiscard { openDocument(uri) }
        return true
    }

    /** Opens the library notebook at [path], or creates it there when [isNew]. */
    private fun openLibraryNotebook(path: String, isNew: Boolean) {
        val file = LibraryStore.forApp(this).file(path)
        val uri = Uri.fromFile(file)
        startLoading()
        val onOpened = { result: Result<DocumentInfo> ->
            result
                .onSuccess { info ->
                    libraryPath = path
                    saver.currentUri = uri
                    saver.documentName = file.name
                    recovery.clear()
                    showOpenedDocument(info)
                    // A new notebook exists in the library from the start.
                    if (isNew) saver.autosave()
                }
                .onFailure { error ->
                    guard.keepChanges()
                    endLoading()
                    showError(error)
                }
            Unit
        }
        if (isNew) {
            renderer.createEmpty(brush = null, onOpened)
        } else {
            renderer.open({ file.readBytes() }, brush = null, onOpened)
        }
    }

    private fun openDocument(uri: Uri) {
        startLoading()
        renderer.open({ DocumentFiles.read(contentResolver, uri) }, brush = null) { result ->
            result
                .onSuccess { info ->
                    libraryPath = null
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
                    libraryPath = null
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
            presets = ToolOptionsPanel.PRESET_WIDTHS,
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
        title = saver.documentName
        actionBar?.subtitle = if (state.modified) getString(R.string.unsaved) else null
        refreshSelectionBar()
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

    companion object {
        /** The path, in the app's library, of the notebook to open. */
        const val EXTRA_LIBRARY_PATH = "io.github.yx14ng.rnote.viewer.LIBRARY_PATH"

        /** With [EXTRA_LIBRARY_PATH]: create the notebook there instead of opening it. */
        const val EXTRA_NEW = "io.github.yx14ng.rnote.viewer.NEW"

        private const val TAG = "RnoteViewer"

        /** The sidebar's width and margin, plus a gap: tool options open beside it. */
        private const val TOOL_OPTIONS_MARGIN_DP = 94f

        private const val REQUEST_OPEN = 1
        private const val REQUEST_SAVE_AS = 2
    }
}
