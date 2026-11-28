package io.github.yx14ng.rnote.viewer

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint

/**
 * The circle under the hovering S Pen: as wide as the pen's stroke, in its color, or as wide
 * as the eraser. A dark and a light ring around it keep it visible on any paper.
 */
internal class HoverCursor(density: Float) {
    private val ring = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = density
    }
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }

    private var x = 0f
    private var y = 0f
    private var radius: Float? = null
    private var color = Color.TRANSPARENT

    val shown: Boolean
        get() = radius != null

    /** Shows the circle of [radiusPx] at [x], [y]; [color] fills it, if any. */
    fun show(x: Float, y: Float, radiusPx: Float?, color: Int?) {
        this.x = x
        this.y = y
        radius = radiusPx
        this.color = color ?: Color.TRANSPARENT
    }

    fun hide() {
        radius = null
    }

    fun draw(canvas: Canvas) {
        val r = radius ?: return
        if (Color.alpha(color) > 0) {
            fill.color = color
            canvas.drawCircle(x, y, r, fill)
        }
        ring.color = DARK
        canvas.drawCircle(x, y, r + ring.strokeWidth, ring)
        ring.color = LIGHT
        canvas.drawCircle(x, y, r + 2 * ring.strokeWidth, ring)
    }

    companion object {
        /** The eraser's side in document units until one is chosen: the engine's default. */
        const val DEFAULT_ERASER_WIDTH = 12.0

        /** The smallest circle shown, so a thin pen far out still shows where it is. */
        const val MIN_RADIUS_PX = 1.5f

        private const val DARK = 0xA000_0000.toInt()
        private const val LIGHT = 0xA0FF_FFFF.toInt()

        /**
         * The radius in pixels of the circle for [tool] (or the eraser end, [eraserEnd]) with a
         * pen [brushWidth] wide, or an eraser [eraserWidth] wide, at [zoom]; null where hovering
         * shows nothing.
         */
        fun radiusPx(
            tool: Tool,
            eraserEnd: Boolean,
            brushWidth: Double,
            zoom: Double,
            eraserWidth: Double = DEFAULT_ERASER_WIDTH,
        ): Float? {
            val width = when {
                eraserEnd || tool == Tool.ERASER -> eraserWidth
                tool == Tool.BRUSH -> brushWidth
                else -> return null
            }
            return (width * zoom / 2).toFloat().coerceAtLeast(MIN_RADIUS_PX)
        }
    }
}
