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
 * The start screen: the notebooks of the library, to open one or create a new one. Opening
 * one starts [MainActivity] with its path.
 */
class LibraryActivity : Activity() {
    private lateinit var store: LibraryStore
    private lateinit var grid: NotebookGrid
    private lateinit var title: TextView
    private lateinit var viewButton: ImageButton
    private val preferences by lazy { getSharedPreferences(PREFERENCES, MODE_PRIVATE) }

    private var snapshot = LibrarySnapshot(emptyList(), emptyList(), emptyList(), emptyList())
    private var listMode = false
    private var sortByName = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = LibraryStore.forApp(this)
        listMode = preferences.getBoolean(KEY_LIST_MODE, false)
        sortByName = preferences.getBoolean(KEY_SORT_BY_NAME, false)
        grid = NotebookGrid(this, ThumbnailCache(this, store)).apply {
            onOpen = ::open
            onNew = ::createNotebook
        }
        setContentView(buildLayout())
    }

    override fun onResume() {
        super.onResume()
        // Back from the editor: the notebook changed, and so did its thumbnail's key.
        reload()
    }

    private fun buildLayout(): View {
        title = TextView(this).apply {
            textSize = 20f
            setTextColor(Graphite.TEXT)
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
            setText(R.string.library_all)
        }
        viewButton = Graphite.iconButton(this, R.drawable.ic_list, R.string.library_view_list) { toggleView() }
        val bar = LinearLayout(this).apply {
            gravity = Gravity.CENTER_VERTICAL
            setBackgroundColor(Graphite.BAR)
            val pad = dp(12f)
            setPadding(dp(20f), pad / 2, pad, pad / 2)
            addView(title, LinearLayout.LayoutParams(0, dp(48f), 1f).apply { gravity = Gravity.CENTER_VERTICAL })
            addView(viewButton, LinearLayout.LayoutParams(dp(48f), dp(48f)))
            addView(
                Graphite.iconButton(this@LibraryActivity, R.drawable.ic_sort, R.string.library_sort) { toggleSort() },
                LinearLayout.LayoutParams(dp(48f), dp(48f)),
            )
        }
        title.gravity = Gravity.CENTER_VERTICAL
        updateViewButton()
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(Graphite.DESK)
            addView(bar, LinearLayout.LayoutParams(MATCH_PARENT, dp(60f)))
            addView(FrameLayout(this@LibraryActivity).apply { addView(grid) }, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                setOnApplyWindowInsetsListener { view, insets ->
                    val bars = insets.getInsets(WindowInsets.Type.systemBars())
                    view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
                    insets
                }
            }
        }
    }

    private fun reload() {
        store.list { result ->
            result
                .onSuccess { snapshot = it; showNotebooks() }
                .onFailure(::showError)
        }
    }

    private fun showNotebooks() {
        val notebooks = if (sortByName) {
            snapshot.notebooks.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })
        } else {
            snapshot.notebooks.sortedByDescending { it.modifiedMs }
        }
        grid.show(notebooks, listMode, offerNew = true)
    }

    private fun open(notebook: Notebook) {
        if (!notebook.isXopp) {
            startEditor(notebook.path, isNew = false)
            return
        }
        // A Xournal++ notebook opens as the .rnote made from it; the .xopp stays.
        Toast.makeText(this, R.string.library_importing, Toast.LENGTH_SHORT).show()
        store.call(LibraryStore.request("import_xopp", "path" to notebook.path)) { result ->
            result
                .onSuccess { startEditor(it.getString("path"), isNew = false) }
                .onFailure(::showError)
        }
    }

    private fun createNotebook() {
        val request = LibraryStore.request("new_notebook_path", "folder" to null, "stem" to getString(R.string.untitled))
        store.call(request) { result ->
            result
                .onSuccess { startEditor(it.getString("path"), isNew = true) }
                .onFailure(::showError)
        }
    }

    private fun startEditor(path: String, isNew: Boolean) {
        startActivity(
            Intent(this, MainActivity::class.java)
                .putExtra(MainActivity.EXTRA_LIBRARY_PATH, path)
                .putExtra(MainActivity.EXTRA_NEW, isNew),
        )
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
        Toast.makeText(
            this,
            if (sortByName) R.string.library_sorted_by_name else R.string.library_sorted_by_date,
            Toast.LENGTH_SHORT,
        ).show()
        showNotebooks()
    }

    private fun showError(error: Throwable) {
        Toast.makeText(this, error.message ?: error.toString(), Toast.LENGTH_LONG).show()
    }

    private fun dp(value: Float): Int = Graphite.dp(this, value)

    private companion object {
        const val PREFERENCES = "library"
        const val KEY_LIST_MODE = "list_mode"
        const val KEY_SORT_BY_NAME = "sort_by_name"
    }
}
