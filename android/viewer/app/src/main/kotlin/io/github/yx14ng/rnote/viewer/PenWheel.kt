package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.util.TypedValue
import android.view.MotionEvent
import android.view.View
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.sin

/**
 * A wheel around the pen to pick the brush color (outer ring) and width (inner ring),
 * opened with the stylus button. Covers the screen while shown: picking applies right away
 * through [onPick] and keeps the wheel open, so both can be chosen; the center or a tap
 * outside closes it through [onClose].
 */
@SuppressLint("ViewConstructor") // Built in code only, with what it shows.
internal class PenWheel(
    context: Context,
    private val colors: List<Int>,
    private val widths: List<Double>,
    private var color: Int,
    private var brushWidth: Double,
    private val onPick: (color: Int?, width: Double?) -> Unit,
    private val onClose: () -> Unit,
) : View(context) {
    private val outerRadius = dp(150f)
    private val colorRingInner = dp(100f)
    private val widthRingInner = dp(48f)
    private val centerRadius = dp(44f)

    private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val outline = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = dp(2f)
        color = BAR_COLOR
    }
    private val highlight = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = dp(4f)
        color = Color.WHITE
    }

    private var centerX = 0f
    private var centerY = 0f
    private var requestedX = 0f
    private var requestedY = 0f

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
        drawRing(colors.size, colorRingInner, outerRadius) { i, path ->
            fill.color = colors[i]
            canvas.drawPath(path, fill)
            canvas.drawPath(path, outline)
            if (colors[i] == color) canvas.drawPath(path, highlight)
        }
        drawRing(widths.size, widthRingInner, colorRingInner) { i, path ->
            fill.color = if (widths[i] == brushWidth) HIGHLIGHT_COLOR else BAR_COLOR
            canvas.drawPath(path, fill)
            canvas.drawPath(path, outline)
            // A dot as thick as the brush at the fit zoom, in the current color.
            val angle = segmentMiddle(i, widths.size)
            val radius = (widthRingInner + colorRingInner) / 2
            fill.color = color
            canvas.drawCircle(
                centerX + radius * cos(angle),
                centerY + radius * sin(angle),
                (dp(2f) + widths[i].toFloat() * dp(1.2f)).coerceAtMost(dp(16f)),
                fill,
            )
        }
        fill.color = BAR_COLOR
        canvas.drawCircle(centerX, centerY, centerRadius, fill)
        // A cross: the center closes the wheel.
        val arm = centerRadius / 3
        highlight.strokeWidth = dp(3f)
        canvas.drawLine(centerX - arm, centerY - arm, centerX + arm, centerY + arm, highlight)
        canvas.drawLine(centerX - arm, centerY + arm, centerX + arm, centerY - arm, highlight)
        highlight.strokeWidth = dp(4f)
    }

    @SuppressLint("ClickableViewAccessibility") // Taps pick from rings, not a single action.
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.actionMasked != MotionEvent.ACTION_UP) {
            return true
        }
        val distance = hypot(event.x - centerX, event.y - centerY)
        val angle = atan2(event.y - centerY, event.x - centerX)
        when {
            distance <= centerRadius || distance > outerRadius -> onClose()
            distance >= colorRingInner -> {
                color = colors[segmentAt(angle, colors.size)]
                onPick(color, null)
            }
            distance >= widthRingInner -> {
                brushWidth = widths[segmentAt(angle, widths.size)]
                onPick(null, brushWidth)
            }
            else -> onClose()
        }
        invalidate()
        return true
    }

    /** Draws [count] equal segments between [inner] and [outer], the first one on top. */
    private inline fun drawRing(count: Int, inner: Float, outer: Float, draw: (Int, Path) -> Unit) {
        val sweep = 360f / count
        val outerBox = RectF(centerX - outer, centerY - outer, centerX + outer, centerY + outer)
        val innerBox = RectF(centerX - inner, centerY - inner, centerX + inner, centerY + inner)
        for (i in 0 until count) {
            val start = -90f - sweep / 2 + i * sweep
            val path = Path().apply {
                arcTo(outerBox, start, sweep)
                arcTo(innerBox, start + sweep, -sweep)
                close()
            }
            draw(i, path)
        }
    }

    /** The middle angle of segment [i] of [count], in radians, the first one on top. */
    private fun segmentMiddle(i: Int, count: Int): Float =
        Math.toRadians((-90.0 + i * 360.0 / count)).toFloat()

    /** Which of [count] segments, the first one on top, the [angle] (radians) falls in. */
    private fun segmentAt(angle: Float, count: Int): Int {
        val fromTop = Math.toDegrees(angle.toDouble()) + 90.0 + 180.0 / count
        return ((fromTop.mod(360.0)) / (360.0 / count)).toInt().coerceIn(0, count - 1)
    }

    private fun placeCenter() {
        val margin = outerRadius + dp(8f)
        centerX = if (width > 0) requestedX.coerceIn(margin, maxOf(margin, width - margin)) else requestedX
        centerY = if (height > 0) requestedY.coerceIn(margin, maxOf(margin, height - margin)) else requestedY
    }

    private fun dp(value: Float): Float =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, value, resources.displayMetrics)

    private companion object {
        val BAR_COLOR = Color.argb(0xF2, 0x30, 0x30, 0x35)
        val HIGHLIGHT_COLOR = Color.argb(0xFF, 0x35, 0x84, 0xE4)
        val SCRIM_COLOR = Color.argb(0x26, 0x00, 0x00, 0x00)
    }
}
