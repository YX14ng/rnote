package io.github.yx14ng.rnote.viewer

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint

/**
 * Immediate feedback for the brush stroke being drawn, painted on top of the last rendered
 * frame.
 *
 * The engine renders the real stroke off the UI thread; this only bridges the latency until
 * a frame that includes it arrives. Other tools rely on the engine's own overlay. Points are
 * kept in document coordinates, so the preview follows the viewport.
 */
internal class StrokePreview {
    private var brush = Brush(Color.BLACK, BrushPalette.DEFAULT_WIDTH)
    private var style = BrushStyle.SOLID

    // x, y, pressure triples in document coordinates.
    private var points = DoubleArray(INITIAL_CAPACITY)
    private var size = 0

    private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
    }

    val isEmpty: Boolean
        get() = size == 0

    fun start(brush: Brush, style: BrushStyle) {
        this.brush = brush
        this.style = style
        size = 0
    }

    fun add(x: Double, y: Double, pressure: Double) {
        if (size + VALUES_PER_POINT > points.size) {
            points = points.copyOf(points.size * 2)
        }
        points[size] = x
        points[size + 1] = y
        points[size + 2] = pressure
        size += VALUES_PER_POINT
    }

    fun clear() {
        size = 0
    }

    fun draw(canvas: Canvas, viewport: Viewport) {
        if (isEmpty) {
            return
        }
        strokePaint.color = brush.color
        var index = 0
        while (index + VALUES_PER_POINT < size) {
            val next = index + VALUES_PER_POINT
            val pressure = (points[index + 2] + points[next + 2]) / 2
            strokePaint.strokeWidth = (StrokeWidth.at(style, brush.width, pressure) * viewport.zoom).toFloat()
            canvas.drawLine(
                screenX(points[index], viewport),
                screenY(points[index + 1], viewport),
                screenX(points[next], viewport),
                screenY(points[next + 1], viewport),
                strokePaint,
            )
            index = next
        }
    }

    private fun screenX(x: Double, viewport: Viewport) = ((x - viewport.x) * viewport.zoom).toFloat()

    private fun screenY(y: Double, viewport: Viewport) = ((y - viewport.y) * viewport.zoom).toFloat()

    private companion object {
        const val VALUES_PER_POINT = 3
        const val INITIAL_CAPACITY = 3 * 256
    }
}

/**
 * How wide the engine draws a brush stroke where the pen pressed with [pressure] (0 to 1),
 * in document units: the pressure curves of `rnote-engine`'s brush styles, which the
 * Android app leaves at their defaults. Marker ignores the pressure; solid and textured
 * strokes grow with it linearly, up to the brush width at full pressure.
 */
internal object StrokeWidth {
    fun at(style: BrushStyle, width: Double, pressure: Double): Double = when (style) {
        BrushStyle.MARKER -> width
        BrushStyle.SOLID, BrushStyle.TEXTURED -> width * pressure.coerceIn(0.0, 1.0)
    }
}
