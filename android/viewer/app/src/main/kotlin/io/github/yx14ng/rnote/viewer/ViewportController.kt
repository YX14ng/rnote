package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.widget.OverScroller
import kotlin.math.roundToInt

/**
 * Which part of the document a view shows: fitting it to the width, panning, zooming and
 * flinging, always keeping the document on screen.
 *
 * Every change is clamped; callers redraw after calling a mutating method.
 */
internal class ViewportController(context: Context) {
    private val scroller = OverScroller(context)
    private var width = 0
    private var height = 0

    var bounds: DocumentBounds? = null
        private set

    var viewport = Viewport(0.0, 0.0, 1.0)
        private set

    /** The zoom that fits the document width into the view. */
    private var fitZoom = 1.0

    /** Shows [newBounds] fitted to the view width, from the top. */
    fun showDocument(newBounds: DocumentBounds) {
        bounds = newBounds
        fitToWidth()
    }

    /** Follows a document that grew or shrank, without moving the view more than needed. */
    fun updateBounds(newBounds: DocumentBounds) {
        bounds = newBounds
        viewport = clamp(viewport)
    }

    /** Adapts to a new view size, keeping the zoom relative to the width (e.g. on rotation). */
    fun resize(newWidth: Int, newHeight: Int) {
        val firstLayout = width == 0
        width = newWidth
        height = newHeight
        val b = bounds ?: return
        if (firstLayout) {
            fitToWidth()
            return
        }
        val relativeZoom = viewport.zoom / fitZoom
        fitZoom = width / b.width
        viewport = clamp(viewport.copy(zoom = fitZoom * relativeZoom))
    }

    fun panBy(dx: Float, dy: Float) {
        viewport = clamp(viewport.copy(x = viewport.x + dx / viewport.zoom, y = viewport.y + dy / viewport.zoom))
    }

    /** Zooms to [targetZoom], keeping the document point under ([focusX], [focusY]) fixed. */
    fun zoomAround(targetZoom: Double, focusX: Float, focusY: Float) {
        val zoom = targetZoom.coerceIn(fitZoom * MIN_ZOOM_FACTOR, fitZoom * MAX_ZOOM_FACTOR)
        val focusDocX = viewport.x + focusX / viewport.zoom
        val focusDocY = viewport.y + focusY / viewport.zoom
        viewport = clamp(Viewport(focusDocX - focusX / zoom, focusDocY - focusY / zoom, zoom))
    }

    /** Double tap: zooms in around the point, or back out to the width if zoomed in. */
    fun toggleZoom(focusX: Float, focusY: Float) {
        val zoomedIn = viewport.zoom > fitZoom * DOUBLE_TAP_THRESHOLD
        zoomAround(if (zoomedIn) fitZoom else fitZoom * DOUBLE_TAP_ZOOM, focusX, focusY)
    }

    fun fling(velocityX: Float, velocityY: Float) {
        val b = bounds ?: return
        val zoom = viewport.zoom
        val minX = (b.left * zoom).roundToInt()
        val minY = (b.top * zoom).roundToInt()
        val maxX = maxOf(minX, (b.right * zoom).roundToInt() - width)
        val maxY = maxOf(minY, (b.bottom * zoom).roundToInt() - height)
        scroller.fling(
            (viewport.x * zoom).roundToInt(),
            (viewport.y * zoom).roundToInt(),
            -velocityX.roundToInt(),
            -velocityY.roundToInt(),
            minX,
            maxX,
            minY,
            maxY,
        )
    }

    fun stopFling() = scroller.forceFinished(true)

    /** Advances a running fling. Returns whether it is still running. */
    fun advanceFling(): Boolean {
        if (!scroller.computeScrollOffset()) {
            return false
        }
        viewport = clamp(viewport.copy(x = scroller.currX / viewport.zoom, y = scroller.currY / viewport.zoom))
        return true
    }

    /** Pans just enough to show the document point ([x], [y]) with [marginPx] around it. */
    fun ensureVisible(x: Double, y: Double, marginPx: Float) {
        val margin = marginPx / viewport.zoom
        val visibleWidth = width / viewport.zoom
        val visibleHeight = height / viewport.zoom
        val newX = when {
            x - margin < viewport.x -> x - margin
            x + margin > viewport.x + visibleWidth -> x + margin - visibleWidth
            else -> viewport.x
        }
        val newY = when {
            y - margin < viewport.y -> y - margin
            y + margin > viewport.y + visibleHeight -> y + margin - visibleHeight
            else -> viewport.y
        }
        viewport = clamp(viewport.copy(x = newX, y = newY))
    }

    /** Document coordinates of the view point ([x], [y]). */
    fun toDocumentX(x: Float): Double = viewport.x + x / viewport.zoom

    fun toDocumentY(y: Float): Double = viewport.y + y / viewport.zoom

    private fun fitToWidth() {
        val b = bounds ?: return
        if (width == 0) {
            return
        }
        fitZoom = width / b.width
        viewport = clamp(Viewport(b.left, b.top, fitZoom))
    }

    /** Centers the document along an axis where it fits, else keeps the view inside it. */
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

    private companion object {
        const val MIN_ZOOM_FACTOR = 0.25
        const val MAX_ZOOM_FACTOR = 8.0
        const val DOUBLE_TAP_ZOOM = 2.0
        const val DOUBLE_TAP_THRESHOLD = 1.5
    }
}
