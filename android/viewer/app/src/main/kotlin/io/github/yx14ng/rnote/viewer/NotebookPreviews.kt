package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.text.TextPaint
import android.text.TextUtils
import android.view.View

/** A sheet of paper with its template, as a new notebook will look. */
@SuppressLint("ViewConstructor") // Built in code only.
internal class PaperPreview(context: Context) : View(context) {
    var paper: Int = Color.WHITE
        set(value) { field = value; invalidate() }
    var line: Int = Color.GRAY
        set(value) { field = value; invalidate() }
    var template: Template = Template.GRID
        set(value) { field = value; invalidate() }
    /** The spacing of the template in millimeters, drawn at [PX_PER_MM] dp. */
    var spacingMm: Double = 5.0
        set(value) { field = value; invalidate() }

    private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply { strokeWidth = Graphite.dp(context, 1f).toFloat() }
    private val radius = Graphite.dp(context, 4f).toFloat()
    private val clip = Path()
    private val bounds = RectF()

    override fun onDraw(canvas: Canvas) {
        bounds.set(0f, 0f, width.toFloat(), height.toFloat())
        clip.reset()
        clip.addRoundRect(bounds, radius, radius, Path.Direction.CW)
        canvas.save()
        canvas.clipPath(clip)
        fill.color = paper
        canvas.drawRect(bounds, fill)
        stroke.color = line
        val step = (spacingMm * PX_PER_MM * resources.displayMetrics.density).toFloat().coerceAtLeast(4f)
        when (template) {
            Template.BLANK -> Unit
            Template.LINES -> forEachStep(height, step) { y -> canvas.drawLine(0f, y, bounds.right, y, stroke) }
            Template.GRID -> {
                forEachStep(height, step) { y -> canvas.drawLine(0f, y, bounds.right, y, stroke) }
                forEachStep(width, step) { x -> canvas.drawLine(x, 0f, x, bounds.bottom, stroke) }
            }
            Template.DOTS -> {
                fill.color = line
                val dot = Graphite.dp(context, 1.3f).toFloat()
                forEachStep(height, step) { y -> forEachStep(width, step) { x -> canvas.drawCircle(x, y, dot, fill) } }
            }
        }
        canvas.restore()
    }

    private inline fun forEachStep(size: Int, step: Float, draw: (Float) -> Unit) {
        var at = step
        while (at < size) {
            draw(at)
            at += step
        }
    }

    private companion object {
        /** Preview scale: a millimeter of template spacing is this many dp. */
        const val PX_PER_MM = 1.6
    }
}

/** A notebook cover: its color, a darker spine and its name on a light label. */
@SuppressLint("ViewConstructor") // Built in code only.
internal class CoverView(context: Context) : View(context) {
    var color: Int = CoverColor.BLUE.argb
        set(value) { field = value; invalidate() }
    var name: String = ""
        set(value) { field = value; contentDescription = value; invalidate() }

    private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val text = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Graphite.SURFACE
        isFakeBoldText = true
    }
    private val path = Path()
    private val rect = RectF()
    private val small = Graphite.dp(context, 6f).toFloat()
    private val large = Graphite.dp(context, 12f).toFloat()
    // Bound side on the left: smaller corners there, like a real notebook.
    private val corners = floatArrayOf(small, small, large, large, large, large, small, small)

    override fun onDraw(canvas: Canvas) {
        val w = width.toFloat()
        val h = height.toFloat()
        path.reset()
        rect.set(0f, 0f, w, h)
        path.addRoundRect(rect, corners, Path.Direction.CW)
        canvas.save()
        canvas.clipPath(path)
        fill.color = color
        canvas.drawRect(0f, 0f, w, h, fill)
        fill.color = SPINE
        canvas.drawRect(0f, 0f, w * SPINE_WIDTH, h, fill)
        val left = w * LABEL_LEFT
        val right = w * (1 - LABEL_RIGHT)
        val top = h * LABEL_TOP
        val labelHeight = h * LABEL_HEIGHT
        fill.color = LABEL
        rect.set(left, top, right, top + labelHeight)
        canvas.drawRoundRect(rect, small, small, fill)
        text.textSize = labelHeight * TEXT_SCALE
        val padding = Graphite.dp(context, 8f).toFloat()
        val shown = TextUtils.ellipsize(name, text, right - left - 2 * padding, TextUtils.TruncateAt.END)
        val baseline = top + labelHeight / 2 - (text.descent() + text.ascent()) / 2
        canvas.drawText(shown, 0, shown.length, left + padding, baseline, text)
        canvas.restore()
    }

    private companion object {
        val SPINE = Color.argb(0x2E, 0, 0, 0)
        val LABEL = Color.argb(0xDB, 0xFF, 0xFF, 0xFF)
        const val SPINE_WIDTH = 0.08f
        const val LABEL_LEFT = 0.16f
        const val LABEL_RIGHT = 0.10f
        const val LABEL_TOP = 0.39f
        const val LABEL_HEIGHT = 0.23f
        const val TEXT_SCALE = 0.36f
    }
}
