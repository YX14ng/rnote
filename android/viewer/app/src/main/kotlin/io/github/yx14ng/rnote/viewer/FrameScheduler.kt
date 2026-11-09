package io.github.yx14ng.rnote.viewer

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint

/** What to render next: the region on screen and the last edit it must include. */
internal data class RenderRequest(
    val viewport: Viewport,
    val width: Int,
    val height: Int,
    val editSequence: Int,
)

/**
 * Keeps at most one render in flight and the last finished frame on screen.
 *
 * Requests that arrive while rendering are coalesced into one more render with the latest
 * state, taken from [nextRequest] when it starts. Two bitmaps alternate: the displayed one
 * is never handed to the renderer, which draws into the other.
 */
internal class FrameScheduler(
    private val nextRequest: () -> RenderRequest?,
    private val onFrameShown: (Frame) -> Unit,
    private val onError: (Throwable) -> Unit,
) {
    var renderer: DocumentRenderer? = null

    /** Bumped for every new document, so results for the previous one are dropped. */
    var generation = 0
        private set

    private var displayed: Frame? = null
    private var spare: Bitmap? = null
    private var inFlight = false
    private var pending = false
    private val bitmapPaint = Paint(Paint.FILTER_BITMAP_FLAG)

    /** Forgets the current document's frames. */
    fun reset() {
        generation++
        displayed?.let { spare = it.bitmap }
        displayed = null
    }

    fun request() {
        val currentRenderer = renderer ?: return
        if (inFlight) {
            pending = true
            return
        }
        val next = nextRequest() ?: return
        inFlight = true
        pending = false
        val requestGeneration = generation
        // The spare bitmap is not displayed, so the renderer may draw into it.
        val reuse = spare
        spare = null
        currentRenderer.render(next.viewport, next.width, next.height, reuse, next.editSequence) { result ->
            onRendered(result, requestGeneration)
        }
    }

    /** Draws the last frame transformed to [viewport], so it follows gestures until a new one. */
    fun draw(canvas: Canvas, viewport: Viewport) {
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

    private fun onRendered(result: Result<Frame>, requestGeneration: Int) {
        inFlight = false
        result
            .onSuccess { frame ->
                if (requestGeneration == generation) {
                    displayed?.let { spare = it.bitmap }
                    displayed = frame
                    onFrameShown(frame)
                } else {
                    spare = frame.bitmap
                }
            }
            .onFailure(onError)
        if (pending) {
            request()
        }
    }
}
