package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.view.MotionEvent
import android.view.ViewConfiguration
import kotlin.math.hypot

/**
 * Recognizes a quick tap with two fingers, the undo gesture of other note-taking apps: both
 * fingers land close together in time, barely move, and lift soon after.
 *
 * Only watches the events; the view still passes them on, so a slower two-finger gesture
 * pans and zooms as usual.
 */
internal class TwoFingerTap(context: Context, private val onTap: () -> Unit) {
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop.toFloat()

    private var candidate = false
    private var sawSecondFinger = false
    private var downTime = 0L
    private val pointerIds = IntArray(MAX_POINTERS)
    private val startX = FloatArray(MAX_POINTERS)
    private val startY = FloatArray(MAX_POINTERS)

    fun onTouch(event: MotionEvent) {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                candidate = isFinger(event, 0)
                sawSecondFinger = false
                downTime = event.eventTime
                remember(event, 0, slot = 0)
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                val index = event.actionIndex
                val inTime = event.eventTime - downTime <= MAX_LANDING_GAP_MS
                candidate = candidate && !sawSecondFinger && event.pointerCount == MAX_POINTERS &&
                    isFinger(event, index) && inTime
                if (candidate) {
                    sawSecondFinger = true
                    remember(event, index, slot = 1)
                }
            }
            MotionEvent.ACTION_MOVE -> if (candidate && moved(event)) candidate = false
            MotionEvent.ACTION_UP -> {
                val quick = event.eventTime - downTime <= MAX_TAP_DURATION_MS
                if (candidate && sawSecondFinger && quick && !moved(event)) {
                    onTap()
                }
                candidate = false
            }
            MotionEvent.ACTION_CANCEL -> candidate = false
        }
    }

    private fun isFinger(event: MotionEvent, index: Int) = event.getToolType(index) == MotionEvent.TOOL_TYPE_FINGER

    private fun remember(event: MotionEvent, index: Int, slot: Int) {
        pointerIds[slot] = event.getPointerId(index)
        startX[slot] = event.getX(index)
        startY[slot] = event.getY(index)
    }

    /** Whether a remembered finger still down moved farther than a tap allows. */
    private fun moved(event: MotionEvent): Boolean {
        val fingers = if (sawSecondFinger) MAX_POINTERS else 1
        return (0 until fingers).any { slot ->
            val index = event.findPointerIndex(pointerIds[slot])
            index >= 0 && hypot(event.getX(index) - startX[slot], event.getY(index) - startY[slot]) > touchSlop
        }
    }

    private companion object {
        const val MAX_POINTERS = 2
        const val MAX_LANDING_GAP_MS = 150L
        const val MAX_TAP_DURATION_MS = 300L
    }
}
