package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.annotation.TargetApi
import android.content.Context
import android.os.Build
import android.view.MotionEvent
import android.view.MotionPredictor

/**
 * Guesses where the pen is going, from the samples so far, so the live stroke can reach a
 * little past the last sample and keep up with the pen tip. Only Android 14 and later can
 * tell; earlier versions get no prediction.
 */
internal interface PenPrediction {
    /** Feeds a sample of the stroke being drawn, from its down event on. */
    fun record(event: MotionEvent)

    /** Where the pen will likely be shortly after [event]; the caller recycles the result. */
    fun predict(event: MotionEvent): MotionEvent?

    companion object {
        fun create(context: Context): PenPrediction? =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) SystemPrediction(context) else null
    }
}

// Only created behind the version check above; @RequiresApi would need AndroidX.
@SuppressLint("UseRequiresApi")
@TargetApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
private class SystemPrediction(context: Context) : PenPrediction {
    private val predictor = MotionPredictor(context)

    override fun record(event: MotionEvent) {
        if (predictor.isPredictionAvailable(event.deviceId, event.source)) {
            // A stream it can't follow (another device midway) only costs the prediction.
            runCatching { predictor.record(event) }
        }
    }

    override fun predict(event: MotionEvent): MotionEvent? {
        if (!predictor.isPredictionAvailable(event.deviceId, event.source)) return null
        return runCatching { predictor.predict(event.eventTimeNanos + AHEAD_NANOS) }.getOrNull()
    }

    private companion object {
        /** About one frame ahead: further guesses wander off the stroke. */
        const val AHEAD_NANOS = 16_000_000L
    }
}
