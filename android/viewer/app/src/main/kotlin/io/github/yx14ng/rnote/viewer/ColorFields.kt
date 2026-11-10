package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapShader
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.ComposeShader
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.PorterDuff
import android.graphics.RectF
import android.graphics.Shader
import android.view.MotionEvent
import android.view.View

/** A color as hue (0–360), saturation and value (0–1) and alpha (0–1), as the panel edits it. */
internal data class Hsva(val hue: Float, val saturation: Float, val value: Float, val alpha: Float) {
    fun toColor(): Int = Color.HSVToColor((alpha * 255).toInt().coerceIn(0, 255), floatArrayOf(hue, saturation, value))

    companion object {
        fun of(color: Int): Hsva {
            val hsv = FloatArray(3)
            Color.colorToHSV(color, hsv)
            return Hsva(hsv[0], hsv[1], hsv[2], Color.alpha(color) / 255f)
        }
    }
}

/** Shared drawing of the color fields: rounded shape, white thumb with a dark edge. */
@SuppressLint("ViewConstructor") // Built in code only.
internal abstract class ColorField(context: Context) : View(context) {
    protected val area = RectF()
    protected val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val thumb = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.STROKE }
    protected val radius = Graphite.dp(context, 10f).toFloat()
    private val hsv = FloatArray(3)

    /** The opaque color of [hue], [saturation] and [value], without allocating while drawing. */
    protected fun hsvColor(hue: Float, saturation: Float, value: Float): Int {
        hsv[0] = hue
        hsv[1] = saturation
        hsv[2] = value
        return Color.HSVToColor(hsv)
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        super.onSizeChanged(w, h, oldw, oldh)
        val inset = Graphite.dp(context, THUMB_DP).toFloat()
        area.set(inset, inset, w - inset, h - inset)
        updateShader()
    }

    /** Rebuilds the field's shader for its size and current color. */
    protected abstract fun updateShader()

    /** The thumb at ([x], [y]): white ring with a dark edge, readable on any color. */
    protected fun drawThumb(canvas: Canvas, x: Float, y: Float, inner: Int) {
        val r = Graphite.dp(context, THUMB_DP).toFloat()
        fill.shader = null
        fill.color = inner
        canvas.drawCircle(x, y, r - Graphite.dp(context, 2f), fill)
        thumb.color = Color.WHITE
        thumb.strokeWidth = Graphite.dp(context, 3f).toFloat()
        canvas.drawCircle(x, y, r - Graphite.dp(context, 2f), thumb)
        thumb.color = THUMB_EDGE
        thumb.strokeWidth = Graphite.dp(context, 1f).toFloat()
        canvas.drawCircle(x, y, r - Graphite.dp(context, 0.5f), thumb)
    }

    @SuppressLint("ClickableViewAccessibility") // A continuous control; the hex field is the accessible way.
    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_MOVE -> {
                parent?.requestDisallowInterceptTouchEvent(true)
                val x = ((event.x - area.left) / area.width()).coerceIn(0f, 1f)
                val y = ((event.y - area.top) / area.height()).coerceIn(0f, 1f)
                onDrag(x, y)
                invalidate()
            }
        }
        return true
    }

    /** The touch at ([x], [y]), both from 0 to 1 across the field. */
    protected abstract fun onDrag(x: Float, y: Float)

    protected companion object {
        const val THUMB_DP = 11f
        val THUMB_EDGE = Color.argb(0x80, 0, 0, 0)
    }
}

/** Saturation left to right, value top to bottom, for one hue. */
@SuppressLint("ViewConstructor") // Built in code only.
internal class SaturationValueField(context: Context, private val onChange: (Float, Float) -> Unit) : ColorField(context) {
    var hue = 0f
        set(value) { field = value; updateShader(); invalidate() }
    var saturation = 1f
        set(value) { field = value; invalidate() }
    var value = 1f
        set(value) { field = value; invalidate() }

    override fun updateShader() {
        if (area.isEmpty) return
        val pure = hsvColor(hue, 1f, 1f)
        val across = LinearGradient(area.left, 0f, area.right, 0f, Color.WHITE, pure, Shader.TileMode.CLAMP)
        val down = LinearGradient(0f, area.top, 0f, area.bottom, Color.TRANSPARENT, Color.BLACK, Shader.TileMode.CLAMP)
        areaShader = ComposeShader(across, down, PorterDuff.Mode.SRC_OVER)
    }

    private var areaShader: Shader? = null

    override fun onDraw(canvas: Canvas) {
        fill.shader = areaShader
        canvas.drawRoundRect(area, radius, radius, fill)
        val x = area.left + saturation * area.width()
        val y = area.top + (1 - value) * area.height()
        drawThumb(canvas, x, y, hsvColor(hue, saturation, value))
    }

    override fun onDrag(x: Float, y: Float) {
        saturation = x
        value = 1 - y
        onChange(saturation, value)
    }
}

/** The hues, red to red. */
@SuppressLint("ViewConstructor") // Built in code only.
internal class HueBar(context: Context, private val onChange: (Float) -> Unit) : ColorField(context) {
    var hue = 0f
        set(value) { field = value; invalidate() }

    override fun updateShader() {
        if (area.isEmpty) return
        val stops = IntArray(7) { i -> Color.HSVToColor(floatArrayOf(i * 60f % 360f, 1f, 1f)) }.also { it[6] = Color.RED }
        barShader = LinearGradient(area.left, 0f, area.right, 0f, stops, null, Shader.TileMode.CLAMP)
    }

    private var barShader: Shader? = null

    override fun onDraw(canvas: Canvas) {
        fill.shader = barShader
        canvas.drawRoundRect(area, area.height() / 2, area.height() / 2, fill)
        drawThumb(canvas, area.left + hue / 360f * area.width(), area.centerY(), hsvColor(hue, 1f, 1f))
    }

    override fun onDrag(x: Float, y: Float) {
        hue = (x * 360f).coerceAtMost(359.9f)
        onChange(hue)
    }
}

/** The opacity of [color], over a checkerboard. */
@SuppressLint("ViewConstructor") // Built in code only.
internal class AlphaBar(context: Context, private val onChange: (Float) -> Unit) : ColorField(context) {
    var color = Color.BLACK
        set(value) { field = value; updateShader(); invalidate() }
    private var barShader: Shader? = null
    private val checker = Paint().apply {
        val cell = Graphite.dp(context, 5f)
        val bitmap = Bitmap.createBitmap(cell * 2, cell * 2, Bitmap.Config.ARGB_8888)
        Canvas(bitmap).apply {
            drawColor(CHECKER_LIGHT)
            val dark = Paint().apply { color = CHECKER_DARK }
            drawRect(0f, 0f, cell.toFloat(), cell.toFloat(), dark)
            drawRect(cell.toFloat(), cell.toFloat(), cell * 2f, cell * 2f, dark)
        }
        shader = BitmapShader(bitmap, Shader.TileMode.REPEAT, Shader.TileMode.REPEAT)
    }

    override fun updateShader() {
        if (area.isEmpty) return
        val opaque = color or (0xFF shl 24)
        barShader = LinearGradient(area.left, 0f, area.right, 0f, opaque and 0x00FFFFFF, opaque, Shader.TileMode.CLAMP)
    }

    override fun onDraw(canvas: Canvas) {
        val corner = area.height() / 2
        canvas.drawRoundRect(area, corner, corner, checker)
        fill.shader = barShader
        canvas.drawRoundRect(area, corner, corner, fill)
        drawThumb(canvas, area.left + Color.alpha(color) / 255f * area.width(), area.centerY(), color)
    }

    override fun onDrag(x: Float, y: Float) = onChange(x)

    private companion object {
        val CHECKER_LIGHT = Color.rgb(0x9A, 0x9C, 0xA3)
        val CHECKER_DARK = Color.rgb(0x5E, 0x61, 0x69)
    }
}
