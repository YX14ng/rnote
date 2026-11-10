package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.view.MotionEvent
import android.view.View
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.hypot
import kotlin.math.ln
import kotlin.math.sin

/**
 * A wheel around the pen, opened with the stylus button: the outer ring picks the brush
 * color, the inner ring is a dial for its width. Turning the dial changes the width
 * continuously, with ticks (and a light snap) at the [presets]; the change applies live
 * through [onPick]. The center or a tap outside closes it through [onClose].
 */
@SuppressLint("ViewConstructor") // Built in code only, with what it shows.
internal class PenWheel(
    context: Context,
    private val colors: List<Int>,
    private val presets: List<Double>,
    private var color: Int,
    private var brushWidth: Double,
    private val onPick: (color: Int?, width: Double?) -> Unit,
    private val onClose: () -> Unit,
) : View(context) {
    private val outerRadius = dp(150f)
    private val colorRingInner = dp(100f)
    private val dialRadius = dp(76f)
    private val dialThickness = dp(22f)
    private val centerRadius = dp(44f)
    private val thumbRadius = dp(19f)

    private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val line = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
    }
    private val label = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Graphite.BAR
        textAlign = Paint.Align.CENTER
        textSize = dp(14f)
        isFakeBoldText = true
    }

    private var centerX = 0f
    private var centerY = 0f
    private var requestedX = 0f
    private var requestedY = 0f
    private var gesture = Gesture.NONE

    init {
        contentDescription = context.getString(R.string.pen_wheel)
    }

    /** Centers the wheel on ([x], [y]), moved inward as needed to fit on screen. */
    fun centerAt(x: Float, y: Float) {
        requestedX = x
        requestedY = y
        placeCenter()
        invalidate()
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        placeCenter()
    }

    override fun onDraw(canvas: Canvas) {
        canvas.drawColor(SCRIM_COLOR)
        drawColorRing(canvas)
        fill.color = DISK_COLOR
        canvas.drawCircle(centerX, centerY, colorRingInner - dp(2f), fill)
        drawDial(canvas)
        // The center closes the wheel.
        fill.color = Graphite.SURFACE
        canvas.drawCircle(centerX, centerY, centerRadius, fill)
        line.color = Color.WHITE
        line.strokeWidth = dp(2.5f)
        canvas.drawCircle(centerX, centerY, centerRadius, line)
        val arm = centerRadius / 3
        line.strokeWidth = dp(3f)
        canvas.drawLine(centerX - arm, centerY - arm, centerX + arm, centerY + arm, line)
        canvas.drawLine(centerX - arm, centerY + arm, centerX + arm, centerY - arm, line)
    }

    private fun drawColorRing(canvas: Canvas) {
        val count = colors.size
        val sweep = 360f / count
        val outer = box(outerRadius)
        val inner = box(colorRingInner)
        colors.forEachIndexed { i, segmentColor ->
            val start = -90f - sweep / 2 + i * sweep
            val path = Path().apply {
                arcTo(outer, start, sweep)
                arcTo(inner, start + sweep, -sweep)
                close()
            }
            fill.color = segmentColor
            canvas.drawPath(path, fill)
            line.color = Graphite.BAR
            line.strokeWidth = dp(1.5f)
            canvas.drawPath(path, line)
            if (segmentColor == color) {
                // The current color gets a white edge along the outside.
                line.color = Color.WHITE
                line.strokeWidth = dp(5f)
                canvas.drawArc(box(outerRadius - dp(2.5f)), start + 2, sweep - 4, false, line)
            }
        }
    }

    private fun drawDial(canvas: Canvas) {
        val track = box(dialRadius)
        line.strokeWidth = dialThickness
        line.color = TRACK_COLOR
        canvas.drawArc(track, DIAL_START, DIAL_SWEEP, false, line)
        val angle = angleFor(brushWidth)
        line.color = Graphite.ACCENT
        canvas.drawArc(track, DIAL_START, angle - DIAL_START, false, line)
        // Ticks at the preset widths, just inside the track.
        line.color = TICK_COLOR
        line.strokeWidth = dp(2f)
        for (preset in presets) {
            val radians = Math.toRadians(angleFor(preset).toDouble())
            val from = dialRadius - dialThickness / 2 - dp(3f)
            val to = from - dp(7f)
            canvas.drawLine(
                centerX + from * cos(radians).toFloat(), centerY + from * sin(radians).toFloat(),
                centerX + to * cos(radians).toFloat(), centerY + to * sin(radians).toFloat(),
                line,
            )
        }
        val radians = Math.toRadians(angle.toDouble())
        val thumbX = centerX + dialRadius * cos(radians).toFloat()
        val thumbY = centerY + dialRadius * sin(radians).toFloat()
        fill.color = Color.WHITE
        canvas.drawCircle(thumbX, thumbY, thumbRadius, fill)
        canvas.drawText(ToolSidebar.formatWidth(brushWidth), thumbX, thumbY - (label.ascent() + label.descent()) / 2, label)
    }

    @SuppressLint("ClickableViewAccessibility") // Rings and a dial, not a single action.
    override fun onTouchEvent(event: MotionEvent): Boolean {
        val distance = hypot(event.x - centerX, event.y - centerY)
        val degrees = Math.toDegrees(atan2((event.y - centerY).toDouble(), (event.x - centerX).toDouble())).toFloat()
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                gesture = when {
                    distance <= centerRadius -> Gesture.CLOSE
                    distance <= colorRingInner -> Gesture.DIAL
                    distance <= outerRadius -> Gesture.COLOR
                    else -> Gesture.OUTSIDE
                }
                if (gesture == Gesture.DIAL) turnDial(degrees)
            }
            MotionEvent.ACTION_MOVE -> if (gesture == Gesture.DIAL) turnDial(degrees)
            MotionEvent.ACTION_UP -> {
                when (gesture) {
                    Gesture.CLOSE -> if (distance <= centerRadius) onClose()
                    Gesture.OUTSIDE -> if (distance > outerRadius) onClose()
                    Gesture.COLOR -> if (distance in colorRingInner..outerRadius) {
                        color = colors[segmentAt(degrees, colors.size)]
                        onPick(color, null)
                    }
                    Gesture.DIAL, Gesture.NONE -> {}
                }
                gesture = Gesture.NONE
            }
            MotionEvent.ACTION_CANCEL -> gesture = Gesture.NONE
        }
        invalidate()
        return true
    }

    /** Sets the width from the dial position at [degrees], snapping to presets nearby. */
    private fun turnDial(degrees: Float) {
        var along = (degrees - DIAL_START).mod(360f)
        if (along > DIAL_SWEEP) {
            // In the gap at the bottom: the nearer end.
            along = if (along > DIAL_SWEEP + (360f - DIAL_SWEEP) / 2) 0f else DIAL_SWEEP
        }
        val snapped = presets.firstOrNull { abs(angleFor(it) - DIAL_START - along) <= SNAP_DEGREES }
        val width = snapped ?: widthAt(along / DIAL_SWEEP).let { Math.round(it * 10) / 10.0 }
        if (width != brushWidth) {
            brushWidth = width
            onPick(null, width)
        }
    }

    /** The dial angle (degrees, screen orientation) for [width], on a logarithmic scale. */
    private fun angleFor(width: Double): Float {
        val t = ln(width.coerceIn(MIN_WIDTH, MAX_WIDTH) / MIN_WIDTH) / ln(MAX_WIDTH / MIN_WIDTH)
        return DIAL_START + (t * DIAL_SWEEP).toFloat()
    }

    private fun widthAt(t: Float): Double = MIN_WIDTH * exp(t * ln(MAX_WIDTH / MIN_WIDTH))

    /** Which of [count] segments, the first one on top, [degrees] falls in. */
    private fun segmentAt(degrees: Float, count: Int): Int {
        val fromTop = degrees + 90f + 180f / count
        return (fromTop.mod(360f) / (360f / count)).toInt().coerceIn(0, count - 1)
    }

    private fun box(radius: Float) = RectF(centerX - radius, centerY - radius, centerX + radius, centerY + radius)

    private fun placeCenter() {
        val margin = outerRadius + dp(8f)
        centerX = if (width > 0) requestedX.coerceIn(margin, maxOf(margin, width - margin)) else requestedX
        centerY = if (height > 0) requestedY.coerceIn(margin, maxOf(margin, height - margin)) else requestedY
    }

    private fun dp(value: Float): Float = Graphite.dp(context, value).toFloat()

    private enum class Gesture { NONE, CLOSE, DIAL, COLOR, OUTSIDE }

    private companion object {
        val DISK_COLOR = Color.rgb(0x2E, 0x30, 0x36)
        val TRACK_COLOR = Color.rgb(0x44, 0x47, 0x4F)
        val TICK_COLOR = Color.rgb(0xC9, 0xCB, 0xD2)
        val SCRIM_COLOR = Color.argb(0x1A, 0x00, 0x00, 0x00)

        /** The dial runs clockwise from bottom left to bottom right, open at the bottom. */
        const val DIAL_START = 120f
        const val DIAL_SWEEP = 300f
        const val SNAP_DEGREES = 4f
        const val MIN_WIDTH = 0.5
        const val MAX_WIDTH = 20.0
    }
}
