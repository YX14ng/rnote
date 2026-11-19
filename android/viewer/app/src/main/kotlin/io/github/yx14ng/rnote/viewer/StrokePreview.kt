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
    private var prediction = EMPTY

    private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
    }

    val isEmpty: Boolean
        get() = size == 0

    fun start(brush: Brush, style: BrushStyle) {
        this.brush = brush
        this.style = style
        clear()
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

    /**
     * Where the pen is likely to be in a moment, `x, y, pressure` triples in document
     * coordinates: drawn after the stroke so it keeps up with the pen, replaced as real
     * samples arrive.
     */
    fun predict(predicted: DoubleArray) {
        prediction = predicted
    }

    fun clear() {
        size = 0
        prediction = EMPTY
    }

    fun draw(canvas: Canvas, viewport: Viewport) {
        if (isEmpty) {
            return
        }
        strokePaint.color = brush.color
        if (size == VALUES_PER_POINT) {
            val radius = (StrokeWidth.at(style, brush.width, points[2]) * viewport.zoom / 2).toFloat()
            strokePaint.style = Paint.Style.FILL
            canvas.drawCircle(screenX(points[0], viewport), screenY(points[1], viewport), radius, strokePaint)
            strokePaint.style = Paint.Style.STROKE
            return
        }
        val curve = PreviewCurve.smooth(points, size)
        drawPolyline(canvas, viewport, curve, curve.size)
        if (prediction.isNotEmpty()) {
            val tail = DoubleArray(VALUES_PER_POINT + prediction.size)
            curve.copyInto(tail, 0, curve.size - VALUES_PER_POINT, curve.size)
            prediction.copyInto(tail, VALUES_PER_POINT)
            drawPolyline(canvas, viewport, tail, tail.size)
        }
    }

    private fun drawPolyline(canvas: Canvas, viewport: Viewport, line: DoubleArray, length: Int) {
        var index = 0
        while (index + VALUES_PER_POINT < length) {
            val next = index + VALUES_PER_POINT
            val pressure = (line[index + 2] + line[next + 2]) / 2
            strokePaint.strokeWidth = (StrokeWidth.at(style, brush.width, pressure) * viewport.zoom).toFloat()
            canvas.drawLine(
                screenX(line[index], viewport),
                screenY(line[index + 1], viewport),
                screenX(line[next], viewport),
                screenY(line[next + 1], viewport),
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
        val EMPTY = DoubleArray(0)
    }
}

/**
 * How wide the engine draws a brush stroke where the pen pressed with [pressure] (0 to 1),
 * in document units: the pressure curves of `rnote-engine`'s brush styles. Marker and
 * ballpoint (solid with a constant curve) ignore the pressure; solid and textured strokes
 * grow with it linearly, up to the brush width at full pressure.
 */
internal object StrokeWidth {
    fun at(style: BrushStyle, width: Double, pressure: Double): Double = when (style) {
        BrushStyle.MARKER, BrushStyle.BALLPOINT -> width
        BrushStyle.SOLID, BrushStyle.TEXTURED -> width * pressure.coerceIn(0.0, 1.0)
    }
}

/**
 * The curve the live stroke is drawn along: through the middle between each two samples,
 * bending towards the samples themselves (quadratic Béziers), so corners between samples
 * come out round instead of as kinks. The ends stay where the pen started and is.
 */
internal object PreviewCurve {
    /** Pieces each bend is drawn with. */
    private const val STEPS = 4

    /** Smooths the first [size] values of [points], `x, y, pressure` triples, the same way. */
    fun smooth(points: DoubleArray, size: Int): DoubleArray {
        val count = size / 3
        if (count <= 2) return points.copyOf(count * 3)
        val out = DoubleArray((2 + (count - 2) * STEPS) * 3)
        var at = 0
        fun put(x: Double, y: Double, p: Double) {
            out[at] = x
            out[at + 1] = y
            out[at + 2] = p
            at += 3
        }
        fun mid(i: Int, k: Int) = (points[i * 3 + k] + points[(i + 1) * 3 + k]) / 2
        put(points[0], points[1], points[2])
        for (i in 1 until count - 1) {
            for (step in 1..STEPS) {
                val t = step.toDouble() / STEPS
                val a = (1 - t) * (1 - t)
                val b = 2 * (1 - t) * t
                val c = t * t
                put(
                    a * mid(i - 1, 0) + b * points[i * 3] + c * mid(i, 0),
                    a * mid(i - 1, 1) + b * points[i * 3 + 1] + c * mid(i, 1),
                    a * mid(i - 1, 2) + b * points[i * 3 + 2] + c * mid(i, 2),
                )
            }
        }
        val last = (count - 1) * 3
        put(points[last], points[last + 1], points[last + 2])
        return out
    }
}
