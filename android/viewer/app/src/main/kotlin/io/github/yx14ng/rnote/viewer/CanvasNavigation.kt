package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.view.GestureDetector
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.ScaleGestureDetector

/**
 * Moving around the document with fingers and the mouse wheel: pinch and Ctrl+wheel zoom,
 * drag and wheel pan, fling, and double tap to zoom in or back out.
 *
 * [onMoved] runs after every move; [onFlingStarted] asks for animation frames, which
 * advance the fling; [onTap] gets single taps.
 */
internal class CanvasNavigation(
    context: Context,
    private val camera: ViewportController,
    private val onMoved: () -> Unit,
    private val onFlingStarted: () -> Unit,
    private val onTap: () -> Boolean,
) {
    private val scaleDetector = ScaleGestureDetector(
        context,
        object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
            override fun onScale(detector: ScaleGestureDetector): Boolean {
                camera.zoomAround(camera.viewport.zoom * detector.scaleFactor, detector.focusX, detector.focusY)
                onMoved()
                return true
            }
        },
    )

    private val gestureDetector = GestureDetector(
        context,
        object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(e: MotionEvent): Boolean {
                camera.stopFling()
                return true
            }

            override fun onScroll(e1: MotionEvent?, e2: MotionEvent, distanceX: Float, distanceY: Float): Boolean {
                camera.panBy(distanceX, distanceY)
                onMoved()
                return true
            }

            override fun onFling(e1: MotionEvent?, e2: MotionEvent, velocityX: Float, velocityY: Float): Boolean {
                camera.fling(velocityX, velocityY)
                onFlingStarted()
                return true
            }

            override fun onSingleTapConfirmed(e: MotionEvent): Boolean = onTap()

            override fun onDoubleTap(e: MotionEvent): Boolean {
                camera.toggleZoom(e.x, e.y)
                onMoved()
                return true
            }
        },
    )

    /** Feeds a touch event meant for navigation to the gesture detectors. */
    fun onTouch(event: MotionEvent) {
        scaleDetector.onTouchEvent(event)
        gestureDetector.onTouchEvent(event)
    }

    /** Pans with the mouse wheel, or zooms with Ctrl held. Returns whether [event] was that. */
    fun onWheel(event: MotionEvent): Boolean {
        if (!event.isFromSource(InputDevice.SOURCE_CLASS_POINTER) || event.actionMasked != MotionEvent.ACTION_SCROLL) {
            return false
        }
        val vertical = event.getAxisValue(MotionEvent.AXIS_VSCROLL)
        val horizontal = event.getAxisValue(MotionEvent.AXIS_HSCROLL)
        if (event.metaState and KeyEvent.META_CTRL_ON != 0) {
            camera.zoomAround(camera.viewport.zoom * (1 + WHEEL_ZOOM_STEP * vertical), event.x, event.y)
        } else {
            camera.panBy(-horizontal * WHEEL_PAN_STEP_PX, -vertical * WHEEL_PAN_STEP_PX)
        }
        onMoved()
        return true
    }

    private companion object {
        const val WHEEL_PAN_STEP_PX = 120f
        const val WHEEL_ZOOM_STEP = 0.1
    }
}
