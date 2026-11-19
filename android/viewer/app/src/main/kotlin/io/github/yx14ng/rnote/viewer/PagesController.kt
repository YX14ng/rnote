package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.app.AlertDialog
import android.net.Uri

/**
 * Opens and closes the column of sheets ([PagesPanel]) and carries out what it asks: going
 * to a sheet, inserting blank sheets or a PDF, and removing sheets, asking first when
 * something written would go with them. Every change can be undone.
 */
internal class PagesController(
    private val activity: Activity,
    private val layout: EditorLayout,
    private val canvas: NoteCanvasView,
    private val renderer: DocumentRenderer,
    private val pickPdf: () -> Unit,
    onError: (Throwable) -> Unit,
) : PagesPanel.Listener {
    private val inserter = PageInserter(activity, renderer, canvas, onError)
    private var panel: PagesPanel? = null

    // Where the PDF being picked goes.
    private var insertAt = 0

    val shown: Boolean
        get() = panel != null

    /** Opens the column, or closes it if it is open. */
    fun toggle() {
        if (panel != null) {
            closePages()
            return
        }
        val opened = PagesPanel(activity, renderer, this)
        panel = opened
        layout.sidePanel = opened
        opened.refreshNow()
        followView()
    }

    /** The view moved: mark the sheet now on screen. */
    fun followView() {
        val viewport = canvas.currentViewport
        panel?.follow(viewport, canvas.height / viewport.zoom)
    }

    /** The document changed: redraw the column. */
    fun onEdited() {
        panel?.refresh()
    }

    /** Another document is shown: read its sheets, or close if it has none. */
    fun onDocumentShown() {
        panel?.refreshNow()
    }

    /** The user picked the PDF to insert. */
    fun onPdfPicked(uri: Uri) = inserter.start(uri, insertAt)

    override fun goToPage(page: Int, sheetHeight: Double) {
        canvas.showDocumentFromY(Pages.top(page, sheetHeight))
    }

    override fun insertBlankPage(at: Int) {
        canvas.applyEdit { document -> document.insertBlankPage(at) }
    }

    override fun insertPdf(at: Int) {
        insertAt = at
        pickPdf()
    }

    override fun removePages(pages: Set<Int>, withContent: BooleanArray) {
        val remove = {
            canvas.applyEdit { document -> document.removePages(pages) }
            panel?.endSelecting()
        }
        if (!Pages.removingLosesContent(pages, withContent)) {
            remove()
            return
        }
        AlertDialog.Builder(activity)
            .setTitle(activity.resources.getQuantityString(R.plurals.pages_remove_confirm, pages.size, pages.size))
            .setMessage(R.string.pages_remove_confirm_message)
            .setPositiveButton(R.string.pages_remove) { _, _ -> remove() }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    override fun closePages() {
        panel = null
        layout.sidePanel = null
    }
}
