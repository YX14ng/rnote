package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import android.view.Gravity
import android.view.Menu
import android.view.MenuItem
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.WindowInsets
import android.widget.FrameLayout
import android.widget.TextView
import java.io.IOException

/** Opens `.rnote` files from other apps or the system file picker and shows them. */
class MainActivity : Activity() {
    private val renderer = DocumentRenderer()
    private lateinit var canvasView: NoteCanvasView
    private lateinit var messageView: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        canvasView = NoteCanvasView(this).apply {
            renderer = this@MainActivity.renderer
            onRenderError = ::showError
        }
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
        handleIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    override fun onCreateOptionsMenu(menu: Menu): Boolean {
        menu.add(Menu.NONE, MENU_OPEN, Menu.NONE, R.string.action_open)
            .setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        if (item.itemId != MENU_OPEN) {
            return super.onOptionsItemSelected(item)
        }
        val pick = Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("*/*")
        startActivityForResult(pick, REQUEST_OPEN)
        return true
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == REQUEST_OPEN && resultCode == RESULT_OK) {
            data?.data?.let(::openDocument)
        }
    }

    override fun onDestroy() {
        renderer.close()
        super.onDestroy()
    }

    private fun handleIntent(intent: Intent) {
        if (intent.action == Intent.ACTION_VIEW) {
            intent.data?.let(::openDocument)
        }
    }

    private fun openDocument(uri: Uri) {
        messageView.visibility = View.VISIBLE
        messageView.setText(R.string.loading)
        title = displayName(uri) ?: getString(R.string.app_name)
        renderer.open(
            load = {
                contentResolver.openInputStream(uri)?.use { it.readBytes() }
                    ?: throw IOException("Cannot read $uri")
            },
        ) { result ->
            result
                .onSuccess { bounds ->
                    messageView.visibility = View.GONE
                    canvasView.showDocument(bounds)
                }
                .onFailure(::showError)
        }
    }

    private fun displayName(uri: Uri): String? =
        runCatching {
            contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
        }.getOrNull()

    private fun showError(error: Throwable) {
        Log.e(TAG, "Opening or rendering the document failed", error)
        messageView.visibility = View.VISIBLE
        messageView.text = getString(R.string.open_failed, error.message ?: error.toString())
    }

    private companion object {
        const val TAG = "RnoteViewer"
        const val MENU_OPEN = 1
        const val REQUEST_OPEN = 1
        const val MESSAGE_TEXT_SIZE_SP = 18f
    }
}
