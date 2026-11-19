package io.github.yx14ng.rnote.viewer

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.hypot

/** The live stroke is drawn through a curve, not straight from sample to sample. */
class PreviewCurveTest {
    private fun points(vararg xy: Double): DoubleArray =
        xy.toList().chunked(2).flatMap { (x, y) -> listOf(x, y, 1.0) }.toDoubleArray()

    @Test
    fun theCurveStartsAndEndsWhereThePenDid() {
        val curve = PreviewCurve.smooth(points(0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 20.0, 10.0), 12)

        assertEquals(0.0, curve[0], 1e-9)
        assertEquals(0.0, curve[1], 1e-9)
        assertEquals(20.0, curve[curve.size - 3], 1e-9)
        assertEquals(10.0, curve[curve.size - 2], 1e-9)
    }

    @Test
    fun aCornerIsRounded() {
        val curve = PreviewCurve.smooth(points(0.0, 0.0, 10.0, 0.0, 10.0, 10.0), 9)

        // No point of the curve reaches the sharp corner at (10, 0).
        val nearest = (curve.indices step 3).minOf { hypot(curve[it] - 10.0, curve[it + 1]) }
        assertTrue("got $nearest", nearest > 1.0)
    }

    @Test
    fun aStraightLineStaysStraight() {
        val curve = PreviewCurve.smooth(points(0.0, 0.0, 5.0, 0.0, 10.0, 0.0, 15.0, 0.0), 12)

        for (i in curve.indices step 3) assertEquals(0.0, curve[i + 1], 1e-9)
    }

    @Test
    fun oneOrTwoSamplesAreKeptAsTheyAre() {
        assertEquals(3, PreviewCurve.smooth(points(1.0, 2.0), 3).size)
        assertEquals(6, PreviewCurve.smooth(points(1.0, 2.0, 3.0, 4.0), 6).size)
    }
}
