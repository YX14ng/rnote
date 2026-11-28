package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.graphics.Point
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.HapticFeedbackConstants
import android.view.KeyEvent
import android.view.View
import android.widget.Toast
import android.window.OnBackInvokedDispatcher
import androidx.annotation.StringRes

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
    private lateinit var ink: InkIndexer
    private lateinit var search: InkSearch
    private lateinit var controls: EditorControls
    private lateinit var selectionBar: SelectionBar
    private lateinit var pages: PagesController
    private lateinit var pdfExport: PdfExport

    // Saves the notebook a few seconds after the writing pauses, so a crash loses little.
    private val autosave = AutosaveTimer(
        hasChanges = { documentOpen && editState.modified && !guard.changesDiscarded },
        penDown = { canvasView.penDown },
        save = { saver.autosave() },
    )
    private var shownZoomPercent = 0

    private var documentOpen = false

    // What came back from another app (camera, pickers) waits here while the notebook loads.
    private val untilOpen = UntilOpen()

    /** The open notebook's path in the library, if it is one of its notebooks; kept by [saver]. */
    private val libraryPath: String?
        get() = saver.libraryPath

    private lateinit var openSync: OpenNotebookSync
    private val syncListener: (SyncStatus) -> Unit = { openSync.onStatus(it) }
    private var editState = EditState.CLEAN
    private var documentSettings: DocumentSettings? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        recovery = Recovery(this)
        guard = UnsavedChangesGuard(this, onDiscard = recovery::clear, onBack = ::leave)
        saver = DocumentSaver(this, renderer, recovery, onSaved = ::onSaved, requestSaveAs = ::saveAs)
        saver.onLocationChanged = ::rememberLocation
        registerBackSteps()
        openSync = OpenNotebookSync(
            this,
            saver,
            modified = { editState.modified },
            onMoved = ::refreshBar,
            reopen = { path ->
                openLibraryNotebook(path, isNew = false, keepView = true)
                Toast.makeText(this, R.string.sync_reloaded, Toast.LENGTH_SHORT).show()
            },
            close = {
                Toast.makeText(this, R.string.sync_removed, Toast.LENGTH_LONG).show()
                toLibrary()
            },
        )
        saver.onConflictCopy = {
            Toast.makeText(this, R.string.sync_saved_as_copy, Toast.LENGTH_LONG).show()
            refreshBar()
        }
        ink = InkIndexer(this, InkRecognizer(applicationContext), renderer)
        canvasView = NoteCanvasView(this).apply {
            renderer = this@MainActivity.renderer
            onError = { showError(R.string.failed_edit, it) }
            onEditState = ::updateEditState
            onTwoFingerTap = ::undoByGesture
            onThreeFingerTap = ::redoByGesture
            onPenWheel = ::showPenWheel
            onLongPress = ::showCanvasMenu
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
        topBar = EditorTopBar(
            this, toolRow, onAction = ::perform, onBack = ::leave, onTitle = ::renameNotebook,
            onSearch = { search.open() },
        )
        search = InkSearch(this, canvasView, renderer, ink, topBar)
        selectionBar = SelectionBar(this, palette = { controls.paletteOnPaper }, onAction = ::perform)
        clipboard = ClipboardActions(this, canvasView, renderer, onToolChanged = { tool ->
            // The engine already switched to the tool that shows what was pasted.
            controls.change(controls.settings.copy(tool = tool))
        }, onError = { showError(R.string.failed_paste, it) })
        layout = EditorLayout(this, canvasView, topBar)
        layout.selectionBar = selectionBar
        pages = PagesController(this, layout, canvasView, renderer, pickPdf = {
            @Suppress("DEPRECATION")
            startActivityForResult(DocumentFiles.openIntent(null).setType("application/pdf"), REQUEST_INSERT_PDF)
        }, onError = { showError(R.string.failed_pages, it) })
        pdfExport = PdfExport(this, renderer, onError = { showError(R.string.failed_export, it) })
        savedInstanceState?.let { state ->
            val sheets = state.getIntArray(STATE_PDF_SHEETS)?.let { it[0]..it[1] }
            pdfExport.restorePending(PdfOptions(state.getBoolean(STATE_PDF_WHITE_PAPER), sheets))
        }
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
        // Back from another app or the recents screen: the notebook may have changed on another
        // device meanwhile, and writing on the old version would end in a conflict copy.
        if (libraryPath != null) SyncManager.syncNow(this)
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
                is EditorMenu.Action.PasteAt -> clipboard.paste(action.x to action.y)
                EditorMenu.Action.InsertImage -> pickImage(null)
                is EditorMenu.Action.InsertImageAt -> pickImage(action.x to action.y)
                EditorMenu.Action.TakePhoto -> takePhoto(null)
                is EditorMenu.Action.TakePhotoAt -> takePhoto(action.x to action.y)
                EditorMenu.Action.Document -> showDocumentDialog()
                EditorMenu.Action.Pages ->
                    // Only notebooks of sheets have pages: say how to get them.
                    if (documentSettings?.layout == DocumentLayout.FIXED_SIZE) {
                        pages.toggle()
                    } else {
                        val layoutName = getString(R.string.layout_fixed_size)
                        val menuName = getString(R.string.page_and_background).trimEnd('…', '.')
                        Toast.makeText(this, getString(R.string.pages_need_sheets, layoutName, menuName), Toast.LENGTH_LONG).show()
                    }
                EditorMenu.Action.New -> newNotebook()
                EditorMenu.Action.Open ->
                    guard.confirmDiscard { startActivityForResult(DocumentFiles.openIntent(saver.currentUri), REQUEST_OPEN) }
                EditorMenu.Action.Save -> saver.save()
                EditorMenu.Action.SaveAs -> saveAs()
                EditorMenu.Action.ExportPdf -> askPdfOptions(R.string.pdf_export_action) {
                    val pick = Intent(Intent.ACTION_CREATE_DOCUMENT)
                        .addCategory(Intent.CATEGORY_OPENABLE)
                        .setType(PdfExport.MIME_TYPE)
                        .putExtra(Intent.EXTRA_TITLE, PdfExport.fileName(saver.documentName))
                    @Suppress("DEPRECATION")
                    startActivityForResult(pick, REQUEST_EXPORT_PDF)
                }
                EditorMenu.Action.SharePdf -> askPdfOptions(R.string.pdf_share_action) { options ->
                    pdfExport.share(saver.documentName, options)
                }
                else -> {}
            }
        }
        refreshBar()
    }

    /** Shows the notebook's name and state in the top bar. */
    private fun refreshBar() {
        topBar.show(saver.documentName.removeSuffix(".rnote"), menuState())
    }

    private fun menuState() = MenuState(
        documentOpen,
        editState,
        controls.settings,
        canvasView.zoomPercent,
        hasPages = documentSettings?.layout == DocumentLayout.FIXED_SIZE,
        inLibrary = libraryPath != null,
    )

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
                        InkIndexFiles.move(this, path, renamed)
                        saver.moveTo(renamed, Uri.fromFile(store.file(renamed)), store.file(renamed).name)
                    }
                    .onFailure { showError(R.string.failed_rename, it) }
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
                result.onSuccess { openLibraryNotebook(it, isNew = false) }.onFailure { showError(R.string.failed_create, it) }
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
                pages.onDocumentShown()
            })
        }
    }

    /** The view moved: the selection bar follows, and the zoom shown updates. */
    private fun onViewportMoved() {
        selectionBar.follow(canvasView.currentViewport)
        pages.followView()
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
            canvasView.selectionBounds = null
            return
        }
        renderer.query({ it.selectionBounds() }) { result ->
            result.onSuccess { bounds ->
                canvasView.selectionBounds = bounds
                selectionBar.show(bounds, canvasView.currentViewport)
            }
        }
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == REQUEST_TAKE_PHOTO) {
            untilOpen.run { onPhotoTaken(resultCode == RESULT_OK) }
            return
        }
        val uri = data?.data
        if (resultCode != RESULT_OK || uri == null) {
            // Backed out of the picker: the open document stays, and so does its autosave.
            guard.keepChanges()
            return
        }
        if (requestCode == REQUEST_OPEN) {
            openDocument(uri)
            return
        }
        // The others need the notebook, which may still be loading after a restart.
        untilOpen.run {
            when (requestCode) {
                REQUEST_SAVE_AS -> saver.saveAs(uri)
                REQUEST_INSERT_PDF -> pages.onPdfPicked(uri)
                REQUEST_EXPORT_PDF -> pdfExport.saveTo(uri, saver.documentName, pdfExport.pending)
                REQUEST_INSERT_IMAGE -> {
                    val resolver = contentResolver
                    clipboard.insertImage(insertAt) {
                        resolver.openInputStream(uri)?.use { it.readBytes() } ?: error("The picture can't be read")
                    }
                }
            }
        }
    }

    // Where the picture or photo being picked goes, in document coordinates; null for the
    // middle of the view, also when the activity was recreated meanwhile.
    private var insertAt: Pair<Double, Double>? = null

    private fun pickImage(at: Pair<Double, Double>?) {
        insertAt = at
        val pick = Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("image/*")
        @Suppress("DEPRECATION")
        startActivityForResult(pick, REQUEST_INSERT_IMAGE)
    }

    private fun takePhoto(at: Pair<Double, Double>?) {
        insertAt = at
        PhotoCapture.start(this, REQUEST_TAKE_PHOTO)
    }

    private fun onPhotoTaken(taken: Boolean) {
        // Cancelled, or the camera wrote nothing: the notebook stays as it was.
        val photo = (if (taken) PhotoCapture.takenPhoto(this) else null) ?: return PhotoCapture.clear(this)
        // Read and then forgotten off the main thread: a photo can weigh several MB.
        clipboard.insertImage(insertAt) { photo.readBytes().also { photo.delete() } }
    }

    // Only reached before Android 13: from 13 on the manifest opts into OnBackInvokedCallback,
    // which UnsavedChangesGuard registers. No AndroidX here, so no OnBackPressedDispatcher.
    @SuppressLint("GestureBackNavigation")
    @Deprecated("Only used before Android 13; later versions use OnBackInvokedCallback.")
    override fun onBackPressed() {
        if (closeOnBack()) return
        if (editState.modified) {
            leave()
        } else {
            @Suppress("DEPRECATION")
            super.onBackPressed()
        }
    }

    /**
     * Closes what is open on top of the page (a panel, the search, the pages, a selection);
     * false when there's nothing, and back leaves the notebook.
     */
    private fun closeOnBack(): Boolean {
        val step = BackStep.first(
            overlay = layout.overlayShown,
            search = search.isOpen,
            pages = pages.shown,
            selection = documentOpen && editState.hasSelection,
        )
        when (step) {
            BackStep.OVERLAY -> {
                layout.hideOverlay()
                refreshSelectionBar()
            }
            BackStep.SEARCH -> search.close()
            BackStep.PAGES -> pages.closePages()
            BackStep.SELECTION -> perform(EditorMenu.Action.Selection(SelectionAction.DESELECT_ALL))
            BackStep.LEAVE -> return false
        }
        return true
    }

    /** From Android 13 on: back first closes what is open on top of the page. */
    private fun registerBackSteps() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        // Above the unsaved changes guard, which it falls back to by leaving as it would.
        onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_OVERLAY) {
            if (closeOnBack()) return@registerOnBackInvokedCallback
            if (editState.modified) leave() else finish()
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
        autosave.stop()
        // Before the renderer closes: a search waiting to run would use it.
        search.close()
        ink.close()
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

    /**
     * Opens the library notebook at [path], or creates it there when [isNew]; with [keepView]
     * (read again after a sync) the view stays where it was.
     */
    private fun openLibraryNotebook(path: String, isNew: Boolean, keepView: Boolean = false) {
        // Changes it had when the app stopped unexpectedly come back with it.
        if (!isNew && recovery.exists && recovery.libraryPath == path) {
            restoreRecovery()
            return
        }
        val file = LibraryStore.forApp(this).notebookFile(path)
        // A new notebook never replaces one, whoever asks.
        if (file == null || (isNew && file.exists())) {
            showError(R.string.failed_open, IllegalArgumentException(getString(R.string.library_bad_path, path)))
            return
        }
        val uri = Uri.fromFile(file)
        startLoading()
        // Read on the renderer's thread, used on the main one.
        var readSha256: String? = null
        val onOpened = { result: Result<DocumentInfo> ->
            result
                .onSuccess { info ->
                    saver.moveTo(path, uri, file.name)
                    saver.knownSha256 = readSha256
                    // A recovery copy belongs to another document: it stays until restored.
                    saver.recoveryClaim.opened()
                    showOpenedDocument(info, keepView)
                    // A new notebook exists in the library from the start.
                    if (isNew) saver.autosave()
                    // A sync may have written it while it was being read.
                    (SyncManager.status as? SyncStatus.Done)?.let(openSync::onStatus)
                }
                .onFailure { error ->
                    guard.keepChanges()
                    endLoading()
                    showError(R.string.failed_open, error)
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
                    val name = DocumentFiles.displayName(contentResolver, uri)
                        ?: uri.lastPathSegment
                        ?: getString(R.string.untitled_file_name)
                    saver.moveTo(null, uri, name)
                    // A recovery copy belongs to another document: it stays until restored.
                    saver.recoveryClaim.opened()
                    showOpenedDocument(info)
                }
                .onFailure { error ->
                    // The current document stays open: keep autosaving it.
                    guard.keepChanges()
                    endLoading()
                    showError(R.string.failed_open, error)
                }
        }
    }

    /** Reopens the unsaved changes kept by [DocumentSaver.autosave], still marked as unsaved. */
    private fun restoreRecovery() {
        startLoading()
        renderer.open({ recovery.read() }, brush = null) { result ->
            result
                .onSuccess { info ->
                    val path = recovery.libraryPath
                    val file = path?.let { LibraryStore.forApp(this).notebookFile(it) }?.takeIf { it.exists() }
                    if (path != null && file != null) {
                        // Back in its place in the library; saving compares with the file as it
                        // was then, so changes a sync brought meanwhile go to a conflict copy.
                        saver.moveTo(path, Uri.fromFile(file), file.name)
                        saver.knownSha256 = recovery.knownSha256
                    } else {
                        saver.moveTo(null, null, recovery.documentName ?: getString(R.string.untitled_file_name))
                    }
                    saver.recoveryClaim.restored()
                    showOpenedDocument(info)
                    canvasView.applyEdit(NativeDocument::markModified)
                    Toast.makeText(this, R.string.recovered, Toast.LENGTH_LONG).show()
                }
                .onFailure { error ->
                    Log.e(TAG, "Restoring the recovery copy failed", error)
                    endLoading()
                    if (!documentOpen) {
                        layout.showMessage(getString(R.string.empty_hint))
                    }
                    // The copy is all there is of those changes: never thrown away unasked.
                    AlertDialog.Builder(this)
                        .setTitle(R.string.recovery_restore_failed_title)
                        .setMessage(getString(R.string.recovery_restore_failed_message, ErrorText.reason(this, error)))
                        .setPositiveButton(R.string.recovery_keep) { _, _ -> leaveIfEmpty() }
                        .setNegativeButton(R.string.recovery_discard) { _, _ ->
                            recovery.clear()
                            leaveIfEmpty()
                        }
                        .setCancelable(false)
                        .show()
                }
        }
    }

    /** With no document to show, goes back to where the editor was opened from. */
    private fun leaveIfEmpty() {
        if (!documentOpen) finish()
    }

    /** Shows that a document is loading, and keeps input away from the current one. */
    private fun startLoading() {
        untilOpen.loading()
        canvasView.loading = true
        layout.showMessage(getString(R.string.loading))
    }

    /** Undoes [startLoading] after a failure: the current document, if any, stays usable. */
    private fun endLoading() {
        canvasView.loading = false
        if (documentOpen) {
            layout.hideMessage()
            untilOpen.opened()
        } else {
            untilOpen.failed()
        }
    }

    private fun showOpenedDocument(info: DocumentInfo, keepView: Boolean = false) {
        autosave.stop()
        documentOpen = true
        controls.documentOpen = true
        documentSettings = info.settings
        guard.changesDiscarded = false
        layout.hideMessage()
        controls.adaptToBackground(info.backgroundColor)
        canvasView.showDocument(info.bounds, info.settings, info.startAt, keepView)
        controls.applyToDocument()
        updateEditState(info.editState)
        search.close()
        pages.onDocumentShown()
        // A notebook with no file yet has no index: its text boxes are searched still.
        ink.open((libraryPath ?: saver.currentUri?.toString())?.let { InkIndexer.indexFileFor(this, it) })
        untilOpen.opened()
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
                .onFailure { showError(R.string.failed_settings, it) }
        }
    }

    /**
     * Opens the page's menu where a finger held still: at ([x], [y]) on the canvas, and
     * ([documentX], [documentY]) in the document, where pasting puts things.
     */
    private fun showCanvasMenu(x: Float, y: Float, documentX: Double, documentY: Double) {
        if (!documentOpen) {
            return
        }
        val entries = CanvasMenu.entries(editState.hasSelection, documentX, documentY)
        val panel = CanvasMenu.build(this, entries, ::perform)
        val canvasAt = IntArray(2).also(canvasView::getLocationInWindow)
        val point = Point(canvasAt[0] + x.toInt(), canvasAt[1] + y.toInt())
        val dismiss = {
            layout.hideOverlay()
            refreshSelectionBar()
        }
        layout.showOverlay(PanelOverlay(this, panel, CanvasMenu.PANEL_WIDTH_DP, null, dismiss, point))
    }

    /** Opens the wheel to pick the brush color and width around the pen at ([x], [y]). */
    private fun showPenWheel(x: Float, y: Float) {
        if (!documentOpen) {
            return
        }
        val settings = controls.settings
        val wheel = PenWheel(
            this,
            colors = controls.paletteOnPaper,
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

    /** A three-finger tap redoes, like in other note-taking apps. */
    private fun redoByGesture() {
        if (documentOpen && editState.canRedo) {
            canvasView.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
            canvasView.applyEdit(NativeDocument::redo)
        }
    }

    /** Asks how to export the PDF, telling the dialog which sheet the view shows. */
    private fun askPdfOptions(action: Int, then: (PdfOptions) -> Unit) {
        val viewport = canvasView.currentViewport
        pdfExport.askThen(action, viewport.y, canvasView.height / viewport.zoom, then)
    }

    /**
     * Keeps this screen's intent on the open notebook: when Android recreates the screen (the
     * app was killed in the background), it opens this one again, not the one it started with.
     */
    private fun rememberLocation() {
        refreshBar()
        val path = saver.libraryPath
        val uri = saver.currentUri
        intent = when {
            path != null -> Intent(this, MainActivity::class.java).putExtra(EXTRA_LIBRARY_PATH, path)
            uri != null -> Intent(Intent.ACTION_VIEW, uri, this, MainActivity::class.java)
            else -> Intent(this, MainActivity::class.java)
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        // The file picker may outlive this process: the PDF still comes out as chosen.
        pdfExport.pending.let { options ->
            outState.putBoolean(STATE_PDF_WHITE_PAPER, options.onWhitePaper)
            options.sheets?.let { outState.putIntArray(STATE_PDF_SHEETS, intArrayOf(it.first, it.last)) }
        }
    }

    private fun saveAs() {
        startActivityForResult(DocumentFiles.createIntent(saver.documentName, saver.currentUri), REQUEST_SAVE_AS)
    }

    /** Saved: a library notebook goes to Drive a few seconds later. */
    private fun onSaved(state: EditState) {
        autosave.saved()
        updateEditState(state)
        if (libraryPath != null) SyncManager.syncSoon(this)
    }

    private fun updateEditState(state: EditState) {
        if (documentOpen && state.modified) {
            ink.onEdited()
            search.onEdited()
            autosave.edited()
        }
        if (state.modified) pages.onEdited()
        // Every batch of pen samples reports a state, nearly always the one already shown.
        if (state != editState) {
            editState = state
            refreshBar()
            guard.modified = state.modified
        }
        // Even with the same state: a moved selection takes its bar along.
        refreshSelectionBar()
    }

    /** Tells that [action] failed, and why, in words the user can act on. */
    private fun showError(@StringRes action: Int, error: Throwable) {
        val message = ErrorText.of(this, action, error)
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

        private const val STATE_PDF_WHITE_PAPER = "pdf_white_paper"
        private const val STATE_PDF_SHEETS = "pdf_sheets"

        private const val REQUEST_OPEN = 1
        private const val REQUEST_SAVE_AS = 2
        private const val REQUEST_INSERT_PDF = 3
        private const val REQUEST_EXPORT_PDF = 4
        private const val REQUEST_INSERT_IMAGE = 5
        private const val REQUEST_TAKE_PHOTO = 6
    }
}
