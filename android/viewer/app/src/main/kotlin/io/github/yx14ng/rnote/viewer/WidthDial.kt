package io.github.yx14ng.rnote.viewer

import android.graphics.Path
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.ln
import kotlin.math.sin

/**
 * The scale of the pen wheel's width dial: an arc open at the bottom, running clockwise
 * from bottom left to bottom right, with widths on a logarithmic scale so thin widths get
 * as much room as thick ones. Angles are in degrees, in screen orientation.
 */
internal object WidthDial {
    const val START = 120f
    const val SWEEP = 300f
    const val MIN_WIDTH = 0.5
    const val MAX_WIDTH = 20.0

    /** Where [width] sits along the dial, from 0 at the start to 1 at the end. */
    fun fractionOf(width: Double): Float =
        (ln(width.coerceIn(MIN_WIDTH, MAX_WIDTH) / MIN_WIDTH) / ln(MAX_WIDTH / MIN_WIDTH)).toFloat()

    fun widthAt(fraction: Float): Double = MIN_WIDTH * exp(fraction * ln(MAX_WIDTH / MIN_WIDTH))

    fun angleAt(fraction: Float): Float = START + fraction * SWEEP

    /** The fraction for a touch at [degrees]; in the gap at the bottom, the nearer end. */
    fun fractionAt(degrees: Float): Float {
        val along = (degrees - START).mod(360f)
        if (along <= SWEEP) {
            return along / SWEEP
        }
        return if (along > SWEEP + (360f - SWEEP) / 2) 0f else 1f
    }

    /**
     * Sets [path] to the dial's track from its start to [fraction], [thin] wide at the start
     * and growing to [thick] at the end, so the track itself shows how wide the brush is.
     * The ends are flat; round caps are drawn apart.
     */
    fun taper(path: Path, cx: Float, cy: Float, radius: Float, thin: Float, thick: Float, fraction: Float) {
        path.reset()
        val steps = maxOf(2, (fraction * TAPER_STEPS).toInt())
        // Out along the outer edge, back along the inner one.
        for (step in 0..2 * steps + 1) {
            val outward = step <= steps
            val i = if (outward) step else 2 * steps + 1 - step
            val t = fraction * i / steps
            val half = thicknessAt(thin, thick, t) / 2
            val r = if (outward) radius + half else radius - half
            val radians = Math.toRadians(angleAt(t).toDouble())
            val x = cx + r * cos(radians).toFloat()
            val y = cy + r * sin(radians).toFloat()
            if (step == 0) path.moveTo(x, y) else path.lineTo(x, y)
        }
        path.close()
    }

    /** How wide the track is at [fraction]. */
    fun thicknessAt(thin: Float, thick: Float, fraction: Float): Float = thin + (thick - thin) * fraction

    private const val TAPER_STEPS = 72
}
