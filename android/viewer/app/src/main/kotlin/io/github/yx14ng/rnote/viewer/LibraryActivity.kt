package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.text.Editable
import android.text.TextUtils
import android.text.TextWatcher
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.view.WindowInsets
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.annotation.StringRes
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

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
    private lateinit var creator: NotebookCreator
    private val editorLaunch = LaunchGuard()
    private val preferences by lazy { getSharedPreferences(PREFERENCES, MODE_PRIVATE) }

    private var snapshot = LibrarySnapshot(emptyList(), emptyList(), emptyList(), emptyList())
    private var section: LibrarySection = LibrarySection.All

    // A search by name across the library, while the search field is open.
    private var query = ""
    private var searching = false
    private lateinit var searchField: EditText
    private lateinit var emptyText: TextView
    private var listMode = false
    private var sortByName = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = LibraryStore.forApp(this)
        creator = NotebookCreator(this, store)
        actions = NotebookActions(this, store, { snapshot }, onChanged = ::onLibraryChanged, onError = { showError(R.string.failed_library, it) })
        listMode = preferences.getBoolean(KEY_LIST_MODE, false)
        sortByName = preferences.getBoolean(KEY_SORT_BY_NAME, false)
        grid = NotebookGrid(this, ThumbnailCache.forApp(this)).apply {
            onOpen = ::onNotebookTapped
            onHold = ::onNotebookHeld
            onNew = ::showCreateMenu
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
            onOpenNotebook = { path -> startEditor(path, isNew = false) }
            view.layoutParams = LinearLayout.LayoutParams(dp(48f), dp(48f))
        }
        // Back from being recreated (e.g. the tablet turned): the same section, search and selection.
        if (savedInstanceState != null) {
            section = LibrarySearch.sectionFrom(savedInstanceState.getString(KEY_SECTION))
            query = savedInstanceState.getString(KEY_QUERY).orEmpty()
            searching = savedInstanceState.getBoolean(KEY_SEARCHING)
            grid.selection = savedInstanceState.getStringArrayList(KEY_SELECTION)?.toSet()
        }
        setContentView(buildLayout())
        showSearch(searching)
        // Unsaved changes kept when the app last stopped: the editor restores them first.
        if (savedInstanceState == null && Recovery(this).exists) {
            startActivity(Intent(this, MainActivity::class.java))
        }
    }

    override fun onResume() {
        super.onResume()
        editorLaunch.resumed()
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
        if (requestCode == REQUEST_PHOTO) {
            onPhotoTaken(resultCode == RESULT_OK)
            return
        }
        val uri = data?.data
        if (resultCode == RESULT_OK && uri != null) {
            when (requestCode) {
                REQUEST_PDF -> PdfImporter(this, creator, ::openCreated, { showError(R.string.failed_import, it) }).start(uri, snapshot.folders, currentFolder)
                REQUEST_IMAGE -> {
                    val name = DocumentFiles.displayName(contentResolver, uri)?.substringBeforeLast('.').orEmpty()
                    creator.fromImage(currentFolder, name, { DocumentFiles.read(contentResolver, uri) }) { result ->
                        result.onSuccess(::openCreated).onFailure { showError(R.string.failed_import, it) }
                    }
                }
            }
        }
        if (requestCode == REQUEST_IMPORT && resultCode == RESULT_OK && uri != null) {
            LibraryImport(this, store, folder = (section as? LibrarySection.Folder)?.name).import(uri) { result ->
                result.onSuccess { reload() }.onFailure { showError(R.string.failed_import, it) }
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
        searchField = EditText(this).apply {
            setSingleLine()
            setText(query)
            setHint(R.string.library_search_hint)
            setTextColor(Graphite.TEXT)
            setHintTextColor(Graphite.TEXT_SECONDARY)
            imeOptions = EditorInfo.IME_ACTION_SEARCH
            addTextChangedListener(object : TextWatcher {
                override fun beforeTextChanged(text: CharSequence?, start: Int, count: Int, after: Int) = Unit

                override fun onTextChanged(text: CharSequence?, start: Int, before: Int, count: Int) = Unit

                override fun afterTextChanged(text: Editable?) {
                    query = text?.toString().orEmpty()
                    showNotebooks()
                }
            })
        }
        emptyText = TextView(this).apply {
            setTextColor(Graphite.TEXT_SECONDARY)
            textSize = 16f
            gravity = Gravity.CENTER
            setPadding(dp(32f), dp(32f), dp(32f), dp(32f))
            visibility = View.GONE
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
            addView(searchField, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f).apply { marginStart = dp(8f) })
            addView(barButtons)
        }
        selectionBar = LinearLayout(this).apply {
            gravity = Gravity.CENTER
            setBackgroundColor(Graphite.BAR)
            visibility = View.GONE
        }
        val content = LinearLayout(this).apply {
            addView(sidebar, LinearLayout.LayoutParams(dp(SIDEBAR_WIDTH_DP), MATCH_PARENT))
            addView(
                FrameLayout(this@LibraryActivity).apply {
                    addView(grid)
                    addView(emptyText, FrameLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT, Gravity.CENTER))
                },
                LinearLayout.LayoutParams(0, MATCH_PARENT, 1f),
            )
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
        if (searching) {
            barButtons.addView(barButton(R.drawable.ic_close, R.string.library_search_close) { showSearch(false) })
        } else if (section != LibrarySection.Trash) {
            barButtons.addView(barButton(R.drawable.ic_search, R.string.library_search) { showSearch(true) })
        }
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

    /**
     * A notebook was renamed, moved, tagged or trashed here: shows it, and syncs soon so the
     * change reaches Drive while the library stays open, not only when the editor saves.
     */
    private fun onLibraryChanged() {
        reload()
        SyncManager.syncSoon(this)
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
                .onFailure { showError(R.string.failed_read_library, it) }
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

    /** Opens the search field, with the keyboard, or closes it and shows the section again. */
    private fun showSearch(open: Boolean) {
        searching = open
        title.visibility = if (open) View.GONE else View.VISIBLE
        searchField.visibility = if (open) View.VISIBLE else View.GONE
        val keyboard = getSystemService(InputMethodManager::class.java)
        if (open) {
            searchField.requestFocus()
            keyboard?.showSoftInput(searchField, InputMethodManager.SHOW_IMPLICIT)
        } else {
            keyboard?.hideSoftInputFromWindow(searchField.windowToken, 0)
            // Clearing the text shows the section again.
            if (query.isNotEmpty()) searchField.setText("")
        }
        updateBar()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putString(KEY_SECTION, LibrarySearch.sectionText(section))
        outState.putString(KEY_QUERY, query)
        outState.putBoolean(KEY_SEARCHING, searching)
        grid.selection?.let { outState.putStringArrayList(KEY_SELECTION, ArrayList(it)) }
    }

    private fun showNotebooks() {
        val searchingNow = searching && query.isNotBlank()
        val source = if (searchingNow) LibrarySearch.find(snapshot, query) else snapshot.filter(section)
        val notebooks = source.let { notebooks ->
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
        grid.show(notebooks, listMode, offerNew = !searchingNow && !inTrash && section !is LibrarySection.Tag)
        val empty = if (notebooks.isEmpty()) LibrarySearch.emptyMessage(section, if (searchingNow) query else "") else null
        emptyText.text = empty?.let { getString(it, query.trim()) }
        emptyText.visibility = if (empty != null) View.VISIBLE else View.GONE
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
                    result.onSuccess { startEditor(it.getString("path"), isNew = false) }.onFailure { showError(R.string.failed_import, it) }
                }
            }
        }
    }

    /** The folder new notebooks go to: the one shown, or none. */
    private val currentFolder: String?
        get() = (section as? LibrarySection.Folder)?.name

    private fun showCreateMenu(anchor: View) {
        val opened = { result: Result<String> -> result.onSuccess(::openCreated).onFailure { showError(R.string.failed_create, it) }; Unit }
        CreateMenu.show(this, anchor, listOf(
            CreateMenu.Item(R.drawable.ic_notebook, R.string.create_new_notebook) { showNewNotebook() },
            CreateMenu.Item(R.drawable.ic_quick_note, R.string.create_quick_note, hint = R.string.create_quick_note_hint) {
                creator.quick(currentFolder, done = opened)
            },
            CreateMenu.Item(R.drawable.ic_infinite, R.string.create_infinite_note) {
                creator.quick(currentFolder, SetupLayout.INFINITE, opened)
            },
            CreateMenu.Item(R.drawable.ic_import_pdf, R.string.create_import_pdf, detail = R.string.create_import_pdf_detail, startsGroup = true) {
                pick("application/pdf", REQUEST_PDF)
            },
            CreateMenu.Item(R.drawable.ic_image, R.string.create_import_image) { pick("image/*", REQUEST_IMAGE) },
            CreateMenu.Item(R.drawable.ic_camera, R.string.create_take_photo) {
                // Kept across the app being stopped while the camera is open.
                preferences.edit().putString(KEY_PHOTO_FOLDER, currentFolder).apply()
                PhotoCapture.start(this, REQUEST_PHOTO)
            },
        ))
    }

    /** Picks a file of [mimeType] with the system picker; it comes back with [requestCode]. */
    private fun pick(mimeType: String, requestCode: Int) {
        @Suppress("DEPRECATION")
        startActivityForResult(DocumentFiles.openIntent(null).setType(mimeType), requestCode)
    }

    /** Back from the camera: the photo becomes a notebook in the folder shown when it was taken. */
    private fun onPhotoTaken(ok: Boolean) {
        val photo = if (ok) PhotoCapture.takenPhoto(this) else null
        if (photo == null) {
            PhotoCapture.clear(this)
            return
        }
        val name = getString(R.string.create_photo_name, SimpleDateFormat(PHOTO_DATE, Locale.ROOT).format(Date()))
        val folder = preferences.getString(KEY_PHOTO_FOLDER, null)
        creator.fromImage(folder, name, { photo }) { result ->
            PhotoCapture.clear(this)
            result.onSuccess(::openCreated).onFailure { showError(R.string.failed_create, it) }
        }
    }

    private fun showNewNotebook() {
        val folder = currentFolder
        creator.defaults(folder) { result ->
            result
                .onSuccess { setup ->
                    NewNotebookScreen(this, snapshot, folder, setup) { chosenFolder, title, chosen, tags ->
                        creator.create(chosenFolder, title, chosen, tags) { created ->
                            created.onSuccess(::openCreated).onFailure { showError(R.string.failed_create, it) }
                        }
                    }.show()
                }
                .onFailure { showError(R.string.failed_create, it) }
        }
    }

    /** Opens a notebook just made in the library; it exists already, with its contents. */
    private fun openCreated(path: String) = startEditor(path, isNew = false)

    private fun startEditor(path: String, isNew: Boolean) {
        if (!editorLaunch.tryStart()) return
        startActivity(
            Intent(this, MainActivity::class.java)
                .putExtra(MainActivity.EXTRA_LIBRARY_PATH, path)
                .putExtra(MainActivity.EXTRA_NEW, isNew),
        )
    }

    /** Holding a card selects it; the trash has no selection. */
    private fun onNotebookHeld(notebook: Notebook) {
        if (section == LibrarySection.Trash) return
        grid.selection = CardSelection.held(grid.selection, notebook.path)
        updateBar()
        updateSelectionBar()
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

    /** Tells that [action] failed, and why, in words the user can act on. */
    private fun showError(@StringRes action: Int, error: Throwable) {
        Toast.makeText(this, ErrorText.of(this, action, error), Toast.LENGTH_LONG).show()
    }

    private fun dp(value: Float): Int = Graphite.dp(this, value)

    private companion object {
        const val PREFERENCES = "library"
        const val KEY_LIST_MODE = "list_mode"
        const val KEY_SORT_BY_NAME = "sort_by_name"
        const val KEY_SECTION = "section"
        const val KEY_QUERY = "query"
        const val KEY_SEARCHING = "searching"
        const val KEY_SELECTION = "selection"
        const val KEY_PHOTO_FOLDER = "photo_folder"
        const val REQUEST_IMPORT = 1
        const val REQUEST_PDF = 2
        const val REQUEST_IMAGE = 3
        const val REQUEST_PHOTO = 4
        /** Photos are named after when they were taken, e.g. "Photo 2026-11-12 10.30". */
        const val PHOTO_DATE = "yyyy-MM-dd HH.mm"
        const val SIDEBAR_WIDTH_DP = 280f
        /** From this width on, the sections show beside the notebooks from the start. */
        const val WIDE_SCREEN_DP = 840
        const val DISABLED_ALPHA = 0.4f
    }
}
