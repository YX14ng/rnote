package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.view.HapticFeedbackConstants
import android.view.KeyEvent
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
    private lateinit var toolRow: ToolRow
    private lateinit var topBar: EditorTopBar
    private lateinit var controls: EditorControls
    private lateinit var selectionBar: SelectionBar
    private var shownZoomPercent = 0

    private var documentOpen = false

    /** The open notebook's path in the library, if it is one of its notebooks; kept by [saver]. */
    private var libraryPath: String?
        get() = saver.libraryPath
        set(path) {
            saver.libraryPath = path
            if (path == null) saver.knownSha256 = null
        }

    private val syncListener: (SyncStatus) -> Unit = ::onSyncStatus
    private var editState = EditState.CLEAN
    private var documentSettings: DocumentSettings? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        recovery = Recovery(this)
        guard = UnsavedChangesGuard(this, onDiscard = recovery::clear, onBack = ::leave)
        saver = DocumentSaver(this, renderer, recovery, onSaved = ::onSaved, requestSaveAs = ::saveAs)
        saver.onConflictCopy = {
            Toast.makeText(this, R.string.sync_saved_as_copy, Toast.LENGTH_LONG).show()
            refreshBar()
        }
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
        toolRow = ToolRow(this)
        controls = EditorControls(this, canvasView, toolRow, renderer, onChanged = ::refreshBar)
        toolRow.onToolSelected = controls::selectTool
        toolRow.onColorSelected = controls::selectPaletteColor
        toolRow.onColorEdit = { slot -> showColorPanel(slot, toolRow.swatch(slot)) }
        toolRow.onColorOptions = ::showColorPanel
        toolRow.onToolOptions = { tool -> toggleToolOptions(tool, toolRow.buttonFor(tool)) }
        toolRow.onWidthTapped = { toggleToolOptions(Tool.BRUSH, toolRow.widthAnchor) }
        topBar = EditorTopBar(this, toolRow, onAction = ::perform, onBack = ::leave, onTitle = ::renameNotebook)
        selectionBar = SelectionBar(this, palette = { controls.settings.palette }, onAction = ::perform)
        clipboard = ClipboardActions(this, canvasView, renderer, onToolChanged = { tool ->
            // The engine already switched to the tool that shows what was pasted.
            controls.change(controls.settings.copy(tool = tool))
        }, onError = ::showError)
        layout = EditorLayout(this, canvasView, topBar)
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

    override fun onResume() {
        super.onResume()
        SyncManager.addListener(this, syncListener)
    }

    override fun onPause() {
        super.onPause()
        SyncManager.removeListener(syncListener)
    }

    override fun onStop() {
        super.onStop()
        // Also while the configuration changes: one the manifest doesn't list recreates the
        // activity, and the native document goes with the old one.
        if (documentOpen && editState.modified && !guard.changesDiscarded) {
            saver.autosave()
        }
    }

    override fun onKeyShortcut(keyCode: Int, event: KeyEvent): Boolean {
        val action = KeyShortcuts.actionFor(keyCode, event.isCtrlPressed, event.isShiftPressed)
            ?: return super.onKeyShortcut(keyCode, event)
        val needsDocument = action != EditorMenu.Action.New && action != EditorMenu.Action.Open
        if (documentOpen || !needsDocument) {
            perform(action)
        }
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
                EditorMenu.Action.New -> newNotebook()
                EditorMenu.Action.Open ->
                    guard.confirmDiscard { startActivityForResult(DocumentFiles.openIntent(saver.currentUri), REQUEST_OPEN) }
                EditorMenu.Action.Save -> saver.save()
                EditorMenu.Action.SaveAs -> saveAs()
                else -> {}
            }
        }
        refreshBar()
    }

    /** Shows the notebook's name and state in the top bar. */
    private fun refreshBar() {
        topBar.show(saver.documentName.removeSuffix(".rnote"), menuState())
    }

    private fun menuState() = MenuState(documentOpen, editState, controls.settings, canvasView.zoomPercent)

    private fun showMenu() {
        val panel = MenuPanel.build(this, menuState(), ::perform)
        layout.showOverlay(PanelOverlay(this, panel, MenuPanel.PANEL_WIDTH_DP, topBar.moreButton, layout::hideOverlay))
    }

    /** Opens the options of [tool] below [anchor], the button tapped, or closes them if open. */
    private fun toggleToolOptions(tool: Tool, anchor: View) {
        if (layout.overlayShown) {
            layout.hideOverlay()
            return
        }
        val panel = ToolOptionsPanel.build(this, tool, controls.settings, ::perform)
        layout.showOverlay(PanelOverlay(this, panel, ToolOptionsPanel.PANEL_WIDTH_DP, anchor, layout::hideOverlay))
    }

    /** Opens the color menu of the palette color in [slot] below [anchor]; changes apply at once. */
    private fun showColorPanel(slot: Int, anchor: View) {
        val settings = controls.settings
        val start = settings.palette.getOrNull(slot) ?: return
        val panel = ColorPanel(
            this,
            start,
            settings.savedColors,
            onChange = { color -> controls.setPaletteColor(slot, color) },
            onSavedChange = controls::keepColors,
        )
        layout.showOverlay(PanelOverlay(this, panel.view, ColorPanel.PANEL_WIDTH_DP, anchor, layout::hideOverlay))
    }

    /** Renames the open library notebook, from its name in the top bar. */
    private fun renameNotebook() {
        val path = libraryPath ?: return
        val form = FormDialog(this, R.string.library_rename)
        val field = form.addText(R.string.library_name, saver.documentName.removeSuffix(".rnote"))
        form.show {
            if (field.text.isEmpty()) return@show
            val store = LibraryStore.forApp(this)
            store.call(LibraryStore.request("rename", "path" to path, "new_name" to field.text)) { result ->
                result
                    .onSuccess { answer ->
                        val renamed = answer.getString("path")
                        libraryPath = renamed
                        saver.currentUri = Uri.fromFile(store.file(renamed))
                        saver.documentName = store.file(renamed).name
                        refreshBar()
                    }
                    .onFailure(::showError)
            }
        }
    }

    /**
     * A quick note in the library, beside the open notebook and set up like the last one of
     * its folder; the open one is saved first, or its unsaved changes dropped on agreement if
     * it isn't a library notebook.
     */
    private fun newNotebook() {
        val folder = libraryPath?.let { path -> if ('/' in path) path.substringBefore('/') else null }
        val create = {
            NotebookCreator(this, LibraryStore.forApp(this)).quick(folder) { result ->
                result.onSuccess { openLibraryNotebook(it, isNew = false) }.onFailure(::showError)
            }
        }
        when {
            !editState.modified -> create()
            libraryPath != null -> saver.save(onDone = create)
            else -> guard.confirmDiscard(create)
        }
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
            refreshBar()
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
            !editState.modified -> toLibrary()
            libraryPath != null -> saver.save(onDone = ::toLibrary)
            else -> guard.confirmDiscard(::toLibrary)
        }
    }

    /** Closes the editor; opened from another app, the library shows instead. */
    private fun toLibrary() {
        if (isTaskRoot) {
            startActivity(Intent(this, LibraryActivity::class.java))
        }
        finish()
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
        val file = LibraryStore.forApp(this).notebookFile(path)
        // A new notebook never replaces one, whoever asks.
        if (file == null || (isNew && file.exists())) {
            showError(IllegalArgumentException(getString(R.string.library_bad_path, path)))
            return
        }
        val uri = Uri.fromFile(file)
        startLoading()
        // Read on the renderer's thread, used on the main one.
        var readSha256: String? = null
        val onOpened = { result: Result<DocumentInfo> ->
            result
                .onSuccess { info ->
                    libraryPath = path
                    saver.knownSha256 = readSha256
                    saver.currentUri = uri
                    saver.documentName = file.name
                    // A recovery copy belongs to another document: it stays until restored.
                    showOpenedDocument(info)
                    // A new notebook exists in the library from the start.
                    if (isNew) saver.autosave()
                    // A sync may have written it while it was being read.
                    (SyncManager.status as? SyncStatus.Done)?.let(::onSyncStatus)
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
            renderer.open({ file.readBytes().also { readSha256 = DocumentSaver.sha256Hex(it) } }, brush = null, onOpened)
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
        controls.adaptToBackground(info.backgroundColor)
        canvasView.showDocument(info.bounds, info.settings, info.startAt)
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

    /** Saved: a library notebook goes to Drive a few seconds later. */
    private fun onSaved(state: EditState) {
        updateEditState(state)
        if (libraryPath != null) SyncManager.syncSoon(this)
    }

    /** A sync wrote the open library notebook: see whether it's still the file the editor has. */
    private fun onSyncStatus(status: SyncStatus) {
        val path = libraryPath ?: return
        if (status is SyncStatus.Done && path in status.report.changedHere) checkOpenNotebook(path)
    }

    /**
     * If the file at [path] isn't what the editor read or wrote: without unsaved changes it
     * reopens, or closes if it's gone; with them, saving decides (a conflict copy).
     */
    private fun checkOpenNotebook(path: String) {
        val known = saver.knownSha256
        val store = LibraryStore.forApp(this)
        store.run({ store.notebookFile(path)?.takeIf { it.exists() }?.readBytes()?.let(DocumentSaver::sha256Hex) }) done@{ result ->
            val current = result.getOrElse { return@done }
            if (libraryPath != path || saver.knownSha256 != known || editState.modified || current == known) return@done
            if (current != null) {
                openLibraryNotebook(path, isNew = false)
                Toast.makeText(this, R.string.sync_reloaded, Toast.LENGTH_SHORT).show()
            } else {
                Toast.makeText(this, R.string.sync_removed, Toast.LENGTH_LONG).show()
                toLibrary()
            }
        }
    }

    private fun updateEditState(state: EditState) {
        editState = state
        refreshSelectionBar()
        refreshBar()
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

        private const val REQUEST_OPEN = 1
        private const val REQUEST_SAVE_AS = 2
    }
}
