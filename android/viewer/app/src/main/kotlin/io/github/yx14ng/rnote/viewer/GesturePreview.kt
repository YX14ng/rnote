package io.github.yx14ng.rnote.viewer

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.DashPathEffect
import android.graphics.Paint
import android.graphics.Path

/**
 * The eraser and the lasso while the pen moves, drawn by the app itself on top of the last
 * frame, as [StrokePreview] does for the brush: waiting for the engine to render each one
 * made them lag behind the pen. Frames leave the engine's own drawing of them out.
 * Positions are in document coordinates, so they follow the viewport.
 */
internal class GesturePreview {
    private var eraserX = 0.0
    private var eraserY = 0.0
    private var erasing = false

    // The lasso's points, x and y in document coordinates; drawn only while [lassoing].
    private var lasso = DoubleArray(INITIAL_POINTS * 2)
    private var lassoSize = 0

    /** Whether the selector's touch draws a lasso, rather than moving the selection. */
    var lassoing = false

    private val eraserFill = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.FILL
        color = Color.argb(160, 0xF6, 0x61, 0x51)
    }
    private val eraserOutline = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = OUTLINE_PX
        color = Color.argb(240, 0xE0, 0x1B, 0x24)
    }
    private val lassoFill = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.FILL
        color = Color.argb(24, 0x35, 0x84, 0xE4)
    }
    private val lassoOutline = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = OUTLINE_PX
        color = Color.argb(240, 0x35, 0x84, 0xE4)
        pathEffect = DashPathEffect(floatArrayOf(12f, 6f), 0f)
    }
    private val path = Path()

    /** The eraser touches the document at ([x], [y]). */
    fun eraseAt(x: Double, y: Double) {
        eraserX = x
        eraserY = y
        erasing = true
    }

    /** The selector's pen reached ([x], [y]); the lasso goes through it if [lassoing]. */
    fun lassoTo(x: Double, y: Double) {
        if (lassoSize + 2 > lasso.size) lasso = lasso.copyOf(lasso.size * 2)
        lasso[lassoSize] = x
        lasso[lassoSize + 1] = y
        lassoSize += 2
    }

    fun clear() {
        erasing = false
        lassoing = false
        lassoSize = 0
    }

    fun draw(canvas: Canvas, viewport: Viewport) {
        if (erasing) {
            val half = (ERASER_WIDTH / 2 * viewport.zoom).toFloat()
            val x = screenX(eraserX, viewport)
            val y = screenY(eraserY, viewport)
            canvas.drawRect(x - half, y - half, x + half, y + half, eraserFill)
            canvas.drawRect(x - half, y - half, x + half, y + half, eraserOutline)
        }
        if (lassoing && lassoSize >= 4) {
            path.rewind()
            path.moveTo(screenX(lasso[0], viewport), screenY(lasso[1], viewport))
            for (i in 2 until lassoSize step 2) {
                path.lineTo(screenX(lasso[i], viewport), screenY(lasso[i + 1], viewport))
            }
            path.close()
            canvas.drawPath(path, lassoFill)
            canvas.drawPath(path, lassoOutline)
        }
    }

    private fun screenX(x: Double, viewport: Viewport) = ((x - viewport.x) * viewport.zoom).toFloat()

    private fun screenY(y: Double, viewport: Viewport) = ((y - viewport.y) * viewport.zoom).toFloat()

    companion object {
        /** The eraser's side in document units: `EraserConfig::WIDTH_DEFAULT`, which the app keeps. */
        const val ERASER_WIDTH = 12.0

        private const val OUTLINE_PX = 2f
        private const val INITIAL_POINTS = 256

        /**
         * Whether a selector touch at ([x], [y]) starts a new lasso rather than moving the
         * [selection]: like the engine, a touch on the selection, or within [margin] of it
         * where its handles are, moves it.
         */
        fun startsLasso(x: Double, y: Double, selection: DocumentBounds?, margin: Double): Boolean {
            if (selection == null) return true
            val inside = x >= selection.left - margin && x <= selection.right + margin &&
                y >= selection.top - margin && y <= selection.bottom + margin
            return !inside
        }
    }
}
