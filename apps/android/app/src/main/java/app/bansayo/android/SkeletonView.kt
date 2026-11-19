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

    /** Bones drawn in the strike color instead of tracking cyan (the home screen's hero pose). */
    var strikeBones: Set<Pair<Int, Int>> = emptySet()

    private val glow = Paint(Paint.ANTI_ALIAS_FLAG).apply { strokeWidth = 18f; strokeCap = Paint.Cap.ROUND }
    private val bone = Paint(Paint.ANTI_ALIAS_FLAG).apply { strokeWidth = 5f; strokeCap = Paint.Cap.ROUND }
    private val halo = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Palette.alpha(Palette.GOLD, 70) }
    private val joint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Palette.GOLD }
    private val core = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.WHITE }
    private val grid = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Palette.alpha(Palette.ACCENT, 34); strokeWidth = 1.5f }
    private val back = Paint().apply { color = Palette.SURFACE }

    /** A perspective floor grid, the "VR room" behind the figure when there is no camera image. */
    private fun drawFloor(canvas: Canvas) {
        val w = width.toFloat()
        val h = height.toFloat()
        val horizon = h * 0.55f
        for (i in -8..8) canvas.drawLine(w / 2 + i * w * 0.04f, horizon, w / 2 + i * w * 0.22f, h, grid)
        var y = 0.02f
        while (y < 1f) {
            canvas.drawLine(0f, horizon + (h - horizon) * y, w, horizon + (h - horizon) * y, grid)
            y *= 1.6f
        }
    }

    override fun onDraw(canvas: Canvas) {
        canvas.drawRect(0f, 0f, width.toFloat(), height.toFloat(), back)
        if (image == null) drawFloor(canvas)
        val (iw, ih) = imageSize
        if (iw <= 0 || ih <= 0) return
        val s = minOf(width / iw.toFloat(), height / ih.toFloat())
        val ox = (width - iw * s) / 2
        val oy = (height - ih * s) / 2
        image?.let { canvas.drawBitmap(it, null, RectF(ox, oy, ox + iw * s, oy + ih * s), null) }
        fun at(i: Int) = points.getOrNull(i)?.let { floatArrayOf(ox + it[0] * s, oy + it[1] * s) }
        for (pair in if (schema.startsWith("coco17")) COCO17 else CANONICAL) {
            val p = at(pair.first) ?: continue
            val q = at(pair.second) ?: continue
            val color = if (pair in strikeBones) Palette.STRIKE else Palette.ACCENT
            glow.color = Palette.alpha(color, 60)
            bone.color = color
            canvas.drawLine(p[0], p[1], q[0], q[1], glow)
            canvas.drawLine(p[0], p[1], q[0], q[1], bone)
        }
        for (i in points.indices) at(i)?.let {
            canvas.drawCircle(it[0], it[1], 13f, halo)
            canvas.drawCircle(it[0], it[1], 6f, joint)
            canvas.drawCircle(it[0], it[1], 2.5f, core)
        }
    }

    companion object {
        /** A side kick in COCO-17 on a 560x460 image: the home screen's hero pose. */
        val SIDE_KICK: List<FloatArray?> = listOf(
            420f to 150f, 430f to 141f, 424f to 143f, 405f to 148f, 410f to 151f,
            398f to 214f, 432f to 210f, 446f to 268f, 368f to 262f, 478f to 214f, 392f to 212f,
            408f to 330f, 440f to 324f, 392f to 430f, 572f to 300f, 398f to 532f, 720f to 270f,
        ).map { floatArrayOf(it.first - 240f, it.second - 110f) }
        val KICK_BONES = setOf(12 to 14, 14 to 16)

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
