package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.WindowInsets
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast

/**
 * The start screen: the library's sections on the left, their notebooks on the right, to
 * open one, create one, or organize them. Opening one starts [MainActivity] with its path.
 */
class LibraryActivity : Activity() {
    private lateinit var store: LibraryStore
    private lateinit var actions: NotebookActions
    private lateinit var grid: NotebookGrid
    private lateinit var sidebar: LibrarySidebar
    private lateinit var title: TextView
    private lateinit var viewButton: ImageButton
    private lateinit var barButtons: LinearLayout
    private lateinit var selectionBar: LinearLayout
    private lateinit var syncButton: SyncButton
    private val preferences by lazy { getSharedPreferences(PREFERENCES, MODE_PRIVATE) }

    private var snapshot = LibrarySnapshot(emptyList(), emptyList(), emptyList(), emptyList())
    private var section: LibrarySection = LibrarySection.All
    private var listMode = false
    private var sortByName = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = LibraryStore.forApp(this)
        actions = NotebookActions(this, store, { snapshot }, onChanged = ::reload, onError = ::showError)
        listMode = preferences.getBoolean(KEY_LIST_MODE, false)
        sortByName = preferences.getBoolean(KEY_SORT_BY_NAME, false)
        grid = NotebookGrid(this, ThumbnailCache(this, store)).apply {
            onOpen = ::onNotebookTapped
            onNew = ::createNotebook
            onMenu = { notebook, anchor -> actions.notebookMenu(notebook, anchor, section == LibrarySection.Trash) }
        }
        sidebar = LibrarySidebar(this).apply {
            onSection = ::showSection
            onCreateFolder = actions::createFolder
            onCreateTag = { actions.tags.create() }
            onFolderMenu = actions::folderMenu
            onTagMenu = actions::tagMenu
        }
        syncButton = SyncButton(this).apply {
            onLibraryChanged = ::reload
            view.layoutParams = LinearLayout.LayoutParams(dp(48f), dp(48f))
        }
        setContentView(buildLayout())
        // Unsaved changes kept when the app last stopped: the editor restores them first.
        if (savedInstanceState == null && Recovery(this).exists) {
            startActivity(Intent(this, MainActivity::class.java))
        }
    }

    override fun onResume() {
        super.onResume()
        // Back from the editor: the notebook changed, and so did its thumbnail's key.
        reload()
        syncButton.attach()
        SyncManager.syncNow(this)
    }

    override fun onPause() {
        super.onPause()
        syncButton.detach()
    }

    @Deprecated("Picking a file to import still goes through startActivityForResult: no AndroidX here.")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        @Suppress("DEPRECATION")
        super.onActivityResult(requestCode, resultCode, data)
        if (syncButton.onActivityResult(requestCode, resultCode, data)) {
            return
        }
        val uri = data?.data
        if (requestCode == REQUEST_IMPORT && resultCode == RESULT_OK && uri != null) {
            LibraryImport(this, store, folder = (section as? LibrarySection.Folder)?.name).import(uri) { result ->
                result.onSuccess { reload() }.onFailure(::showError)
            }
        }
    }

    private fun buildLayout(): View {
        title = TextView(this).apply {
            textSize = 20f
            setTextColor(Graphite.TEXT)
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
            gravity = Gravity.CENTER_VERTICAL
        }
        viewButton = barButton(R.drawable.ic_list, R.string.library_view_list) { toggleView() }
        updateViewButton()
        barButtons = LinearLayout(this)
        val bar = LinearLayout(this).apply {
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(Graphite.BAR)
            setPadding(dp(4f), 0, dp(8f), 0)
            addView(barButton(R.drawable.ic_menu, R.string.library_sections) { toggleSidebar() })
            addView(title, LinearLayout.LayoutParams(0, MATCH_PARENT, 1f).apply { marginStart = dp(8f) })
            addView(barButtons)
        }
        selectionBar = LinearLayout(this).apply {
            gravity = Gravity.CENTER
            setBackgroundColor(Graphite.BAR)
            visibility = View.GONE
        }
        val content = LinearLayout(this).apply {
            addView(sidebar, LinearLayout.LayoutParams(dp(SIDEBAR_WIDTH_DP), MATCH_PARENT))
            addView(FrameLayout(this@LibraryActivity).apply { addView(grid) }, LinearLayout.LayoutParams(0, MATCH_PARENT, 1f))
        }
        sidebar.visibility = if (resources.configuration.screenWidthDp >= WIDE_SCREEN_DP) View.VISIBLE else View.GONE
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Graphite.DESK)
            addView(bar, LinearLayout.LayoutParams(MATCH_PARENT, dp(60f)))
            addView(content, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
            addView(selectionBar, LinearLayout.LayoutParams(MATCH_PARENT, dp(72f)))
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                setOnApplyWindowInsetsListener { view, insets ->
                    val bars = insets.getInsets(WindowInsets.Type.systemBars())
                    view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
                    insets
                }
            }
        }
    }

    /** The buttons of the top bar, which depend on the section and on selecting. */
    private fun updateBar() {
        barButtons.removeAllViews()
        if (grid.selection != null) {
            return
        }
        barButtons.addView(syncButton.view)
        if (section == LibrarySection.Trash) {
            if (snapshot.trash.isNotEmpty()) {
                barButtons.addView(barButton(R.drawable.ic_delete, R.string.library_empty_trash) { actions.emptyTrash() })
            }
        } else {
            barButtons.addView(barButton(R.drawable.ic_open, R.string.library_import) {
                @Suppress("DEPRECATION")
                startActivityForResult(DocumentFiles.openIntent(null), REQUEST_IMPORT)
            })
        }
        barButtons.addView(barButton(R.drawable.ic_check, R.string.library_select) { startSelecting() })
        barButtons.addView(viewButton)
        barButtons.addView(barButton(R.drawable.ic_sort, R.string.library_sort) { toggleSort() })
    }

    private fun reload() {
        store.list { result ->
            result
                .onSuccess {
                    snapshot = it
                    // The folder or tag shown may be gone, e.g. deleted on this screen.
                    if (sectionGone()) section = LibrarySection.All
                    showNotebooks()
                }
                .onFailure(::showError)
        }
    }

    private fun sectionGone() = when (val current = section) {
        is LibrarySection.Folder -> current.name !in snapshot.folders
        is LibrarySection.Tag -> snapshot.tags.none { it.id == current.id }
        else -> false
    }

    private fun showSection(section: LibrarySection) {
        this.section = section
        grid.selection = null
        showNotebooks()
    }

    private fun showNotebooks() {
        val notebooks = snapshot.filter(section).let { notebooks ->
            if (sortByName) {
                notebooks.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })
            } else {
                notebooks.sortedByDescending { it.modifiedMs }
            }
        }
        title.text = when (val current = section) {
            LibrarySection.All -> getString(R.string.library_all)
            LibrarySection.Uncategorized -> getString(R.string.library_uncategorized)
            is LibrarySection.Folder -> current.name
            is LibrarySection.Tag -> snapshot.tags.firstOrNull { it.id == current.id }?.name.orEmpty()
            LibrarySection.Trash -> getString(R.string.library_trash)
        }
        val inTrash = section == LibrarySection.Trash
        grid.show(notebooks, listMode, offerNew = !inTrash && section !is LibrarySection.Tag)
        sidebar.show(snapshot, section)
        updateBar()
        updateSelectionBar()
    }

    private fun onNotebookTapped(notebook: Notebook, card: View) {
        val selection = grid.selection
        when {
            selection != null -> {
                grid.selection = if (notebook.path in selection) selection - notebook.path else selection + notebook.path
                updateSelectionBar()
            }
            section == LibrarySection.Trash -> actions.notebookMenu(notebook, card, inTrash = true)
            !notebook.isXopp -> startEditor(notebook.path, isNew = false)
            else -> {
                // A Xournal++ notebook opens as the .rnote made from it; the .xopp stays.
                Toast.makeText(this, R.string.library_importing, Toast.LENGTH_SHORT).show()
                store.call(LibraryStore.request("import_xopp", "path" to notebook.path)) { result ->
                    result.onSuccess { startEditor(it.getString("path"), isNew = false) }.onFailure(::showError)
                }
            }
        }
    }

    private fun createNotebook() {
        val folder = (section as? LibrarySection.Folder)?.name
        val request = LibraryStore.request("new_notebook_path", "folder" to folder, "stem" to getString(R.string.untitled))
        store.call(request) { result ->
            result.onSuccess { startEditor(it.getString("path"), isNew = true) }.onFailure(::showError)
        }
    }

    private fun startEditor(path: String, isNew: Boolean) {
        startActivity(
            Intent(this, MainActivity::class.java)
                .putExtra(MainActivity.EXTRA_LIBRARY_PATH, path)
                .putExtra(MainActivity.EXTRA_NEW, isNew),
        )
    }

    private fun startSelecting() {
        grid.selection = emptySet()
        updateBar()
        updateSelectionBar()
    }

    private fun stopSelecting() {
        grid.selection = null
        updateBar()
        updateSelectionBar()
    }

    /** The actions on the selected notebooks, at the bottom while selecting. */
    private fun updateSelectionBar() {
        val selection = grid.selection
        selectionBar.removeAllViews()
        selectionBar.visibility = if (selection == null) View.GONE else View.VISIBLE
        if (selection == null) {
            return
        }
        val chosen = { snapshot.filter(section).filter { it.path in selection } }
        fun action(icon: Int, label: Int, run: (List<Notebook>) -> Unit) {
            val button = PanelViews.bigButton(this, icon, label) {
                val notebooks = chosen()
                if (notebooks.isNotEmpty()) {
                    stopSelecting()
                    run(notebooks)
                }
            }
            button.alpha = if (selection.isEmpty()) DISABLED_ALPHA else 1f
            selectionBar.addView(button, LinearLayout.LayoutParams(dp(96f), MATCH_PARENT))
        }
        if (section == LibrarySection.Trash) {
            action(R.drawable.ic_undo, R.string.library_restore) { actions.restore(it) }
            action(R.drawable.ic_delete, R.string.library_delete_forever) { actions.deleteForever(it) }
        } else {
            action(R.drawable.ic_open, R.string.library_move) { notebooks -> actions.moveTo(notebooks.map { it.path }) }
            action(R.drawable.ic_check, R.string.library_tags) { notebooks -> actions.tags.assign(notebooks.map { it.path }, snapshot) }
            action(R.drawable.ic_delete, R.string.library_to_trash) { notebooks -> actions.trash(notebooks.map { it.path }) }
        }
        selectionBar.addView(PanelViews.bigButton(this, R.drawable.ic_close, android.R.string.cancel) { stopSelecting() },
            LinearLayout.LayoutParams(dp(96f), MATCH_PARENT))
    }

    private fun toggleSidebar() {
        sidebar.visibility = if (sidebar.visibility == View.VISIBLE) View.GONE else View.VISIBLE
    }

    private fun toggleView() {
        listMode = !listMode
        preferences.edit().putBoolean(KEY_LIST_MODE, listMode).apply()
        updateViewButton()
        showNotebooks()
    }

    private fun updateViewButton() {
        viewButton.setImageResource(if (listMode) R.drawable.ic_grid else R.drawable.ic_list)
        viewButton.contentDescription = getString(if (listMode) R.string.library_view_grid else R.string.library_view_list)
        viewButton.tooltipText = viewButton.contentDescription
    }

    private fun toggleSort() {
        sortByName = !sortByName
        preferences.edit().putBoolean(KEY_SORT_BY_NAME, sortByName).apply()
        val message = if (sortByName) R.string.library_sorted_by_name else R.string.library_sorted_by_date
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
        showNotebooks()
    }

    private fun barButton(icon: Int, label: Int, onClick: () -> Unit): ImageButton =
        Graphite.iconButton(this, icon, label, onClick).apply {
            layoutParams = LinearLayout.LayoutParams(dp(48f), dp(48f))
        }

    private fun showError(error: Throwable) {
        Toast.makeText(this, error.message ?: error.toString(), Toast.LENGTH_LONG).show()
    }

    private fun dp(value: Float): Int = Graphite.dp(this, value)

    private companion object {
        const val PREFERENCES = "library"
        const val KEY_LIST_MODE = "list_mode"
        const val KEY_SORT_BY_NAME = "sort_by_name"
        const val REQUEST_IMPORT = 1
        const val SIDEBAR_WIDTH_DP = 280f
        /** From this width on, the sections show beside the notebooks from the start. */
        const val WIDE_SCREEN_DP = 840
        const val DISABLED_ALPHA = 0.4f
    }
}
