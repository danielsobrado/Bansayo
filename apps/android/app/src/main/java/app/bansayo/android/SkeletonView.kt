package app.bansayo.android

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.view.View

/**
 * A camera image with the engine's 2D landmarks drawn over it. Draws only what the engine sent;
 * computes nothing about the movement (10 §4).
 */
class SkeletonView(context: Context) : View(context) {
    var image: Bitmap? = null
        set(v) { field = v; invalidate() }

    /** Landmarks in image pixels (`null` where not detected), the image size and schema id. */
    var points: List<FloatArray?> = emptyList()
    var imageSize: Pair<Int, Int> = 1280 to 720
    var schema: String = "coco17.body"

    fun setLandmarks(points: List<FloatArray?>, size: Pair<Int, Int>, schema: String) {
        this.points = points
        this.imageSize = size
        this.schema = schema
        invalidate()
    }

    private val bone = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.rgb(80, 220, 120); strokeWidth = 6f; strokeCap = Paint.Cap.ROUND
    }
    private val joint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.rgb(255, 210, 60) }
    private val back = Paint().apply { color = Color.rgb(24, 24, 28) }

    override fun onDraw(canvas: Canvas) {
        canvas.drawRect(0f, 0f, width.toFloat(), height.toFloat(), back)
        val (iw, ih) = imageSize
        if (iw <= 0 || ih <= 0) return
        val s = minOf(width / iw.toFloat(), height / ih.toFloat())
        val ox = (width - iw * s) / 2
        val oy = (height - ih * s) / 2
        image?.let { canvas.drawBitmap(it, null, RectF(ox, oy, ox + iw * s, oy + ih * s), null) }
        fun at(i: Int) = points.getOrNull(i)?.let { floatArrayOf(ox + it[0] * s, oy + it[1] * s) }
        for ((a, b) in if (schema.startsWith("coco17")) COCO17 else CANONICAL) {
            val p = at(a) ?: continue
            val q = at(b) ?: continue
            canvas.drawLine(p[0], p[1], q[0], q[1], bone)
        }
        for (i in points.indices) at(i)?.let { canvas.drawCircle(it[0], it[1], 7f, joint) }
    }

    companion object {
        /** COCO-17: face, arms, trunk, legs. */
        val COCO17 = listOf(
            0 to 1, 0 to 2, 1 to 3, 2 to 4, 5 to 6, 5 to 7, 7 to 9, 6 to 8, 8 to 10,
            5 to 11, 6 to 12, 11 to 12, 11 to 13, 13 to 15, 12 to 14, 14 to 16,
        )

        /** `bansayo.body.v1.points` (simulated cameras). */
        val CANONICAL = listOf(
            0 to 1, 1 to 2, 2 to 3, 3 to 4, 3 to 6, 6 to 7, 7 to 8, 3 to 10, 10 to 11, 11 to 12,
            0 to 13, 13 to 14, 14 to 15, 0 to 16, 16 to 17, 17 to 18, 15 to 22, 18 to 23, 15 to 24, 18 to 25,
        )
    }
}
