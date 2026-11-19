package io.github.yx14ng.rnote.viewer

import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import kotlin.math.roundToInt

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
    private val stroke = SmoothedStroke()
    private var prediction = EMPTY

    private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
    }

    // Lines of the same width go to the canvas together; a long stroke has thousands.
    private var lines = FloatArray(INITIAL_LINES * 4)
    private var lineCount = 0
    private var lineWidth = 0f

    val isEmpty: Boolean
        get() = stroke.size == 0

    fun start(brush: Brush, style: BrushStyle) {
        this.brush = brush
        this.style = style
        clear()
    }

    fun add(x: Double, y: Double, pressure: Double) = stroke.add(x, y, pressure)

    /**
     * Where the pen is likely to be in a moment, `x, y, pressure` triples in document
     * coordinates: drawn after the stroke so it keeps up with the pen, replaced as real
     * samples arrive.
     */
    fun predict(predicted: DoubleArray) {
        prediction = predicted
    }

    fun clear() {
        stroke.clear()
        prediction = EMPTY
    }

    fun draw(canvas: Canvas, viewport: Viewport) {
        if (isEmpty) {
            return
        }
        strokePaint.color = brush.color
        if (stroke.size == 1) {
            val radius = (StrokeWidth.at(style, brush.width, stroke.lastPressure) * viewport.zoom / 2).toFloat()
            strokePaint.style = Paint.Style.FILL
            canvas.drawCircle(screenX(stroke.lastX, viewport), screenY(stroke.lastY, viewport), radius, strokePaint)
            strokePaint.style = Paint.Style.STROKE
            return
        }
        stroke.forEachSegment { x0, y0, x1, y1, pressure -> line(canvas, viewport, x0, y0, x1, y1, pressure) }
        var x = stroke.lastX
        var y = stroke.lastY
        for (i in prediction.indices step VALUES_PER_POINT) {
            line(canvas, viewport, x, y, prediction[i], prediction[i + 1], prediction[i + 2])
            x = prediction[i]
            y = prediction[i + 1]
        }
        flush(canvas)
    }

    private fun line(canvas: Canvas, viewport: Viewport, x0: Double, y0: Double, x1: Double, y1: Double, pressure: Double) {
        val width = quantize((StrokeWidth.at(style, brush.width, pressure) * viewport.zoom).toFloat())
        if (width != lineWidth) {
            flush(canvas)
            lineWidth = width
        }
        if (lineCount + 4 > lines.size) lines = lines.copyOf(lines.size * 2)
        lines[lineCount] = screenX(x0, viewport)
        lines[lineCount + 1] = screenY(y0, viewport)
        lines[lineCount + 2] = screenX(x1, viewport)
        lines[lineCount + 3] = screenY(y1, viewport)
        lineCount += 4
    }

    private fun flush(canvas: Canvas) {
        if (lineCount == 0) return
        strokePaint.strokeWidth = lineWidth
        canvas.drawLines(lines, 0, lineCount, strokePaint)
        lineCount = 0
    }

    private fun screenX(x: Double, viewport: Viewport) = ((x - viewport.x) * viewport.zoom).toFloat()

    private fun screenY(y: Double, viewport: Viewport) = ((y - viewport.y) * viewport.zoom).toFloat()

    private companion object {
        const val VALUES_PER_POINT = 3
        const val INITIAL_LINES = 1024
        val EMPTY = DoubleArray(0)

        /** Widths in quarter pixels: steps nobody sees, and far fewer separate draws. */
        fun quantize(width: Float): Float = (width * 4).roundToInt() / 4f
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
    const val STEPS = 4

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
        put(points[0], points[1], points[2])
        for (i in 1 until count - 1) {
            bend(points, i, out, at)
            at += STEPS * 3
        }
        val last = (count - 1) * 3
        put(points[last], points[last + 1], points[last + 2])
        return out
    }

    /**
     * Writes into [out] at [at] the [STEPS] points of the bend at sample [i] of [points]: from
     * the middle before it to the middle after it, pulled towards the sample.
     */
    fun bend(points: DoubleArray, i: Int, out: DoubleArray, at: Int) {
        var to = at
        for (step in 1..STEPS) {
            val t = step.toDouble() / STEPS
            val a = (1 - t) * (1 - t)
            val b = 2 * (1 - t) * t
            val c = t * t
            for (k in 0 until 3) {
                val before = (points[(i - 1) * 3 + k] + points[i * 3 + k]) / 2
                val after = (points[i * 3 + k] + points[(i + 1) * 3 + k]) / 2
                out[to + k] = a * before + b * points[i * 3 + k] + c * after
            }
            to += 3
        }
    }
}

/**
 * The samples of the stroke being drawn and its [PreviewCurve], worked out once per sample
 * as they come instead of again for every frame.
 */
internal class SmoothedStroke {
    private var points = DoubleArray(3 * 256)
    private var bends = DoubleArray(3 * 1024)

    /** The number of samples. */
    var size = 0
        private set

    // How many samples have their bend worked out, after the first.
    private var bent = 0

    val lastX: Double
        get() = points[(size - 1) * 3]
    val lastY: Double
        get() = points[(size - 1) * 3 + 1]
    val lastPressure: Double
        get() = points[(size - 1) * 3 + 2]

    fun clear() {
        size = 0
        bent = 0
    }

    fun add(x: Double, y: Double, pressure: Double) {
        if ((size + 1) * 3 > points.size) points = points.copyOf(points.size * 2)
        points[size * 3] = x
        points[size * 3 + 1] = y
        points[size * 3 + 2] = pressure
        size++
    }

    /** Calls [action] with each piece of the curve: its ends and its mean pressure. */
    fun forEachSegment(action: (Double, Double, Double, Double, Double) -> Unit) {
        while (bent < size - 2) {
            val needed = (bent + 1) * PreviewCurve.STEPS * 3
            if (needed > bends.size) bends = bends.copyOf(maxOf(bends.size * 2, needed))
            PreviewCurve.bend(points, bent + 1, bends, bent * PreviewCurve.STEPS * 3)
            bent++
        }
        val count = if (size <= 2) size else 2 + (size - 2) * PreviewCurve.STEPS
        for (j in 0 until count - 1) {
            action(at(j, 0, count), at(j, 1, count), at(j + 1, 0, count), at(j + 1, 1, count), (at(j, 2, count) + at(j + 1, 2, count)) / 2)
        }
    }

    /** Value [k] of point [j] of the curve, of [count] points. */
    private fun at(j: Int, k: Int, count: Int): Double = when {
        size <= 2 || j == 0 -> points[j * 3 + k]
        j == count - 1 -> points[(size - 1) * 3 + k]
        else -> bends[(j - 1) * 3 + k]
    }
}
