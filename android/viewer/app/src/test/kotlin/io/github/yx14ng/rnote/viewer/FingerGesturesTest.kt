package io.github.yx14ng.rnote.viewer

import io.github.yx14ng.rnote.viewer.FingerGestures.Action
import io.github.yx14ng.rnote.viewer.FingerGestures.Event
import io.github.yx14ng.rnote.viewer.FingerGestures.State
import org.junit.Assert.assertEquals
import org.junit.Test

/** A finger navigates until it moves alone past the slop, then it draws. */
class FingerGesturesTest {
    private fun run(vararg events: Event): List<Pair<State, Action>> {
        var state = State.NONE
        return events.map { event -> FingerGestures.next(state, event).also { state = it.first } }
    }

    @Test
    fun aFingerThatMovesAloneDrawsUntilItLifts() {
        assertEquals(
            listOf(
                State.UNDECIDED to Action.NAVIGATE,
                State.UNDECIDED to Action.NAVIGATE,
                State.DRAWING to Action.START_DRAWING,
                State.DRAWING to Action.CONTINUE_DRAWING,
                State.NONE to Action.END_DRAWING,
            ),
            run(Event.DOWN, Event.MOVE_WITHIN_SLOP, Event.MOVE_PAST_SLOP, Event.MOVE_PAST_SLOP, Event.UP),
        )
    }

    @Test
    fun aSecondFingerBeforeDrawingNavigates_andNeverStartsAStroke() {
        val steps = run(Event.DOWN, Event.POINTER_DOWN, Event.MOVE_PAST_SLOP, Event.OTHER_FINGER_UP, Event.UP)
        assertEquals(listOf(Action.NAVIGATE, Action.NAVIGATE, Action.NAVIGATE, Action.NAVIGATE, Action.NAVIGATE), steps.map { it.second })
        assertEquals(State.NONE, steps.last().first)
    }

    @Test
    fun aRestingHandWhileDrawingChangesNothing_andTheDrawingFingerLiftingEndsTheStroke() {
        val steps = run(Event.DOWN, Event.MOVE_PAST_SLOP, Event.POINTER_DOWN, Event.OTHER_FINGER_UP, Event.DRAWING_FINGER_UP, Event.UP)
        assertEquals(
            listOf(Action.NAVIGATE, Action.START_DRAWING, Action.NOTHING, Action.NOTHING, Action.END_DRAWING, Action.NAVIGATE),
            steps.map { it.second },
        )
    }

    @Test
    fun cancellingOrAStylusTakingOverDropsTheStrokeOrTheNavigation() {
        assertEquals(State.NONE to Action.CANCEL_DRAWING, FingerGestures.next(State.DRAWING, Event.CANCEL))
        assertEquals(State.NONE to Action.NAVIGATE, FingerGestures.next(State.UNDECIDED, Event.CANCEL))
        assertEquals(State.NONE to Action.CANCEL_DRAWING, FingerGestures.next(State.DRAWING, Event.STYLUS_TOOK_OVER))
        assertEquals(State.NONE to Action.CANCEL_NAVIGATION, FingerGestures.next(State.NAVIGATING, Event.STYLUS_TOOK_OVER))
    }
}
