package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.text.InputType
import android.view.GestureDetector
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager

/**
 * Shows and edits a document.
 *
 * Fingers pan (drag, fling) and zoom (pinch, double tap). A stylus, its eraser end or a
 * mouse draw with [tool]; fingers are ignored while the pen is down. With [fingerDrawing],
 * one finger draws too and two fingers navigate. A mouse wheel pans, and zooms with Ctrl
 * held.
 *
 * Rendering happens off the UI thread through [renderer]. While a new frame is being
 * rendered, the last one is drawn transformed to the current viewport, and the stroke being
 * drawn is previewed on top, so input stays responsive.
 */
class NoteCanvasView(context: Context) : View(context) {
    internal var renderer: DocumentRenderer?
        get() = frames.renderer
        set(value) {
            frames.renderer = value
        }
    var onError: ((Throwable) -> Unit)? = null
    var onEditState: ((EditState) -> Unit)? = null

    var tool: Tool
        get() = input.tool
        set(value) {
            input.tool = value
        }

    var brush: Brush
        get() = input.brush
        set(value) {
            input.brush = value
        }

    /** When on, a single finger draws like a stylus, and two fingers pan and zoom. */
    var fingerDrawing: Boolean
        get() = input.fingerDrawing
        set(value) {
            input.fingerDrawing = value
        }

    private val camera = ViewportController(context)
    private val bounds: DocumentBounds?
        get() = camera.bounds
    private val viewport: Viewport
        get() = camera.viewport

    private val frames = FrameScheduler(
        nextRequest = {
            if (bounds == null || width == 0 || height == 0) {
                null
            } else {
                RenderRequest(viewport, width, height, editSequence)
            }
        },
        onFrameShown = { frame ->
            if (clearPreviewAt != NO_PENDING_CLEAR && frame.editSequence >= clearPreviewAt) {
                input.preview.clear()
                clearPreviewAt = NO_PENDING_CLEAR
            }
            invalidate()
        },
        onError = { error -> onError?.invoke(error) },
    )

    // Edits are numbered so the preview is cleared once a frame includes the finished stroke.
    private var editSequence = 0
    private var clearPreviewAt = NO_PENDING_CLEAR

    private val input = CanvasInput(
        context,
        camera,
        navigate = ::navigate,
        sendPen = ::sendPen,
        onPenLifted = { clearPreviewAt = editSequence },
    )

    // Whether the text tool is editing, which shows the soft keyboard.
    private var textInputActive = false
    private val inputMethodManager = context.getSystemService(InputMethodManager::class.java)
    private val textSink = object : TextSink {
        override fun insertText(text: String) = submitEdit { it.handleText(text) }

        override fun pressKey(key: EditingKey, shift: Boolean, ctrl: Boolean) =
            submitEdit { it.handleKey(key, shift, ctrl) }

        override fun pressCharacter(codePoint: Int, shift: Boolean, ctrl: Boolean) =
            submitEdit { it.handleCharKey(codePoint, shift, ctrl) }
    }

    init {
        // Focusable, so the soft keyboard and hardware keys reach the text tool.
        isFocusable = true
        isFocusableInTouchMode = true
    }

    private val scaleDetector =
        ScaleGestureDetector(
            context,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    camera.zoomAround(viewport.zoom * detector.scaleFactor, detector.focusX, detector.focusY)
                    onViewportChanged()
                    return true
                }
            },
        )

    private val gestureDetector =
        GestureDetector(
            context,
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onDown(e: MotionEvent): Boolean {
                    camera.stopFling()
                    return true
                }

                override fun onScroll(
                    e1: MotionEvent?,
                    e2: MotionEvent,
                    distanceX: Float,
                    distanceY: Float,
                ): Boolean {
                    camera.panBy(distanceX, distanceY)
                    onViewportChanged()
                    return true
                }

                override fun onFling(
                    e1: MotionEvent?,
                    e2: MotionEvent,
                    velocityX: Float,
                    velocityY: Float,
                ): Boolean {
                    camera.fling(velocityX, velocityY)
                    postInvalidateOnAnimation()
                    return true
                }

                override fun onSingleTapConfirmed(e: MotionEvent): Boolean = performClick()

                override fun onDoubleTap(e: MotionEvent): Boolean {
                    camera.toggleZoom(e.x, e.y)
                    onViewportChanged()
                    return true
                }
            },
        )

    /** Shows a newly opened document with a page of [pageWidth] fitted to the view width. */
    fun showDocument(newBounds: DocumentBounds, pageWidth: Double) {
        frames.reset()
        input.reset()
        clearPreviewAt = NO_PENDING_CLEAR
        camera.showDocument(newBounds, pageWidth)
        onViewportChanged()
    }

    /** Fits a page to the view width again, e.g. after the page size or layout changed. */
    fun refit(pageWidth: Double) {
        val currentBounds = bounds ?: return
        camera.showDocument(currentBounds, pageWidth)
        onViewportChanged()
    }

    /** Finishes a polyline or polygon being built, like pressing Enter. */
    fun finishShape() {
        if (bounds != null && !input.penDown) {
            submitEdit { it.handleKey(EditingKey.ENTER, shift = false, ctrl = false) }
        }
    }

    /** The center of the view, in document coordinates, where pasted content goes. */
    fun documentCenter(): Pair<Double, Double> = camera.toDocumentX(width / 2f) to camera.toDocumentY(height / 2f)

    override fun onCheckIsTextEditor(): Boolean = textInputActive

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
        if (!textInputActive) {
            return null
        }
        // No suggestions: the engine owns the text, so the keyboard can't read it back.
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or
            InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or EditorInfo.IME_FLAG_NO_FULLSCREEN
        return TextInputConnection(this, textSink)
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        val finishesShape = tool == Tool.SHAPER &&
            (keyCode == KeyEvent.KEYCODE_ENTER || keyCode == KeyEvent.KEYCODE_ESCAPE)
        if (bounds != null && (textInputActive || finishesShape) && KeyMapping.dispatch(event, textSink)) {
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onHoverEvent(event: MotionEvent): Boolean {
        if (bounds != null && input.onHover(event)) {
            return true
        }
        return super.onHoverEvent(event)
    }

    /** Shows or hides the soft keyboard when the text tool starts or stops editing. */
    private fun updateTextInput(active: Boolean) {
        if (active == textInputActive) {
            return
        }
        textInputActive = active
        inputMethodManager.restartInput(this)
        if (active) {
            requestFocus()
            inputMethodManager.showSoftInput(this, 0)
        } else {
            inputMethodManager.hideSoftInputFromWindow(windowToken, 0)
        }
    }

    /** Undoes or redoes through [operation] and refreshes the view. */
    internal fun applyEdit(operation: (NativeDocument) -> EditState) {
        if (bounds == null || input.penDown) {
            return
        }
        submitEdit(operation)
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        camera.resize(w, h)
        if (textInputActive) {
            // The keyboard took part of the view: keep where the text is typed visible.
            camera.ensureVisible(input.lastPenX, input.lastPenY, TEXT_VISIBLE_MARGIN_PX)
        }
        onViewportChanged()
    }

    // Clicks are detected by gestureDetector, which calls performClick() on a single tap.
    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (bounds == null) {
            return false
        }
        input.onTouch(event)
        invalidate()
        return true
    }

    override fun onGenericMotionEvent(event: MotionEvent): Boolean {
        if (bounds == null ||
            !event.isFromSource(InputDevice.SOURCE_CLASS_POINTER) ||
            event.actionMasked != MotionEvent.ACTION_SCROLL
        ) {
            return super.onGenericMotionEvent(event)
        }
        val vertical = event.getAxisValue(MotionEvent.AXIS_VSCROLL)
        val horizontal = event.getAxisValue(MotionEvent.AXIS_HSCROLL)
        if (event.metaState and KeyEvent.META_CTRL_ON != 0) {
            camera.zoomAround(viewport.zoom * (1 + WHEEL_ZOOM_STEP * vertical), event.x, event.y)
        } else {
            camera.panBy(-horizontal * WHEEL_PAN_STEP_PX, -vertical * WHEEL_PAN_STEP_PX)
        }
        onViewportChanged()
        return true
    }

    override fun performClick(): Boolean {
        super.performClick()
        return true
    }

    override fun computeScroll() {
        if (camera.advanceFling()) {
            onViewportChanged()
            postInvalidateOnAnimation()
        }
    }

    override fun onDraw(canvas: Canvas) {
        canvas.drawColor(DESK_COLOR)
        frames.draw(canvas, viewport)
        input.preview.draw(canvas, viewport)
    }

    private fun navigate(event: MotionEvent) {
        scaleDetector.onTouchEvent(event)
        gestureDetector.onTouchEvent(event)
    }

    private fun sendPen(phase: PenPhase, tool: Tool, samples: DoubleArray) {
        val currentViewport = viewport
        val viewWidth = width
        val viewHeight = height
        submitEdit { document ->
            document.handlePen(phase, tool, samples, currentViewport, viewWidth, viewHeight)
        }
    }

    private fun submitEdit(operation: (NativeDocument) -> EditState) {
        val currentRenderer = renderer ?: return
        editSequence++
        val requestGeneration = frames.generation
        currentRenderer.edit(operation) { result ->
            if (requestGeneration != frames.generation) {
                return@edit
            }
            result
                .onSuccess { edit ->
                    edit.bounds?.let { newBounds ->
                        camera.updateBounds(newBounds)
                        // Renders requested before the edit used the old bounds.
                        onViewportChanged()
                    }
                    updateTextInput(edit.state.textInput)
                    onEditState?.invoke(edit.state)
                }
                .onFailure { error -> onError?.invoke(error) }
        }
        frames.request()
    }

    /** Redraws with the last frame right away and asks for a sharp render. */
    private fun onViewportChanged() {
        invalidate()
        frames.request()
    }

    private companion object {
        /** Matches `DESK_COLOR` in `crates/rnote-android/src/render.rs`. */
        val DESK_COLOR = Color.rgb(61, 61, 66)

        const val WHEEL_PAN_STEP_PX = 120f
        const val WHEEL_ZOOM_STEP = 0.1
        const val NO_PENDING_CLEAR = -1
        const val TEXT_VISIBLE_MARGIN_PX = 300f
    }
}
