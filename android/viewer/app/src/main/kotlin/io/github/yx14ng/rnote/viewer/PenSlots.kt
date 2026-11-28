package io.github.yx14ng.rnote.viewer

/**
 * The two pen buttons of the tool row: the pen, in the style last chosen for writing, and
 * the highlighter, one tap apart, as in other note-taking apps.
 */
internal object PenSlots {
    enum class Slot { PEN, HIGHLIGHTER }

    /** The brush style of [slot], the pen being in [penStyle]. */
    fun styleFor(slot: Slot, penStyle: BrushStyle): BrushStyle = when (slot) {
        Slot.PEN -> penStyle
        Slot.HIGHLIGHTER -> BrushStyle.MARKER
    }

    /** The pen's style after choosing [style]: the marker is the highlighter's, not the pen's. */
    fun penStyleAfter(style: BrushStyle, previous: BrushStyle): BrushStyle =
        if (style == BrushStyle.MARKER) previous else style
}
