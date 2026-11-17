package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF

/** The results of a search marked on the page: all of them lightly, the current one more. */
internal class MatchHighlights(context: Context) {
    /** Each result's rectangles in document coordinates: left, top, right, bottom. */
    var matches: List<List<DoubleArray>> = emptyList()

    /** The result shown, if any. */
    var current: Int? = null

    private val fill = Paint().apply { color = Color.argb(0x50, 0xFF, 0xD5, 0x4F) }
    private val currentFill = Paint().apply { color = Color.argb(0x80, 0xFF, 0xB3, 0x00) }
    private val outline = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeWidth = Graphite.dp(context, 2f).toFloat()
        color = Color.rgb(0xFF, 0xB3, 0x00)
    }
    private val rect = RectF()
    private val pad = Graphite.dp(context, 2f).toFloat()

    fun draw(canvas: Canvas, viewport: Viewport) {
        matches.forEachIndexed { index, rects ->
            val isCurrent = index == current
            for (r in rects) {
                rect.set(
                    ((r[0] - viewport.x) * viewport.zoom).toFloat() - pad,
                    ((r[1] - viewport.y) * viewport.zoom).toFloat() - pad,
                    ((r[2] - viewport.x) * viewport.zoom).toFloat() + pad,
                    ((r[3] - viewport.y) * viewport.zoom).toFloat() + pad,
                )
                canvas.drawRect(rect, if (isCurrent) currentFill else fill)
                if (isCurrent) canvas.drawRect(rect, outline)
            }
        }
    }
}
