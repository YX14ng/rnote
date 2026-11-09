package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.os.Build
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.view.WindowInsets
import android.widget.FrameLayout
import android.widget.TextView

/**
 * The editor screen: the canvas, a centered message over it (hints, loading, errors) and the
 * tool sidebar floating at the right edge, kept clear of the system bars and the keyboard.
 */
internal class EditorLayout(activity: Activity, canvas: NoteCanvasView, sidebar: ToolSidebar) {
    private val messageView = TextView(activity).apply {
        gravity = Gravity.CENTER
        textSize = MESSAGE_TEXT_SIZE_SP
        setText(R.string.empty_hint)
    }

    val root: FrameLayout = FrameLayout(activity).apply {
        addView(canvas, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
        addView(messageView, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
        // Floating beside the canvas, like the desktop app's toolbars.
        val margin = (SIDEBAR_MARGIN_DP * activity.resources.displayMetrics.density).toInt()
        addView(
            sidebar,
            FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT, Gravity.END or Gravity.CENTER_VERTICAL)
                .apply { marginEnd = margin },
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Edge-to-edge is enforced from Android 15 on: keep content out of the bars, and
            // above the keyboard so the text being typed stays visible.
            setOnApplyWindowInsetsListener { view, insets ->
                val bars = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.ime())
                view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
                insets
            }
        }
    }

    fun showMessage(text: String) {
        messageView.visibility = View.VISIBLE
        messageView.text = text
    }

    fun hideMessage() {
        messageView.visibility = View.GONE
    }

    private companion object {
        const val MESSAGE_TEXT_SIZE_SP = 18f
        const val SIDEBAR_MARGIN_DP = 18f
    }
}
