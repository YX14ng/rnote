package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.text.InputType
import android.view.GestureDetector
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import android.view.ViewConfiguration
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.widget.OverScroller
import kotlin.math.hypot
import kotlin.math.roundToInt

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
    internal var renderer: DocumentRenderer? = null
    var onError: ((Throwable) -> Unit)? = null
    var onEditState: ((EditState) -> Unit)? = null

    var tool = Tool.BRUSH
    var brush = Brush(Color.BLACK, BrushPalette.DEFAULT_WIDTH)

    /** When on, a single finger draws like a stylus, and two fingers pan and zoom. */
    var fingerDrawing = false

    private var bounds: DocumentBounds? = null
    private var viewport = Viewport(0.0, 0.0, 1.0)
    private var fitZoom = 1.0

    // Bumped for every new document, so frames of the previous one are dropped.
    private var generation = 0
    private var displayed: Frame? = null
    private var spare: Bitmap? = null
    private var renderInFlight = false
    private var renderPending = false

    // Edits are numbered so the preview is cleared once a frame includes the finished stroke.
    private var editSequence = 0
    private var penDown = false
    private var penTool = Tool.BRUSH
    private var clearPreviewAt = NO_PENDING_CLEAR
    private val preview = StrokePreview()

    // With fingerDrawing, a finger only starts drawing once it moved without a second finger.
    private var fingerGesture = FingerGesture.NONE
    private var fingerDownSamples = DoubleArray(0)
    private var fingerDownX = 0f
    private var fingerDownY = 0f
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop

    // Whether the text tool is editing, which shows the soft keyboard.
    private var textInputActive = false
    private val inputMethodManager = context.getSystemService(InputMethodManager::class.java)
    private val textSink = object : TextSink {
        override fun insertText(text: String) = submitEdit { it.handleText(text) }

        override fun pressKey(key: EditingKey, shift: Boolean, ctrl: Boolean) =
            submitEdit { it.handleKey(key, shift, ctrl) }
    }

    init {
        // Focusable, so the soft keyboard and hardware keys reach the text tool.
        isFocusable = true
        isFocusableInTouchMode = true
    }

    private val bitmapPaint = Paint(Paint.FILTER_BITMAP_FLAG)
    private val scroller = OverScroller(context)

    private val scaleDetector =
        ScaleGestureDetector(
            context,
            object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
                override fun onScale(detector: ScaleGestureDetector): Boolean {
                    zoomAround(viewport.zoom * detector.scaleFactor, detector.focusX, detector.focusY)
                    return true
                }
            },
        )

    private val gestureDetector =
        GestureDetector(
            context,
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onDown(e: MotionEvent): Boolean {
                    scroller.forceFinished(true)
                    return true
                }

                override fun onScroll(
                    e1: MotionEvent?,
                    e2: MotionEvent,
                    distanceX: Float,
                    distanceY: Float,
                ): Boolean {
                    panBy(distanceX, distanceY)
                    return true
                }

                override fun onFling(
                    e1: MotionEvent?,
                    e2: MotionEvent,
                    velocityX: Float,
                    velocityY: Float,
                ): Boolean {
                    startFling(velocityX, velocityY)
                    return true
                }

                override fun onSingleTapConfirmed(e: MotionEvent): Boolean = performClick()

                override fun onDoubleTap(e: MotionEvent): Boolean {
                    val zoomedIn = viewport.zoom > fitZoom * DOUBLE_TAP_THRESHOLD
                    zoomAround(if (zoomedIn) fitZoom else fitZoom * DOUBLE_TAP_ZOOM, e.x, e.y)
                    return true
                }
            },
        )

    /** Shows a newly opened document, fitted to the view width. */
    fun showDocument(newBounds: DocumentBounds) {
        generation++
        displayed?.let { spare = it.bitmap }
        displayed = null
        penDown = false
        preview.clear()
        clearPreviewAt = NO_PENDING_CLEAR
        bounds = newBounds
        fitToWidth()
    }

    /** Finishes a polyline or polygon being built, like pressing Enter. */
    fun finishShape() {
        if (bounds != null && !penDown) {
            submitEdit { it.handleKey(EditingKey.ENTER, shift = false, ctrl = false) }
        }
    }

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
        // Hovering moves the next segment of the shape being built. Other tools don't use it,
        // so skip the work for them.
        if (bounds == null || penDown || tool != Tool.SHAPER || !isPenEvent(event) ||
            event.actionMasked != MotionEvent.ACTION_HOVER_MOVE
        ) {
            return super.onHoverEvent(event)
        }
        penTool = tool
        sendPen(PenPhase.PROXIMITY, samplesOf(event, withHistory = true))
        return true
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
        if (bounds == null || penDown) {
            return
        }
        submitEdit(operation)
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        if (oldw == 0) {
            fitToWidth()
            return
        }
        // Keep the zoom relative to the width, so rotating keeps the same framing.
        val b = bounds ?: return
        val relativeZoom = viewport.zoom / fitZoom
        fitZoom = w / b.width
        viewport = viewport.copy(zoom = fitZoom * relativeZoom)
        updateViewport()
    }

    // Clicks are detected by gestureDetector, which calls performClick() on a single tap.
    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (bounds == null) {
            return false
        }
        if (isPenEvent(event)) {
            handlePenEvent(event)
            return true
        }
        if (penDown && fingerGesture != FingerGesture.DRAWING) {
            // Palm rejection: ignore fingers while the pen is on the surface.
            return true
        }
        if (fingerDrawing && event.getToolType(0) == MotionEvent.TOOL_TYPE_FINGER) {
            handleFingerEvent(event)
            return true
        }
        navigate(event)
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
            zoomAround(viewport.zoom * (1 + WHEEL_ZOOM_STEP * vertical), event.x, event.y)
        } else {
            panBy(-horizontal * WHEEL_PAN_STEP_PX, -vertical * WHEEL_PAN_STEP_PX)
        }
        return true
    }

    override fun performClick(): Boolean {
        super.performClick()
        return true
    }

    override fun computeScroll() {
        if (!scroller.computeScrollOffset()) {
            return
        }
        viewport =
            viewport.copy(x = scroller.currX / viewport.zoom, y = scroller.currY / viewport.zoom)
        updateViewport()
        postInvalidateOnAnimation()
    }

    override fun onDraw(canvas: Canvas) {
        canvas.drawColor(DESK_COLOR)
        val frame = displayed
        if (frame != null) {
            val scale = (viewport.zoom / frame.viewport.zoom).toFloat()
            val dx = ((frame.viewport.x - viewport.x) * viewport.zoom).toFloat()
            val dy = ((frame.viewport.y - viewport.y) * viewport.zoom).toFloat()
            canvas.save()
            canvas.translate(dx, dy)
            canvas.scale(scale, scale)
            canvas.drawBitmap(frame.bitmap, 0f, 0f, bitmapPaint)
            canvas.restore()
        }
        preview.draw(canvas, viewport)
    }

    private fun isPenEvent(event: MotionEvent): Boolean =
        when (event.getToolType(0)) {
            MotionEvent.TOOL_TYPE_STYLUS,
            MotionEvent.TOOL_TYPE_ERASER,
            MotionEvent.TOOL_TYPE_MOUSE,
            -> true
            else -> false
        }

    private fun navigate(event: MotionEvent) {
        scaleDetector.onTouchEvent(event)
        gestureDetector.onTouchEvent(event)
    }

    private fun handlePenEvent(event: MotionEvent) {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                scroller.forceFinished(true)
                beginPen(toolFor(event), samplesOf(event, withHistory = false))
            }
            MotionEvent.ACTION_MOVE -> if (penDown) continuePen(samplesOf(event, withHistory = true))
            MotionEvent.ACTION_UP -> if (penDown) endPen(samplesOf(event, withHistory = true))
            MotionEvent.ACTION_CANCEL -> if (penDown) cancelPen()
        }
        invalidate()
    }

    private fun handleFingerEvent(event: MotionEvent) {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                scroller.forceFinished(true)
                fingerGesture = FingerGesture.UNDECIDED
                fingerDownX = event.x
                fingerDownY = event.y
                fingerDownSamples = samplesOf(event, withHistory = false)
                navigate(event)
            }
            MotionEvent.ACTION_POINTER_DOWN -> when (fingerGesture) {
                FingerGesture.UNDECIDED, FingerGesture.NAVIGATING -> {
                    fingerGesture = FingerGesture.NAVIGATING
                    navigate(event)
                }
                // Extra fingers while drawing are a resting hand.
                FingerGesture.DRAWING, FingerGesture.NONE -> {}
            }
            MotionEvent.ACTION_MOVE -> when (fingerGesture) {
                FingerGesture.UNDECIDED -> {
                    if (hypot(event.x - fingerDownX, event.y - fingerDownY) > touchSlop) {
                        fingerGesture = FingerGesture.DRAWING
                        cancelNavigation(event)
                        beginPen(toolFor(event), fingerDownSamples)
                        continuePen(samplesOf(event, withHistory = true))
                    } else {
                        navigate(event)
                    }
                }
                FingerGesture.DRAWING -> continuePen(samplesOf(event, withHistory = true))
                FingerGesture.NAVIGATING -> navigate(event)
                FingerGesture.NONE -> {}
            }
            MotionEvent.ACTION_POINTER_UP -> if (fingerGesture == FingerGesture.NAVIGATING) navigate(event)
            MotionEvent.ACTION_UP -> {
                if (fingerGesture == FingerGesture.DRAWING) {
                    endPen(samplesOf(event, withHistory = true))
                } else {
                    navigate(event)
                }
                fingerGesture = FingerGesture.NONE
            }
            MotionEvent.ACTION_CANCEL -> {
                if (fingerGesture == FingerGesture.DRAWING) cancelPen() else navigate(event)
                fingerGesture = FingerGesture.NONE
            }
        }
        invalidate()
    }

    /** Resets the gesture detectors, which saw the start of what turned into a stroke. */
    private fun cancelNavigation(event: MotionEvent) {
        val cancel = MotionEvent.obtain(event)
        cancel.action = MotionEvent.ACTION_CANCEL
        navigate(cancel)
        cancel.recycle()
    }

    /** The stylus eraser end and its primary button erase; otherwise the selected tool. */
    private fun toolFor(event: MotionEvent): Tool {
        val eraserEnd = event.getToolType(0) == MotionEvent.TOOL_TYPE_ERASER
        val eraserButton = event.buttonState and MotionEvent.BUTTON_STYLUS_PRIMARY != 0
        return if (eraserEnd || eraserButton) Tool.ERASER else tool
    }

    private fun beginPen(tool: Tool, samples: DoubleArray) {
        penDown = true
        penTool = tool
        clearPreviewAt = NO_PENDING_CLEAR
        if (tool == Tool.BRUSH) preview.start(brush) else preview.clear()
        sendPen(PenPhase.DOWN, samples)
    }

    private fun continuePen(samples: DoubleArray) = sendPen(PenPhase.DOWN, samples)

    private fun endPen(samples: DoubleArray) {
        penDown = false
        sendPen(PenPhase.UP, samples)
        clearPreviewAt = editSequence
    }

    private fun cancelPen() {
        penDown = false
        sendPen(PenPhase.CANCEL, DoubleArray(0))
        clearPreviewAt = editSequence
    }

    /** `x, y, pressure` triples in document coordinates. */
    private fun samplesOf(event: MotionEvent, withHistory: Boolean): DoubleArray {
        val history = if (withHistory) event.historySize else 0
        val samples = DoubleArray((history + 1) * 3)
        // Mice and fingers report no meaningful pressure; use the middle, like the desktop app.
        val toolType = event.getToolType(0)
        val hasPressure = toolType == MotionEvent.TOOL_TYPE_STYLUS || toolType == MotionEvent.TOOL_TYPE_ERASER
        for (index in 0..history) {
            val x: Float
            val y: Float
            val rawPressure: Float
            if (index < history) {
                x = event.getHistoricalX(0, index)
                y = event.getHistoricalY(0, index)
                rawPressure = event.getHistoricalPressure(0, index)
            } else {
                x = event.x
                y = event.y
                rawPressure = event.pressure
            }
            samples[index * 3] = viewport.x + x / viewport.zoom
            samples[index * 3 + 1] = viewport.y + y / viewport.zoom
            samples[index * 3 + 2] = if (hasPressure) rawPressure.toDouble().coerceIn(0.0, 1.0) else DEFAULT_PRESSURE
        }
        return samples
    }

    private fun sendPen(phase: PenPhase, samples: DoubleArray) {
        val tool = penTool
        if (tool == Tool.BRUSH) {
            for (index in samples.indices step 3) {
                preview.add(samples[index], samples[index + 1], samples[index + 2])
            }
        }
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
        val requestGeneration = generation
        currentRenderer.edit(operation) { result ->
            if (requestGeneration != generation) {
                return@edit
            }
            result
                .onSuccess { edit ->
                    edit.bounds?.let { newBounds ->
                        bounds = newBounds
                        viewport = clamp(viewport)
                    }
                    updateTextInput(edit.state.textInput)
                    onEditState?.invoke(edit.state)
                }
                .onFailure { error -> onError?.invoke(error) }
        }
        requestRender()
    }

    private fun fitToWidth() {
        val b = bounds ?: return
        if (width == 0) {
            return
        }
        fitZoom = width / b.width
        viewport = Viewport(b.left, b.top, fitZoom)
        updateViewport()
    }

    private fun panBy(dx: Float, dy: Float) {
        viewport = viewport.copy(x = viewport.x + dx / viewport.zoom, y = viewport.y + dy / viewport.zoom)
        updateViewport()
    }

    /** Zooms to [targetZoom], keeping the document point under ([focusX], [focusY]) fixed. */
    private fun zoomAround(targetZoom: Double, focusX: Float, focusY: Float) {
        val zoom = targetZoom.coerceIn(fitZoom * MIN_ZOOM_FACTOR, fitZoom * MAX_ZOOM_FACTOR)
        val focusDocX = viewport.x + focusX / viewport.zoom
        val focusDocY = viewport.y + focusY / viewport.zoom
        viewport = Viewport(focusDocX - focusX / zoom, focusDocY - focusY / zoom, zoom)
        updateViewport()
    }

    private fun startFling(velocityX: Float, velocityY: Float) {
        val b = bounds ?: return
        val zoom = viewport.zoom
        val startX = (viewport.x * zoom).roundToInt()
        val startY = (viewport.y * zoom).roundToInt()
        val minX = (b.left * zoom).roundToInt()
        val minY = (b.top * zoom).roundToInt()
        val maxX = maxOf(minX, (b.right * zoom).roundToInt() - width)
        val maxY = maxOf(minY, (b.bottom * zoom).roundToInt() - height)
        scroller.fling(startX, startY, -velocityX.roundToInt(), -velocityY.roundToInt(), minX, maxX, minY, maxY)
        postInvalidateOnAnimation()
    }

    /** Keeps the document on screen, then redraws and requests a sharp render. */
    private fun updateViewport() {
        viewport = clamp(viewport)
        invalidate()
        requestRender()
    }

    private fun clamp(v: Viewport): Viewport {
        val b = bounds ?: return v
        val visibleWidth = width / v.zoom
        val visibleHeight = height / v.zoom
        val x = if (visibleWidth >= b.width) {
            b.left - (visibleWidth - b.width) / 2
        } else {
            v.x.coerceIn(b.left, b.right - visibleWidth)
        }
        val y = if (visibleHeight >= b.height) {
            b.top - (visibleHeight - b.height) / 2
        } else {
            v.y.coerceIn(b.top, b.bottom - visibleHeight)
        }
        return v.copy(x = x, y = y)
    }

    private fun requestRender() {
        val currentRenderer = renderer ?: return
        if (bounds == null || width == 0 || height == 0) {
            return
        }
        if (renderInFlight) {
            renderPending = true
            return
        }
        renderInFlight = true
        renderPending = false
        val requestGeneration = generation
        // The spare bitmap is not displayed, so the renderer may draw into it.
        val reuse = spare
        spare = null
        currentRenderer.render(viewport, width, height, reuse, editSequence) { result ->
            onFrameRendered(result, requestGeneration)
        }
    }

    private fun onFrameRendered(result: Result<Frame>, requestGeneration: Int) {
        renderInFlight = false
        result
            .onSuccess { frame ->
                if (requestGeneration == generation) {
                    displayed?.let { spare = it.bitmap }
                    displayed = frame
                    if (clearPreviewAt != NO_PENDING_CLEAR && frame.editSequence >= clearPreviewAt) {
                        preview.clear()
                        clearPreviewAt = NO_PENDING_CLEAR
                    }
                    invalidate()
                } else {
                    spare = frame.bitmap
                }
            }
            .onFailure { error -> onError?.invoke(error) }
        if (renderPending) {
            requestRender()
        }
    }

    private companion object {
        /** Matches `DESK_COLOR` in `crates/rnote-android/src/render.rs`. */
        val DESK_COLOR = Color.rgb(61, 61, 66)

        const val MIN_ZOOM_FACTOR = 0.25
        const val MAX_ZOOM_FACTOR = 8.0
        const val DOUBLE_TAP_ZOOM = 2.0
        const val DOUBLE_TAP_THRESHOLD = 1.5
        const val WHEEL_PAN_STEP_PX = 120f
        const val WHEEL_ZOOM_STEP = 0.1
        const val DEFAULT_PRESSURE = 0.5
        const val NO_PENDING_CLEAR = -1
    }

    private enum class FingerGesture {
        NONE,
        UNDECIDED,
        DRAWING,
        NAVIGATING,
    }
}
