package io.github.yx14ng.rnote.viewer

/**
 * What a finger on the page does: it navigates (pan, zoom, taps) until it moves alone past
 * the touch slop, and then draws; extra fingers while drawing are a resting hand.
 */
internal object FingerGestures {
    enum class State { NONE, UNDECIDED, DRAWING, NAVIGATING }

    enum class Event {
        DOWN,
        POINTER_DOWN,
        MOVE_WITHIN_SLOP,
        MOVE_PAST_SLOP,
        /** The finger drawing lifted while another stays. */
        DRAWING_FINGER_UP,
        /** Another finger than the one drawing lifted. */
        OTHER_FINGER_UP,
        UP,
        CANCEL,
        STYLUS_TOOK_OVER,
    }

    enum class Action {
        NOTHING,
        /** The event goes to the pan, zoom and tap detectors. */
        NAVIGATE,
        /** Navigation stops and a stroke starts where the finger went down. */
        START_DRAWING,
        CONTINUE_DRAWING,
        END_DRAWING,
        CANCEL_DRAWING,
        /** The detectors forget what they saw. */
        CANCEL_NAVIGATION,
    }

    /** The state after [event] in [state], and what to do. */
    fun next(state: State, event: Event): Pair<State, Action> = when (event) {
        Event.DOWN -> State.UNDECIDED to Action.NAVIGATE
        Event.POINTER_DOWN -> when (state) {
            State.UNDECIDED, State.NAVIGATING -> State.NAVIGATING to Action.NAVIGATE
            State.DRAWING, State.NONE -> state to Action.NOTHING
        }
        Event.MOVE_WITHIN_SLOP, Event.MOVE_PAST_SLOP -> when (state) {
            State.UNDECIDED ->
                if (event == Event.MOVE_PAST_SLOP) State.DRAWING to Action.START_DRAWING else state to Action.NAVIGATE
            State.DRAWING -> state to Action.CONTINUE_DRAWING
            State.NAVIGATING -> state to Action.NAVIGATE
            State.NONE -> state to Action.NOTHING
        }
        Event.DRAWING_FINGER_UP, Event.OTHER_FINGER_UP -> when (state) {
            State.NAVIGATING -> state to Action.NAVIGATE
            State.DRAWING ->
                if (event == Event.DRAWING_FINGER_UP) State.NONE to Action.END_DRAWING else state to Action.NOTHING
            State.UNDECIDED, State.NONE -> state to Action.NOTHING
        }
        Event.UP -> State.NONE to if (state == State.DRAWING) Action.END_DRAWING else Action.NAVIGATE
        Event.CANCEL -> State.NONE to if (state == State.DRAWING) Action.CANCEL_DRAWING else Action.NAVIGATE
        Event.STYLUS_TOOK_OVER ->
            State.NONE to if (state == State.DRAWING) Action.CANCEL_DRAWING else Action.CANCEL_NAVIGATION
    }
}
