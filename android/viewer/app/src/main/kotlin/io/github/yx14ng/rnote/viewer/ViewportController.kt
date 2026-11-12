package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.widget.OverScroller
import kotlin.math.roundToInt

/**
 * Which part of the document a view shows: fitting a page to the width, panning, zooming and
 * flinging, always keeping the document on screen.
 *
 * Every change is clamped, and the zoom stays within the engine's limits: the engine's camera
 * is set to exactly what the screen shows, since the eraser only looks at strokes inside it
 * and infinite documents grow around it. Callers redraw after calling a mutating method.
 */
internal class ViewportController(context: Context) {
    private val scroller = OverScroller(context)
    private var width = 0
    private var height = 0

    var bounds: DocumentBounds? = null
        private set

    var viewport = Viewport(0.0, 0.0, 1.0)
        private set

    /** Width of a page in document units; the view fits one page to its width. */
    private var pageWidth = 1.0

    /** How the document grows, which decides how far the view may go past its edges. */
    private var layout = DocumentLayout.INFINITE

    /** The zoom that fits a page into the view width. */
    private var fitZoom = 1.0

    /** Where fitting to the width puts the view's top-left corner. */
    private var start = DocumentPoint(0.0, 0.0)

    /**
     * Shows [newBounds] with a page of [newPageWidth] fitted to the view width, from [startAt]
     * or else from the origin page, where documents start. Fitting the bounds instead made
     * infinite documents, whose bounds keep a margin of pages around the view, grow on every
     * edit.
     */
    fun showDocument(
        newBounds: DocumentBounds,
        newPageWidth: Double,
        newLayout: DocumentLayout,
        startAt: DocumentPoint?,
    ) {
        bounds = newBounds
        pageWidth = newPageWidth
        layout = newLayout
        start = startAt ?: DocumentPoint(0.0, 0.0)
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
        if (bounds == null) {
            return
        }
        if (firstLayout) {
            fitToWidth()
            return
        }
        val relativeZoom = viewport.zoom / fitZoom
        fitZoom = pageFitZoom()
        viewport = clamp(viewport.copy(zoom = (fitZoom * relativeZoom).coerceIn(MIN_ZOOM, MAX_ZOOM)))
    }

    fun panBy(dx: Float, dy: Float) {
        viewport = clamp(viewport.copy(x = viewport.x + dx / viewport.zoom, y = viewport.y + dy / viewport.zoom))
    }

    /** Zooms to [targetZoom], keeping the document point under ([focusX], [focusY]) fixed. */
    fun zoomAround(targetZoom: Double, focusX: Float, focusY: Float) {
        val zoom = targetZoom
            .coerceIn(fitZoom * MIN_ZOOM_FACTOR, fitZoom * MAX_ZOOM_FACTOR)
            .coerceIn(MIN_ZOOM, MAX_ZOOM)
        val focusDocX = viewport.x + focusX / viewport.zoom
        val focusDocY = viewport.y + focusY / viewport.zoom
        viewport = clamp(Viewport(focusDocX - focusX / zoom, focusDocY - focusY / zoom, zoom))
    }

    /** The zoom relative to fitting a page to the width, in percent. */
    val zoomPercent: Int
        get() = (viewport.zoom / fitZoom * PERCENT).roundToInt()

    /** Fits a page to the width again, keeping the point at ([focusX], [focusY]) in place. */
    fun zoomToFit(focusX: Float, focusY: Float) = zoomAround(fitZoom, focusX, focusY)

    /** Double tap: zooms in around the point, or back out to the width if zoomed in. */
    fun toggleZoom(focusX: Float, focusY: Float) {
        val zoomedIn = viewport.zoom > fitZoom * DOUBLE_TAP_THRESHOLD
        zoomAround(if (zoomedIn) fitZoom else fitZoom * DOUBLE_TAP_ZOOM, focusX, focusY)
    }

    fun fling(velocityX: Float, velocityY: Float) {
        val range = offsetRange(viewport.zoom) ?: return
        val zoom = viewport.zoom
        val minX = (range.minX * zoom).roundToInt()
        val minY = (range.minY * zoom).roundToInt()
        val maxX = (range.maxX * zoom).roundToInt()
        val maxY = (range.maxY * zoom).roundToInt()
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

    /**
     * Whether the view is as far down as a document of pages lets it go, where pulling on
     * adds a page; the other layouts grow on their own.
     */
    val atPageEnd: Boolean
        get() {
            if (layout != DocumentLayout.FIXED_SIZE) return false
            val range = offsetRange(viewport.zoom) ?: return false
            return viewport.y >= range.maxY - END_TOLERANCE_PX / viewport.zoom
        }

    /** Scrolls so the document line [y] shows a little below the top of the view. */
    fun showFromY(y: Double) {
        viewport = clamp(viewport.copy(y = y - height * SHOW_FROM_FRACTION / viewport.zoom))
    }

    /** Document coordinates of the view point ([x], [y]). */
    fun toDocumentX(x: Float): Double = viewport.x + x / viewport.zoom

    fun toDocumentY(y: Float): Double = viewport.y + y / viewport.zoom

    private fun fitToWidth() {
        if (bounds == null || width == 0) {
            return
        }
        fitZoom = pageFitZoom()
        viewport = clamp(Viewport(start.x, start.y, fitZoom))
    }

    private fun pageFitZoom(): Double = (width / pageWidth).coerceIn(MIN_ZOOM, MAX_ZOOM)

    /** Keeps the view where the engine's camera can follow it. */
    private fun clamp(v: Viewport): Viewport {
        val range = offsetRange(v.zoom) ?: return v
        return v.copy(x = v.x.coerceIn(range.minX, range.maxX), y = v.y.coerceIn(range.minY, range.maxY))
    }

    /**
     * Where the view's top-left corner may be at [zoom], in document coordinates.
     *
     * Along an axis where the document fits on screen it is centered; otherwise the view may
     * go past its edges as far as the desktop app lets it (`Camera::surface_mins_maxs`). The
     * engine's camera takes the result as is, so both show the same area.
     */
    private fun offsetRange(zoom: Double): OffsetRange? {
        val b = bounds ?: return null
        val overshoot = OVERSHOOT_PX / zoom
        val (overshootStart, overshootEnd) = when (layout) {
            DocumentLayout.FIXED_SIZE, DocumentLayout.CONTINUOUS_VERTICAL -> overshoot to overshoot
            DocumentLayout.SEMI_INFINITE -> overshoot to 0.0
            DocumentLayout.INFINITE -> 0.0 to 0.0
        }
        val x = axisRange(b.left, b.right, overshootStart, overshootEnd, width / zoom)
        val y = axisRange(b.top, b.bottom, overshootStart, overshootEnd, height / zoom)
        return OffsetRange(minX = x.first, maxX = x.second, minY = y.first, maxY = y.second)
    }

    private fun axisRange(
        start: Double,
        end: Double,
        overshootStart: Double,
        overshootEnd: Double,
        visible: Double,
    ): Pair<Double, Double> {
        if (end - start <= visible) {
            val centered = (start + end - visible) / 2
            return centered to centered
        }
        val min = start - overshootStart
        return min to maxOf(end + overshootEnd - visible, min)
    }

    private class OffsetRange(val minX: Double, val maxX: Double, val minY: Double, val maxY: Double)

    private companion object {
        /** `Camera::ZOOM_MIN` and `Camera::ZOOM_MAX` in rnote-engine, which clamps to them. */
        const val MIN_ZOOM = 0.2
        const val MAX_ZOOM = 6.0

        /** `Camera::OVERSHOOT_HORIZONTAL` and `OVERSHOOT_VERTICAL`, in screen pixels. */
        const val OVERSHOOT_PX = 96.0
        const val PERCENT = 100.0

        const val MIN_ZOOM_FACTOR = 0.25
        const val MAX_ZOOM_FACTOR = 8.0
        const val DOUBLE_TAP_ZOOM = 2.0
        const val DOUBLE_TAP_THRESHOLD = 1.5

        /** How close to the end, in screen pixels, counts as being there. */
        const val END_TOLERANCE_PX = 1.0

        /** Where [showFromY] puts the line, as a fraction of the view height from the top. */
        const val SHOW_FROM_FRACTION = 0.15
    }
}
