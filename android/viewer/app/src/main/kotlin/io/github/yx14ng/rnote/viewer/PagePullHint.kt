package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import android.text.TextPaint

/** The pill near the bottom of the canvas while pulling to add a page, with its progress. */
internal class PagePullHint(context: Context) {
    private val label = context.getString(R.string.pull_to_add_page)
    private val pad = Graphite.dp(context, 18f).toFloat()
    private val bottomGap = Graphite.dp(context, 28f).toFloat()
    private val barHeight = Graphite.dp(context, 4f).toFloat()
    private val barGap = Graphite.dp(context, 10f).toFloat()
    private val text = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Graphite.TEXT
        textSize = Graphite.dp(context, 15f).toFloat()
    }
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG)
    private val pill = RectF()
    private val bar = RectF()

    /** Draws the pill at [progress] (0 to 1) on a canvas [width] by [height]. */
    fun draw(canvas: Canvas, width: Int, height: Int, progress: Float) {
        val textWidth = text.measureText(label)
        val pillWidth = textWidth + 2 * pad
        val pillHeight = pad + (text.descent() - text.ascent()) + barGap + barHeight + pad
        val left = (width - pillWidth) / 2
        val top = height - bottomGap - pillHeight
        pill.set(left, top, left + pillWidth, top + pillHeight)
        fill.color = Graphite.RAISED
        canvas.drawRoundRect(pill, pillHeight / 2, pillHeight / 2, fill)
        val baseline = top + pad - text.ascent()
        canvas.drawText(label, left + pad, baseline, text)
        val barTop = baseline + text.descent() + barGap
        bar.set(left + pad, barTop, left + pad + textWidth, barTop + barHeight)
        fill.color = Graphite.OUTLINE
        canvas.drawRoundRect(bar, barHeight / 2, barHeight / 2, fill)
        bar.right = bar.left + textWidth * progress
        fill.color = Graphite.ACCENT
        canvas.drawRoundRect(bar, barHeight / 2, barHeight / 2, fill)
    }
}
