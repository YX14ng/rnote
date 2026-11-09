package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.graphics.Color
import android.view.MotionEvent
import android.view.ViewConfiguration
import kotlin.math.hypot

/**
 * Turns touch, stylus and mouse events into pen batches or navigation.
 *
 * A stylus, its eraser end or a mouse uses [tool]; fingers navigate and are ignored while the
 * pen is down. With [fingerDrawing], one finger draws too: it starts a stroke once it moved
 * without a second finger landing, which turns the gesture into navigation instead.
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
) {
    var tool = Tool.BRUSH
    var brush = Brush(Color.BLACK, BrushPalette.DEFAULT_WIDTH)
    var fingerDrawing = false

    /** Immediate feedback for brush strokes, drawn by the view on top of the last frame. */
    val preview = StrokePreview()

    var penDown = false
        private set

    /** Where the pen last touched down, in document coordinates. */
    var lastPenX = 0.0
        private set
    var lastPenY = 0.0
        private set

    private var penTool = Tool.BRUSH
    private var fingerGesture = FingerGesture.NONE
    private var fingerDownSamples = DoubleArray(0)
    private var fingerDownX = 0f
    private var fingerDownY = 0f
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop

    /** Forgets any interaction in progress, e.g. when another document is shown. */
    fun reset() {
        penDown = false
        fingerGesture = FingerGesture.NONE
        preview.clear()
    }

    fun onTouch(event: MotionEvent) {
        when {
            isPenEvent(event) -> handlePenEvent(event)
            // Palm rejection: ignore fingers while the pen is on the surface.
            penDown && fingerGesture != FingerGesture.DRAWING -> {}
            fingerDrawing && event.getToolType(0) == MotionEvent.TOOL_TYPE_FINGER -> handleFingerEvent(event)
            else -> navigate(event)
        }
    }

    /**
     * Hovering moves the next segment of the shape being built. Other tools don't use it, so
     * they skip the work. Returns whether the event was used.
     */
    fun onHover(event: MotionEvent): Boolean {
        if (penDown || tool != Tool.SHAPER || !isPenEvent(event) ||
            event.actionMasked != MotionEvent.ACTION_HOVER_MOVE
        ) {
            return false
        }
        penTool = tool
        sendPen(PenPhase.PROXIMITY, penTool, samplesOf(event, withHistory = true))
        return true
    }

    private fun isPenEvent(event: MotionEvent): Boolean =
        when (event.getToolType(0)) {
            MotionEvent.TOOL_TYPE_STYLUS,
            MotionEvent.TOOL_TYPE_ERASER,
            MotionEvent.TOOL_TYPE_MOUSE,
            -> true
            else -> false
        }

    private fun handlePenEvent(event: MotionEvent) {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                camera.stopFling()
                beginPen(toolFor(event), samplesOf(event, withHistory = false))
            }
            MotionEvent.ACTION_MOVE -> if (penDown) continuePen(samplesOf(event, withHistory = true))
            MotionEvent.ACTION_UP -> if (penDown) endPen(samplesOf(event, withHistory = true))
            MotionEvent.ACTION_CANCEL -> if (penDown) cancelPen()
        }
    }

    private fun handleFingerEvent(event: MotionEvent) {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                camera.stopFling()
                fingerGesture = FingerGesture.UNDECIDED
                fingerDownX = event.x
                fingerDownY = event.y
                fingerDownSamples = samplesOf(event, withHistory = false)
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
                    if (hypot(event.x - fingerDownX, event.y - fingerDownY) > touchSlop) {
                        fingerGesture = FingerGesture.DRAWING
                        cancelNavigation(event)
                        beginPen(toolFor(event), fingerDownSamples)
                        continuePen(samplesOf(event, withHistory = true))
                    } else {
                        navigate(event)
                    }
                }
                FingerGesture.DRAWING -> continuePen(samplesOf(event, withHistory = true))
                FingerGesture.NAVIGATING -> navigate(event)
                FingerGesture.NONE -> {}
            }
            MotionEvent.ACTION_POINTER_UP -> if (fingerGesture == FingerGesture.NAVIGATING) navigate(event)
            MotionEvent.ACTION_UP -> {
                if (fingerGesture == FingerGesture.DRAWING) {
                    endPen(samplesOf(event, withHistory = true))
                } else {
                    navigate(event)
                }
                fingerGesture = FingerGesture.NONE
            }
            MotionEvent.ACTION_CANCEL -> {
                if (fingerGesture == FingerGesture.DRAWING) cancelPen() else navigate(event)
                fingerGesture = FingerGesture.NONE
            }
        }
    }

    /** Resets the gesture detectors, which saw the start of what turned into a stroke. */
    private fun cancelNavigation(event: MotionEvent) {
        val cancel = MotionEvent.obtain(event)
        cancel.action = MotionEvent.ACTION_CANCEL
        navigate(cancel)
        cancel.recycle()
    }

    /** The stylus eraser end and its primary button erase; otherwise the selected tool. */
    private fun toolFor(event: MotionEvent): Tool {
        val eraserEnd = event.getToolType(0) == MotionEvent.TOOL_TYPE_ERASER
        val eraserButton = event.buttonState and MotionEvent.BUTTON_STYLUS_PRIMARY != 0
        return if (eraserEnd || eraserButton) Tool.ERASER else tool
    }

    private fun beginPen(tool: Tool, samples: DoubleArray) {
        if (samples.size >= 2) {
            lastPenX = samples[0]
            lastPenY = samples[1]
        }
        penDown = true
        penTool = tool
        if (tool == Tool.BRUSH) preview.start(brush) else preview.clear()
        send(PenPhase.DOWN, samples)
    }

    private fun continuePen(samples: DoubleArray) = send(PenPhase.DOWN, samples)

    private fun endPen(samples: DoubleArray) {
        penDown = false
        send(PenPhase.UP, samples)
        onPenLifted()
    }

    private fun cancelPen() {
        penDown = false
        send(PenPhase.CANCEL, DoubleArray(0))
        onPenLifted()
    }

    private fun send(phase: PenPhase, samples: DoubleArray) {
        if (penTool == Tool.BRUSH) {
            for (index in samples.indices step 3) {
                preview.add(samples[index], samples[index + 1], samples[index + 2])
            }
        }
        sendPen(phase, penTool, samples)
    }

    /** `x, y, pressure` triples in document coordinates. */
    private fun samplesOf(event: MotionEvent, withHistory: Boolean): DoubleArray {
        val history = if (withHistory) event.historySize else 0
        val samples = DoubleArray((history + 1) * 3)
        // Mice and fingers report no meaningful pressure; use the middle, like the desktop app.
        val toolType = event.getToolType(0)
        val hasPressure = toolType == MotionEvent.TOOL_TYPE_STYLUS || toolType == MotionEvent.TOOL_TYPE_ERASER
        for (index in 0..history) {
            val x: Float
            val y: Float
            val rawPressure: Float
            if (index < history) {
                x = event.getHistoricalX(0, index)
                y = event.getHistoricalY(0, index)
                rawPressure = event.getHistoricalPressure(0, index)
            } else {
                x = event.x
                y = event.y
                rawPressure = event.pressure
            }
            samples[index * 3] = camera.toDocumentX(x)
            samples[index * 3 + 1] = camera.toDocumentY(y)
            samples[index * 3 + 2] = if (hasPressure) rawPressure.toDouble().coerceIn(0.0, 1.0) else DEFAULT_PRESSURE
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
    }
}
