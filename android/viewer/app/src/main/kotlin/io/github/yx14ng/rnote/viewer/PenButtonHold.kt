package io.github.yx14ng.rnote.viewer

import android.util.Log
import android.view.MotionEvent

/**
 * Recognizes the stylus button held for a while with the pen in the air, then released
 * without touching the screen: the gesture that opens the pen wheel. [onRelease] gets where
 * the pen hovered last, in view coordinates.
 *
 * Touching the screen with the button held erases instead, so it cancels the gesture.
 */
internal class PenButtonHold(private val onRelease: (x: Float, y: Float) -> Unit) {
    private var pressedAt = NOT_PRESSED
    private var touched = false
    private var lastX = 0f
    private var lastY = 0f

    /** Follows the pen hovering, whose events tell whether the button is held. */
    fun onHover(event: MotionEvent) {
        if (!isStylus(event)) {
            return
        }
        lastX = event.x
        lastY = event.y
        update(event.buttonState and MotionEvent.BUTTON_STYLUS_PRIMARY != 0, event.eventTime)
    }

    /** Follows `ACTION_BUTTON_PRESS` and `ACTION_BUTTON_RELEASE`, if the device sends them. */
    fun onButtonEvent(event: MotionEvent) {
        if (isStylus(event)) {
            update(event.buttonState and MotionEvent.BUTTON_STYLUS_PRIMARY != 0, event.eventTime)
        }
    }

    /** The pen touched the screen: whatever the button does now, it's not the wheel. */
    fun onTouch() {
        if (pressedAt != NOT_PRESSED) {
            touched = true
        }
    }

    private fun update(pressed: Boolean, time: Long) {
        if (pressed && pressedAt == NOT_PRESSED) {
            Log.d(TAG, "Stylus button pressed in the air")
            pressedAt = time
            touched = false
        } else if (!pressed && pressedAt != NOT_PRESSED) {
            val held = time - pressedAt
            Log.d(TAG, "Stylus button released after $held ms, touched: $touched")
            pressedAt = NOT_PRESSED
            if (!touched && held >= HOLD_MS) {
                onRelease(lastX, lastY)
            }
        }
    }

    private fun isStylus(event: MotionEvent): Boolean {
        val tool = event.getToolType(0)
        return tool == MotionEvent.TOOL_TYPE_STYLUS || tool == MotionEvent.TOOL_TYPE_ERASER
    }

    private companion object {
        const val TAG = "RnoteViewer"
        const val NOT_PRESSED = -1L

        /** How long the button must be held. */
        const val HOLD_MS = 800L
    }
}
