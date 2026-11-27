package io.github.yx14ng.rnote.viewer

/**
 * Each brush style keeps its own color and width, like the pens of a pencil case: the
 * marker, a highlighter drawn under the ink, doesn't take the fountain pen's black, and the
 * pen doesn't come back see-through.
 */
internal object BrushMemory {
    /** What the marker draws with until the user changes it: see-through yellow, wide. */
    val HIGHLIGHTER = Brush(0x66FFD600, 12.0)

    /**
     * Switching from the style [from], drawing with [current], to [to]: the brushes kept per
     * style, now with [current] for [from], and the brush [to] draws with.
     */
    fun switch(
        remembered: Map<BrushStyle, Brush>,
        from: BrushStyle,
        current: Brush,
        to: BrushStyle,
    ): Pair<Map<BrushStyle, Brush>, Brush> {
        val kept = remembered + (from to current)
        val brush = kept[to].takeIf { to != from } ?: when {
            to == from -> current
            to == BrushStyle.MARKER -> HIGHLIGHTER
            from == BrushStyle.MARKER -> Brush(opaque(current.color), BrushPalette.DEFAULT_WIDTH)
            else -> current
        }
        return kept to brush
    }

    private fun opaque(color: Int): Int = color or 0xFF000000.toInt()
}
