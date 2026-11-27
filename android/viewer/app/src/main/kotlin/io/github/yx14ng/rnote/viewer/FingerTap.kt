package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.view.MotionEvent
import android.view.ViewConfiguration
import kotlin.math.hypot

/**
 * Recognizes a quick tap with [fingers] fingers, like other note-taking apps' undo (two) and
 * redo (three): the fingers land close together in time, barely move, and lift soon after.
 * A tap with more fingers isn't one with fewer.
 *
 * Only watches the events; the view still passes them on, so a slower gesture pans and zooms
 * as usual.
 */
internal class FingerTap(
    context: Context,
    private val fingers: Int,
    private val fingersAllowed: (Long) -> Boolean,
    private val onTap: () -> Unit,
) {
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop.toFloat()

    private var candidate = false
    private var landed = 0
    private var downTime = 0L
    private val pointerIds = IntArray(fingers)
    private val startX = FloatArray(fingers)
    private val startY = FloatArray(fingers)

    fun onTouch(event: MotionEvent) {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                // Not the palm resting while the pen is near.
                candidate = isFinger(event, 0) && fingersAllowed(event.eventTime)
                landed = 0
                downTime = event.eventTime
                remember(event, 0)
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                val index = event.actionIndex
                val inTime = event.eventTime - downTime <= MAX_LANDING_GAP_MS
                candidate = candidate && landed < fingers && event.pointerCount == landed + 1 &&
                    isFinger(event, index) && inTime
                if (candidate) remember(event, index)
            }
            MotionEvent.ACTION_MOVE -> if (candidate && moved(event)) candidate = false
            // Android found out a pointer was the palm, after the fact.
            MotionEvent.ACTION_POINTER_UP -> if (canceled(event)) candidate = false
            MotionEvent.ACTION_UP -> {
                if (canceled(event)) candidate = false
                val quick = event.eventTime - downTime <= MAX_TAP_DURATION_MS
                if (candidate && landed == fingers && quick && !moved(event)) {
                    onTap()
                }
                candidate = false
            }
            MotionEvent.ACTION_CANCEL -> candidate = false
        }
    }

    @SuppressLint("InlinedApi")
    private fun canceled(event: MotionEvent) = event.flags and MotionEvent.FLAG_CANCELED != 0

    private fun isFinger(event: MotionEvent, index: Int) = event.getToolType(index) == MotionEvent.TOOL_TYPE_FINGER

    private fun remember(event: MotionEvent, index: Int) {
        pointerIds[landed] = event.getPointerId(index)
        startX[landed] = event.getX(index)
        startY[landed] = event.getY(index)
        landed += 1
    }

    /** Whether a remembered finger still down moved farther than a tap allows. */
    private fun moved(event: MotionEvent): Boolean = (0 until landed).any { slot ->
        val index = event.findPointerIndex(pointerIds[slot])
        index >= 0 && hypot(event.getX(index) - startX[slot], event.getY(index) - startY[slot]) > touchSlop
    }

    private companion object {
        const val MAX_LANDING_GAP_MS = 150L
        const val MAX_TAP_DURATION_MS = 300L
    }
}
