package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.MotionEvent
import android.view.ViewConfiguration
import kotlin.math.hypot

/**
 * Turns touch, stylus and mouse events into pen batches or navigation.
 *
 * A stylus, its eraser end or a mouse uses [tool]; other pointers are ignored while it is
 * down, so a resting palm neither draws nor moves the page, even when it touched first. With
 * [fingerDrawing], one finger draws too: it starts a stroke once it moved without a second
 * finger landing, which turns the gesture into navigation instead.
 *
 * Strokes follow the pointer that started them by id, since pointer indices shift as other
 * pointers come and go.
 *
 * With [holdToSnap], holding the pen still for a moment during a brush stroke calls [onHold],
 * which may turn the stroke into a shape; [onShapeSnapped] then stops its preview.
 *
 * [sendPen] receives `x, y, pressure` triples in document coordinates; [navigate] gets the
 * events meant for the pan and zoom detectors.
 */
internal class CanvasInput(
    context: Context,
    private val camera: ViewportController,
    private val navigate: (MotionEvent) -> Unit,
    private val sendPen: (PenPhase, Tool, DoubleArray) -> Unit,
    private val onPenLifted: () -> Unit,
    private val onHold: () -> Unit,
) {
    var tool = Tool.BRUSH
    var brush = Brush(Color.BLACK, BrushPalette.DEFAULT_WIDTH)
    var fingerDrawing = false
    var holdToSnap = false

    /** Immediate feedback for brush strokes, drawn by the view on top of the last frame. */
    val preview = StrokePreview()

    /** Whether a stroke is in progress. */
    val penDown: Boolean
        get() = strokePointerId != NO_POINTER

    /** Where the pen last touched down, in document coordinates. */
    var lastPenX = 0.0
        private set
    var lastPenY = 0.0
        private set

    private var strokePointerId = NO_POINTER
    private var penTool = Tool.BRUSH
    private var fingerGesture = FingerGesture.NONE
    private var fingerPointerId = NO_POINTER

    // Set when a stroke ended while other pointers (a palm) stay down: they do nothing until
    // every pointer lifts, so the resting hand doesn't start panning.
    private var ignoringRest = false
    private var fingerDownSamples = DoubleArray(0)
    private var fingerDownX = 0f
    private var fingerDownY = 0f
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop

    // Holding still: the pen must stay within holdSlopPx of the anchor for HOLD_DELAY_MS.
    private val holdHandler = Handler(Looper.getMainLooper())
    private val holdSlopPx = HOLD_SLOP_DP * context.resources.displayMetrics.density
    private var holdAnchorX = 0f
    private var holdAnchorY = 0f
    private val holdCheck = Runnable {
        if (penDown && penTool == Tool.BRUSH && holdToSnap) {
            onHold()
        }
    }

    // Whether the brush preview follows the stroke; off once it became a shape.
    private var previewing = true

    /** Forgets any interaction in progress, e.g. when another document is shown. */
    fun reset() {
        strokePointerId = NO_POINTER
        fingerGesture = FingerGesture.NONE
        fingerPointerId = NO_POINTER
        ignoringRest = false
        holdHandler.removeCallbacks(holdCheck)
        preview.clear()
    }

    /** The stroke being drawn became a shape: stop previewing the freehand line. */
    fun onShapeSnapped() {
        previewing = false
        preview.clear()
        holdHandler.removeCallbacks(holdCheck)
    }

    fun onTouch(event: MotionEvent) {
        val action = event.actionMasked
        val actionIndex = event.actionIndex
        val startsPointer = action == MotionEvent.ACTION_DOWN || action == MotionEvent.ACTION_POINTER_DOWN
        when {
            penDown && fingerGesture != FingerGesture.DRAWING -> followStylus(event)
            startsPointer && isPenTool(event.getToolType(actionIndex)) -> {
                // A stylus landing ends whatever the fingers were doing: they were the palm.
                if (fingerGesture != FingerGesture.NONE) {
                    cancelFingers(event)
                } else if (event.pointerCount > 1) {
                    cancelNavigation(event)
                }
                ignoringRest = false
                camera.stopFling()
                beginPen(toolFor(event, actionIndex), event.getPointerId(actionIndex), samplesOf(event, actionIndex, false))
                trackHold(event.getX(actionIndex), event.getY(actionIndex), restart = true)
            }
            ignoringRest -> if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
                ignoringRest = false
            }
            fingerGesture != FingerGesture.NONE -> handleFingerEvent(event)
            action == MotionEvent.ACTION_DOWN && fingerDrawing &&
                event.getToolType(actionIndex) == MotionEvent.TOOL_TYPE_FINGER -> handleFingerEvent(event)
            else -> navigate(event)
        }
    }

    /**
     * Hovering moves the next segment of the shape being built. Other tools don't use it, so
     * they skip the work. Returns whether the event was used.
     */
    fun onHover(event: MotionEvent): Boolean {
        if (penDown || tool != Tool.SHAPER || !isPenTool(event.getToolType(0)) ||
            event.actionMasked != MotionEvent.ACTION_HOVER_MOVE
        ) {
            return false
        }
        penTool = tool
        sendPen(PenPhase.PROXIMITY, penTool, samplesOf(event, 0, withHistory = true))
        return true
    }

    private fun isPenTool(toolType: Int): Boolean =
        when (toolType) {
            MotionEvent.TOOL_TYPE_STYLUS,
            MotionEvent.TOOL_TYPE_ERASER,
            MotionEvent.TOOL_TYPE_MOUSE,
            -> true
            else -> false
        }

    /** Continues or ends the stylus stroke; other pointers are ignored meanwhile. */
    private fun followStylus(event: MotionEvent) {
        val index = event.findPointerIndex(strokePointerId)
        when (event.actionMasked) {
            MotionEvent.ACTION_MOVE -> if (index >= 0) {
                continuePen(samplesOf(event, index, true))
                trackHold(event.getX(index), event.getY(index), restart = false)
            }
            MotionEvent.ACTION_POINTER_UP, MotionEvent.ACTION_UP ->
                if (event.getPointerId(event.actionIndex) == strokePointerId) {
                    endPen(samplesOf(event, event.actionIndex, true))
                    ignoringRest = event.actionMasked == MotionEvent.ACTION_POINTER_UP
                } else if (event.actionMasked == MotionEvent.ACTION_UP) {
                    // The stroke's pointer vanished without its own up: don't keep drawing.
                    cancelPen()
                }
            MotionEvent.ACTION_CANCEL -> cancelPen()
        }
    }

    private fun handleFingerEvent(event: MotionEvent) {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                camera.stopFling()
                fingerGesture = FingerGesture.UNDECIDED
                fingerPointerId = event.getPointerId(0)
                fingerDownX = event.x
                fingerDownY = event.y
                fingerDownSamples = samplesOf(event, 0, withHistory = false)
                navigate(event)
            }
            MotionEvent.ACTION_POINTER_DOWN -> when (fingerGesture) {
                FingerGesture.UNDECIDED, FingerGesture.NAVIGATING -> {
                    fingerGesture = FingerGesture.NAVIGATING
                    navigate(event)
                }
                // Extra fingers while drawing are a resting hand.
                FingerGesture.DRAWING, FingerGesture.NONE -> {}
            }
            MotionEvent.ACTION_MOVE -> when (fingerGesture) {
                FingerGesture.UNDECIDED -> {
                    val index = event.findPointerIndex(fingerPointerId)
                    val moved = index >= 0 &&
                        hypot(event.getX(index) - fingerDownX, event.getY(index) - fingerDownY) > touchSlop
                    if (moved) {
                        fingerGesture = FingerGesture.DRAWING
                        cancelNavigation(event)
                        beginPen(tool, fingerPointerId, fingerDownSamples)
                        continuePen(samplesOf(event, index, withHistory = true))
                        trackHold(event.getX(index), event.getY(index), restart = true)
                    } else {
                        navigate(event)
                    }
                }
                FingerGesture.DRAWING -> {
                    val index = event.findPointerIndex(fingerPointerId)
                    if (index >= 0) {
                        continuePen(samplesOf(event, index, withHistory = true))
                        trackHold(event.getX(index), event.getY(index), restart = false)
                    }
                }
                FingerGesture.NAVIGATING -> navigate(event)
                FingerGesture.NONE -> {}
            }
            MotionEvent.ACTION_POINTER_UP -> when (fingerGesture) {
                FingerGesture.NAVIGATING -> navigate(event)
                FingerGesture.DRAWING ->
                    // The drawing finger lifted while another rests: the stroke ends here.
                    if (event.getPointerId(event.actionIndex) == fingerPointerId) {
                        endPen(samplesOf(event, event.actionIndex, withHistory = true))
                        fingerGesture = FingerGesture.NONE
                        fingerPointerId = NO_POINTER
                        ignoringRest = true
                    }
                FingerGesture.UNDECIDED, FingerGesture.NONE -> {}
            }
            MotionEvent.ACTION_UP -> {
                when (fingerGesture) {
                    FingerGesture.DRAWING -> endPen(samplesOf(event, event.actionIndex, withHistory = true))
                    else -> navigate(event)
                }
                fingerGesture = FingerGesture.NONE
                fingerPointerId = NO_POINTER
            }
            MotionEvent.ACTION_CANCEL -> {
                if (fingerGesture == FingerGesture.DRAWING) cancelPen() else navigate(event)
                fingerGesture = FingerGesture.NONE
                fingerPointerId = NO_POINTER
            }
        }
    }

    /** Ends a finger gesture because a stylus took over. */
    private fun cancelFingers(event: MotionEvent) {
        if (fingerGesture == FingerGesture.DRAWING) {
            cancelPen()
        } else {
            cancelNavigation(event)
        }
        fingerGesture = FingerGesture.NONE
        fingerPointerId = NO_POINTER
    }

    /** Resets the gesture detectors, which saw the start of what turned into something else. */
    private fun cancelNavigation(event: MotionEvent) {
        val cancel = MotionEvent.obtain(event)
        cancel.action = MotionEvent.ACTION_CANCEL
        navigate(cancel)
        cancel.recycle()
    }

    /** The stylus eraser end and its primary button erase; otherwise the selected tool. */
    private fun toolFor(event: MotionEvent, index: Int): Tool {
        val eraserEnd = event.getToolType(index) == MotionEvent.TOOL_TYPE_ERASER
        val eraserButton = event.buttonState and MotionEvent.BUTTON_STYLUS_PRIMARY != 0
        return if (eraserEnd || eraserButton) Tool.ERASER else tool
    }

    private fun beginPen(tool: Tool, pointerId: Int, samples: DoubleArray) {
        if (samples.size >= 2) {
            lastPenX = samples[0]
            lastPenY = samples[1]
        }
        strokePointerId = pointerId
        penTool = tool
        previewing = true
        if (tool == Tool.BRUSH) preview.start(brush) else preview.clear()
        send(PenPhase.DOWN, samples)
    }

    private fun continuePen(samples: DoubleArray) = send(PenPhase.DOWN, samples)

    private fun endPen(samples: DoubleArray) {
        strokePointerId = NO_POINTER
        holdHandler.removeCallbacks(holdCheck)
        send(PenPhase.UP, samples)
        onPenLifted()
    }

    private fun cancelPen() {
        strokePointerId = NO_POINTER
        holdHandler.removeCallbacks(holdCheck)
        send(PenPhase.CANCEL, DoubleArray(0))
        onPenLifted()
    }

    private fun send(phase: PenPhase, samples: DoubleArray) {
        if (penTool == Tool.BRUSH && previewing) {
            for (index in samples.indices step 3) {
                preview.add(samples[index], samples[index + 1], samples[index + 2])
            }
        }
        sendPen(phase, penTool, samples)
    }

    /**
     * Restarts the hold countdown when the pen moved away from where it last rested, at
     * screen position ([x], [y]). Only brush strokes are recognized.
     */
    private fun trackHold(x: Float, y: Float, restart: Boolean) {
        if (!holdToSnap || penTool != Tool.BRUSH || !previewing) {
            return
        }
        if (restart || hypot(x - holdAnchorX, y - holdAnchorY) > holdSlopPx) {
            holdAnchorX = x
            holdAnchorY = y
            holdHandler.removeCallbacks(holdCheck)
            holdHandler.postDelayed(holdCheck, HOLD_DELAY_MS)
        }
    }

    /** `x, y, pressure` triples of the pointer at [index], in document coordinates. */
    private fun samplesOf(event: MotionEvent, index: Int, withHistory: Boolean): DoubleArray {
        val history = if (withHistory) event.historySize else 0
        val samples = DoubleArray((history + 1) * 3)
        // Mice and fingers report no meaningful pressure; use the middle, like the desktop app.
        val toolType = event.getToolType(index)
        val hasPressure = toolType == MotionEvent.TOOL_TYPE_STYLUS || toolType == MotionEvent.TOOL_TYPE_ERASER
        for (step in 0..history) {
            val x: Float
            val y: Float
            val rawPressure: Float
            if (step < history) {
                x = event.getHistoricalX(index, step)
                y = event.getHistoricalY(index, step)
                rawPressure = event.getHistoricalPressure(index, step)
            } else {
                x = event.getX(index)
                y = event.getY(index)
                rawPressure = event.getPressure(index)
            }
            samples[step * 3] = camera.toDocumentX(x)
            samples[step * 3 + 1] = camera.toDocumentY(y)
            samples[step * 3 + 2] = if (hasPressure) rawPressure.toDouble().coerceIn(0.0, 1.0) else DEFAULT_PRESSURE
        }
        return samples
    }

    private enum class FingerGesture {
        NONE,
        UNDECIDED,
        DRAWING,
        NAVIGATING,
    }

    private companion object {
        const val DEFAULT_PRESSURE = 0.5
        const val NO_POINTER = -1

        /** How long the pen must hold still for the stroke to become a shape. */
        const val HOLD_DELAY_MS = 1000L

        /** How far the resting pen may wander and still count as holding still. */
        const val HOLD_SLOP_DP = 6f
    }
}
