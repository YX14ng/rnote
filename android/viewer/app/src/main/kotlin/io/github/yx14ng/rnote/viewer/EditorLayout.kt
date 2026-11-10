package io.github.yx14ng.rnote.viewer

import android.app.Activity
import android.os.Build
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.view.WindowInsets
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView

/**
 * The editor screen: the top bar, and below it the canvas with a centered message over it
 * (hints, loading, errors), kept clear of the system bars and the keyboard. Overlays (panels,
 * the pen wheel) cover everything.
 */
internal class EditorLayout(activity: Activity, canvas: NoteCanvasView, topBar: EditorTopBar) {
    private val messageView = TextView(activity).apply {
        gravity = Gravity.CENTER
        textSize = MESSAGE_TEXT_SIZE_SP
        setText(R.string.empty_hint)
    }

    /** The canvas and what floats over it, in the canvas's coordinates. */
    private val content = FrameLayout(activity).apply {
        addView(canvas, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
        addView(messageView, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
    }

    val root: FrameLayout = FrameLayout(activity).apply {
        setBackgroundColor(Graphite.BAR)
        addView(LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            addView(topBar, LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT))
            addView(content, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
        }, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
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

    /** The actions for the lasso's selection, floating over the canvas. */
    var selectionBar: View? = null
        set(value) {
            field?.let(content::removeView)
            field = value
            // Above the canvas, below any overlay.
            value?.let { content.addView(it, 1, FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT, Gravity.TOP or Gravity.START)) }
        }

    private var overlay: View? = null

    /** Whether an overlay (a panel, the pen wheel) shows. */
    val overlayShown: Boolean
        get() = overlay != null

    /** Shows [view] over everything, e.g. the pen wheel, replacing any other overlay. */
    fun showOverlay(view: View) {
        hideOverlay()
        overlay = view
        root.addView(view, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
    }

    fun hideOverlay() {
        overlay?.let(root::removeView)
        overlay = null
    }

    private companion object {
        const val MESSAGE_TEXT_SIZE_SP = 18f
    }
}
