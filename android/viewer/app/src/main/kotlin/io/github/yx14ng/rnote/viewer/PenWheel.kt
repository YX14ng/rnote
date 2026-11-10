package io.github.yx14ng.rnote.viewer

import android.animation.ValueAnimator
import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.util.TypedValue
import android.view.HapticFeedbackConstants
import android.view.MotionEvent
import android.view.View
import android.view.animation.DecelerateInterpolator
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.sin

/**
 * A wheel around the pen, opened with the stylus button: a floating disc whose outer ring
 * picks the brush color and whose inner dial sets its width. The dial's track grows thicker
 * along the way, and turning it changes the width continuously, with labelled ticks (and a
 * light snap) at the [presets]; changes apply live through [onPick]. The center previews
 * the brush. The close button at the bottom, the center or a tap outside close the wheel,
 * then call [onClose].
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
    private val discRadius = dp(164f)
    private val ringRadius = dp(134f)
    private val ringThickness = dp(34f)
    private val wellRadius = dp(108f)
    private val dialRadius = dp(83f)
    private val dialThin = dp(6f)
    private val dialThick = dp(22f)
    private val labelRadius = dp(58f)
    private val centerRadius = dp(42f)
    private val thumbRadius = dp(14f)
    private val closeRadius = dp(17f)

    private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val raised = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        setShadowLayer(dp(26f), 0f, dp(8f), SHADOW_COLOR)
    }
    private val arc = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
    }
    private val text = Paint(Paint.ANTI_ALIAS_FLAG).apply { textAlign = Paint.Align.CENTER }
    private val track = Path()
    private val filled = Path()
    private val ringBox = RectF()

    private var centerX = 0f
    private var centerY = 0f
    private var requestedX = 0f
    private var requestedY = 0f
    private var gesture = Gesture.NONE
    private var pressedSegment = -1
    private var snappedPreset: Double? = null

    /** How far the wheel has appeared, from 0 (hidden) to 1. */
    private var shown = 0f
    private var animator: ValueAnimator? = null
    private var closing = false

    init {
        contentDescription = context.getString(R.string.pen_wheel)
        // Above the tool sidebar, which floats with an elevation of its own.
        elevation = Graphite.dp(context, WHEEL_ELEVATION_DP).toFloat()
    }

    /** Centers the wheel on ([x], [y]), moved inward as needed to fit on screen. */
    fun centerAt(x: Float, y: Float) {
        requestedX = x
        requestedY = y
        placeCenter()
        invalidate()
    }

    override fun onAttachedToWindow() {
        super.onAttachedToWindow()
        animateTo(1f, APPEAR_MS) {}
    }

    override fun onDetachedFromWindow() {
        animator?.cancel()
        super.onDetachedFromWindow()
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        placeCenter()
    }

    override fun onDraw(canvas: Canvas) {
        canvas.drawColor(Color.argb((SCRIM_ALPHA * shown).toInt(), 0, 0, 0))
        val fading = shown < 1f
        if (fading) {
            canvas.saveLayerAlpha(0f, 0f, width.toFloat(), height.toFloat(), (255 * shown).toInt())
        }
        canvas.save()
        val scale = HIDDEN_SCALE + (1 - HIDDEN_SCALE) * shown
        canvas.scale(scale, scale, centerX, centerY)
        drawDisc(canvas)
        drawColorRing(canvas)
        drawDial(canvas)
        drawCenter(canvas)
        drawCloseButton(canvas)
        canvas.restore()
        if (fading) {
            canvas.restore()
        }
    }

    private fun drawDisc(canvas: Canvas) {
        raised.color = DISC_COLOR
        canvas.drawCircle(centerX, centerY, discRadius, raised)
        arc.strokeWidth = dp(1f)
        arc.color = Graphite.OUTLINE
        canvas.drawCircle(centerX, centerY, discRadius, arc)
        // A recessed well holds the dial.
        fill.color = Graphite.BAR
        canvas.drawCircle(centerX, centerY, wellRadius, fill)
        arc.color = Graphite.DIVIDER
        canvas.drawCircle(centerX, centerY, wellRadius, arc)
    }

    /** The colors as rounded pills; the current one stands out with a white edge. */
    private fun drawColorRing(canvas: Canvas) {
        val sweep = 360f / colors.size
        val capDegrees = Math.toDegrees((ringThickness / 2 / ringRadius).toDouble()).toFloat()
        val pill = (sweep - SEGMENT_GAP_DEGREES - 2 * capDegrees).coerceAtLeast(1f)
        ringBox.set(centerX - ringRadius, centerY - ringRadius, centerX + ringRadius, centerY + ringRadius)
        colors.forEachIndexed { i, segmentColor ->
            val start = -90f + i * sweep - pill / 2
            val current = segmentColor == color
            val thickness = ringThickness + if (current || i == pressedSegment) dp(6f) else 0f
            arc.color = when {
                current -> Color.WHITE
                Color.luminance(segmentColor) < DARK_LUMINANCE -> DARK_RIM_COLOR
                else -> RIM_COLOR
            }
            arc.strokeWidth = thickness + if (current) dp(7f) else dp(2f)
            canvas.drawArc(ringBox, start, pill, false, arc)
            arc.color = segmentColor
            arc.strokeWidth = thickness
            canvas.drawArc(ringBox, start, pill, false, arc)
        }
    }

    /** The width dial: a track that thickens, filled up to the width, and labelled presets. */
    private fun drawDial(canvas: Canvas) {
        val fraction = WidthDial.fractionOf(brushWidth)
        WidthDial.taper(track, centerX, centerY, dialRadius, dialThin, dialThick, 1f)
        fill.color = TRACK_COLOR
        canvas.drawPath(track, fill)
        drawCap(canvas, 0f, dialThin)
        drawCap(canvas, 1f, dialThick)
        WidthDial.taper(filled, centerX, centerY, dialRadius, dialThin, dialThick, fraction)
        fill.color = Graphite.ACCENT
        canvas.drawPath(filled, fill)
        drawCap(canvas, 0f, dialThin)

        text.textSize = sp(11f)
        text.isFakeBoldText = false
        for (preset in presets) {
            val radians = radiansAt(WidthDial.fractionOf(preset))
            val active = preset == brushWidth
            text.color = if (active) Graphite.TEXT else Graphite.TEXT_SECONDARY
            val x = centerX + labelRadius * cos(radians).toFloat()
            val y = centerY + labelRadius * sin(radians).toFloat() - (text.ascent() + text.descent()) / 2
            canvas.drawText(ToolRow.formatWidth(preset), x, y, text)
        }

        val radians = radiansAt(fraction)
        val thumbX = centerX + dialRadius * cos(radians).toFloat()
        val thumbY = centerY + dialRadius * sin(radians).toFloat()
        raised.color = Color.WHITE
        canvas.drawCircle(thumbX, thumbY, thumbRadius, raised)
        fill.color = Graphite.ACCENT
        canvas.drawCircle(thumbX, thumbY, dp(4.5f), fill)
    }

    private fun drawCap(canvas: Canvas, fraction: Float, thickness: Float) {
        val radians = radiansAt(fraction)
        canvas.drawCircle(
            centerX + dialRadius * cos(radians).toFloat(),
            centerY + dialRadius * sin(radians).toFloat(),
            thickness / 2,
            fill,
        )
    }

    /** The center previews the brush: a dot of its color and width, and the width. */
    private fun drawCenter(canvas: Canvas) {
        fill.color = Graphite.RAISED
        canvas.drawCircle(centerX, centerY, centerRadius, fill)
        arc.strokeWidth = dp(1f)
        arc.color = Graphite.OUTLINE
        canvas.drawCircle(centerX, centerY, centerRadius, arc)
        val dotRadius = (dp(2f) + (dp(13f) - dp(2f)) * WidthDial.fractionOf(brushWidth))
        val dotY = centerY - dp(8f)
        fill.color = color
        canvas.drawCircle(centerX, dotY, dotRadius, fill)
        // An edge keeps dark colors visible on the dark center.
        arc.color = DOT_EDGE_COLOR
        canvas.drawCircle(centerX, dotY, dotRadius, arc)
        text.textSize = sp(13f)
        text.isFakeBoldText = true
        text.color = Graphite.TEXT
        canvas.drawText(ToolRow.formatWidth(brushWidth), centerX, centerY + dp(26f), text)
    }

    private fun drawCloseButton(canvas: Canvas) {
        val y = centerY + dialRadius
        fill.color = Graphite.RAISED
        canvas.drawCircle(centerX, y, closeRadius, fill)
        arc.strokeWidth = dp(2f)
        arc.color = Graphite.ICON
        val armLength = dp(5f)
        canvas.drawLine(centerX - armLength, y - armLength, centerX + armLength, y + armLength, arc)
        canvas.drawLine(centerX - armLength, y + armLength, centerX + armLength, y - armLength, arc)
    }

    @SuppressLint("ClickableViewAccessibility") // Rings and a dial, not a single action.
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (closing) {
            return true
        }
        val distance = hypot(event.x - centerX, event.y - centerY)
        val degrees = Math.toDegrees(atan2((event.y - centerY).toDouble(), (event.x - centerX).toDouble())).toFloat()
        val onCloseButton = hypot(event.x - centerX, event.y - centerY - dialRadius) <= closeRadius + dp(6f)
        val onRing = distance > wellRadius && distance <= discRadius
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                gesture = when {
                    onCloseButton || distance <= centerRadius -> Gesture.CLOSE
                    distance <= wellRadius -> Gesture.DIAL
                    distance <= discRadius -> Gesture.COLOR
                    else -> Gesture.OUTSIDE
                }
                if (gesture == Gesture.DIAL) turnDial(degrees)
                if (gesture == Gesture.COLOR) pressedSegment = segmentAt(degrees)
            }
            MotionEvent.ACTION_MOVE -> when (gesture) {
                Gesture.DIAL -> turnDial(degrees)
                Gesture.COLOR -> pressedSegment = if (onRing) segmentAt(degrees) else -1
                else -> {}
            }
            MotionEvent.ACTION_UP -> {
                when (gesture) {
                    Gesture.CLOSE -> if (onCloseButton || distance <= centerRadius) dismiss()
                    Gesture.OUTSIDE -> if (distance > discRadius) dismiss()
                    Gesture.COLOR -> if (pressedSegment >= 0) pickColor(colors[pressedSegment])
                    Gesture.DIAL, Gesture.NONE -> {}
                }
                gesture = Gesture.NONE
                pressedSegment = -1
            }
            MotionEvent.ACTION_CANCEL -> {
                gesture = Gesture.NONE
                pressedSegment = -1
            }
        }
        invalidate()
        return true
    }

    private fun pickColor(picked: Int) {
        if (picked != color) {
            color = picked
            performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK)
        }
        onPick(picked, null)
    }

    /** Sets the width from the dial position at [degrees], snapping to presets nearby. */
    private fun turnDial(degrees: Float) {
        val fraction = WidthDial.fractionAt(degrees)
        val snapped = presets.firstOrNull { abs(WidthDial.fractionOf(it) - fraction) * WidthDial.SWEEP <= SNAP_DEGREES }
        if (snapped != null && snapped != snappedPreset) {
            performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK)
        }
        snappedPreset = snapped
        val width = snapped ?: (Math.round(WidthDial.widthAt(fraction) * 10) / 10.0)
        if (width != brushWidth) {
            brushWidth = width
            onPick(null, width)
        }
    }

    /** Closes with the appearing animation backwards, then [onClose]. */
    private fun dismiss() {
        closing = true
        animateTo(0f, DISMISS_MS, onClose)
    }

    private fun animateTo(target: Float, durationMs: Long, then: () -> Unit) {
        animator?.cancel()
        animator = ValueAnimator.ofFloat(shown, target).apply {
            duration = durationMs
            interpolator = DecelerateInterpolator()
            addUpdateListener {
                shown = it.animatedValue as Float
                invalidate()
            }
            doOnEnd(then)
            start()
        }
    }

    /** Which color segment, the first one on top, [degrees] falls in. */
    private fun segmentAt(degrees: Float): Int {
        val count = colors.size
        val fromTop = degrees + 90f + 180f / count
        return (fromTop.mod(360f) / (360f / count)).toInt().coerceIn(0, count - 1)
    }

    private fun radiansAt(fraction: Float) = Math.toRadians(WidthDial.angleAt(fraction).toDouble())

    private fun placeCenter() {
        val margin = discRadius + dp(12f)
        centerX = if (width > 0) requestedX.coerceIn(margin, maxOf(margin, width - margin)) else requestedX
        centerY = if (height > 0) requestedY.coerceIn(margin, maxOf(margin, height - margin)) else requestedY
    }

    private fun dp(value: Float): Float = Graphite.dp(context, value).toFloat()

    private fun sp(value: Float): Float =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, value, resources.displayMetrics)

    private enum class Gesture { NONE, CLOSE, DIAL, COLOR, OUTSIDE }

    private companion object {
        val DISC_COLOR = Color.argb(0xF7, 0x1B, 0x1C, 0x21)
        val RIM_COLOR = Color.rgb(0x3A, 0x3D, 0x45)
        val DARK_RIM_COLOR = Color.rgb(0x6B, 0x6F, 0x7A)
        val TRACK_COLOR = Color.rgb(0x2E, 0x31, 0x38)
        val DOT_EDGE_COLOR = Color.argb(0x66, 0xFF, 0xFF, 0xFF)
        val SHADOW_COLOR = Color.argb(0x80, 0x00, 0x00, 0x00)

        /** Colors darker than this get a lighter rim, to stand out on the dark disc. */
        const val DARK_LUMINANCE = 0.08f
        const val SCRIM_ALPHA = 0x38
        const val HIDDEN_SCALE = 0.86f
        const val APPEAR_MS = 170L
        const val DISMISS_MS = 120L
        const val SEGMENT_GAP_DEGREES = 7f
        const val SNAP_DEGREES = 4f
        const val WHEEL_ELEVATION_DP = 24f
    }
}

/** Runs [action] once the animation ends, unless it was cancelled. */
private fun ValueAnimator.doOnEnd(action: () -> Unit) {
    addListener(object : android.animation.AnimatorListenerAdapter() {
        private var cancelled = false

        override fun onAnimationCancel(animation: android.animation.Animator) {
            cancelled = true
        }

        override fun onAnimationEnd(animation: android.animation.Animator) {
            if (!cancelled) action()
        }
    })
}
