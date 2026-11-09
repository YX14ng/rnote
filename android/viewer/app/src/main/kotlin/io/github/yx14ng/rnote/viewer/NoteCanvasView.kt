package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.View
import android.widget.OverScroller
import kotlin.math.roundToInt

/**
 * Shows a document with pan, pinch zoom, fling and double-tap zoom.
 *
 * Rendering happens off the UI thread through [renderer]. While a new frame is being
 * rendered, the last one is drawn transformed to the current viewport, so gestures stay
 * smooth and the image sharpens once the render arrives.
 */
class NoteCanvasView(context: Context) : View(context) {
    internal var renderer: DocumentRenderer? = null
    var onRenderError: ((Throwable) -> Unit)? = null

    private var bounds: DocumentBounds? = null
    private var viewport = Viewport(0.0, 0.0, 1.0)
    private var fitZoom = 1.0

    // Bumped for every new document, so frames of the previous one are dropped.
    private var generation = 0
    private var displayed: Frame? = null
    private var spare: Bitmap? = null
    private var renderInFlight = false
    private var renderPending = false

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
        bounds = newBounds
        fitToWidth()
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
        scaleDetector.onTouchEvent(event)
        gestureDetector.onTouchEvent(event)
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
        val frame = displayed ?: return
        val scale = (viewport.zoom / frame.viewport.zoom).toFloat()
        val dx = ((frame.viewport.x - viewport.x) * viewport.zoom).toFloat()
        val dy = ((frame.viewport.y - viewport.y) * viewport.zoom).toFloat()
        canvas.save()
        canvas.translate(dx, dy)
        canvas.scale(scale, scale)
        canvas.drawBitmap(frame.bitmap, 0f, 0f, bitmapPaint)
        canvas.restore()
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
        currentRenderer.render(viewport, width, height, reuse) { result ->
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
                    invalidate()
                } else {
                    spare = frame.bitmap
                }
            }
            .onFailure { error -> onRenderError?.invoke(error) }
        if (renderPending) {
            requestRender()
        }
    }

    private companion object {
        /** Matches `DESK_COLOR` in `crates/rnote-android/src/document_view.rs`. */
        val DESK_COLOR = Color.rgb(61, 61, 66)

        const val MIN_ZOOM_FACTOR = 0.25
        const val MAX_ZOOM_FACTOR = 8.0
        const val DOUBLE_TAP_ZOOM = 2.0
        const val DOUBLE_TAP_THRESHOLD = 1.5
    }
}
