package app.bansayo.android

import android.graphics.BitmapFactory
import android.os.Bundle
import android.view.Gravity
import android.widget.TextView
import android.widget.AdapterView
import android.widget.LinearLayout
import androidx.activity.ComponentActivity
import okhttp3.Call
import okhttp3.Callback
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONObject
import java.io.IOException

/**
 * Live view (ADR-042): a camera's preview with the engine's landmarks, and the training session's
 * state, counts, phase and cues from the `training` topic. Everything shown comes from the engine.
 */
class ViewerActivity : ComponentActivity() {
    private lateinit var conn: Connection
    private val view by lazy { SkeletonView(this) }
    private val camera by lazy { spinner(emptyList()) }
    private val plan by lazy { title("No training session", 18f) }
    private val phase by lazy { pill("Idle", Palette.MUTED) }
    private val counter by lazy { big("–", Palette.ACCENT, 46f) }
    private val unit by lazy { eyebrow("Reps", Palette.MUTED) }
    private val setInfo by lazy { big("", Palette.TEXT, 20f) }
    private val restInfo by lazy { eyebrow("", Palette.GOLD) }
    private val details by lazy { label().apply { typeface = Fonts.mono; textSize = 13f } }
    private val cue by lazy { Notice(this) }
    private var socket: WebSocket? = null
    private var texts: JSONObject = JSONObject()
    @Volatile private var running = true

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        conn = requireConnection() ?: return
        val stage = frame(view).apply {
            background = rounded(Palette.SURFACE, 20, Palette.STROKE)
            post { roundCorners(dp(20).toFloat()) }
        }
        val counts = LinearLayout(this).apply {
            gravity = Gravity.BOTTOM
            addView(column(unit, counter, padding = 0, gap = 0), LinearLayout.LayoutParams(0, -2, 1f))
            addView(column(restInfo, setInfo, padding = 0, gap = 2).apply { gravity = Gravity.END })
        }
        val hud = column(
            LinearLayout(this).apply {
                gravity = Gravity.CENTER_VERTICAL
                addView(plan, LinearLayout.LayoutParams(0, -2, 1f))
                addView(phase)
            },
            counts,
            cue,
            details,
            padding = 16, gap = 10,
        )
        screen(
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                setPadding(dp(16), 0, dp(16), 0)
                addView(camera, LinearLayout.LayoutParams(-1, -2))
                addView(stage, LinearLayout.LayoutParams(-1, 0, 3f).apply { topMargin = dp(10) })
                addView(scroll(hud), LinearLayout.LayoutParams(-1, 0, 2f))
            },
            title = "Live view",
        )
        Engine.call(this, conn, "GET", "state") { data, error ->
            val ids = Engine.cameras(data as? JSONObject).map { Engine.id(it) }
            camera.setItems(ids)
            if (error != null) cue.show(error)
            else if (ids.isEmpty()) cue.show("The engine has no camera. Check the devices in Settings.", settings = true, tone = Palette.GOLD)
        }
        camera.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
            override fun onItemSelected(p: AdapterView<*>?, v: android.view.View?, pos: Int, id: Long) {
                view.setLandmarks(emptyList(), view.imageSize, view.schema)
            }
            override fun onNothingSelected(p: AdapterView<*>?) {}
        }
        Engine.call(this, conn, "GET", "content/i18n/en") { data, _ ->
            (data as? JSONObject)?.let { texts = it.optJSONObject("texts") ?: it }
        }
        stream()
        pollPreview()
    }

    private fun text(key: String) = texts.optString(key, key)

    private fun stream() {
        socket = Engine.socket(conn, "stream?topics=landmarks_2d,training&max_rate_hz=20", object : WebSocketListener() {
            override fun onMessage(webSocket: WebSocket, text: String) {
                val m = runCatching { JSONObject(text) }.getOrNull() ?: return
                runOnUiThread { handle(m) }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                runOnUiThread {
                    if (response?.code == 401) cue.show(Engine.REFUSED)
                    else cue.show("Stream lost (${t.message}); reconnecting…", settings = true, tone = Palette.GOLD)
                }
                if (running) view.postDelayed({ if (running) stream() }, 2000)
            }
        })
    }

    private fun handle(m: JSONObject) {
        when (m.optString("type")) {
            "landmarks_2d" -> {
                val l = m.getJSONObject("landmarks")
                val meta = l.getJSONObject("frame_meta")
                if (meta.optString("device_id") != camera.selected()) return
                val size = meta.getJSONArray("image_size_px")
                val pts = l.getJSONArray("points")
                view.setLandmarks(
                    (0 until pts.length()).map { i ->
                        pts.optJSONObject(i)?.let { floatArrayOf(it.getDouble("x_px").toFloat(), it.getDouble("y_px").toFloat()) }
                    },
                    size.getInt(0) to size.getInt(1),
                    l.optString("schema_id"),
                )
            }
            "training_state" -> showState(m.optJSONObject("state"))
            "training_event" -> {
                val e = m.getJSONObject("event")
                val c = e.optJSONObject("data")?.optJSONObject("cue")
                if (c != null) {
                    val observed = c.optJSONObject("observed")?.optString("key")
                    val suggestion = c.optJSONObject("suggestion")?.optString("key")
                    cue.show(listOfNotNull(observed, suggestion).joinToString("\n") { text(it) }, settings = false, tone = Palette.STRIKE)
                }
                when (e.optString("type")) {
                    "session_paused" -> cue.show("Paused: ${e.optJSONObject("data")?.optString("cause")}", settings = false, tone = Palette.GOLD)
                    "tracking_lost" -> cue.show("Tracking lost — step back into view.", settings = false, tone = Palette.GOLD)
                    "set_completed" -> {
                        val focus = Engine.list(e.optJSONObject("data")?.optJSONArray("cues")).firstOrNull {
                            !it.optBoolean("resolved") && !it.optString("id").startsWith("improved.")
                        }
                        cue.show(
                            focus?.optJSONObject("suggestion")?.optString("key")?.let { text(it) } ?: "Set complete",
                            settings = false,
                            tone = Palette.OK,
                        )
                    }
                }
            }
            "notice" -> cue.show(m.optString("message"), settings = false)
        }
    }

    private fun big(text: String, color: Int, size: Float) = TextView(this).apply {
        this.text = text
        textSize = size
        typeface = Fonts.mono
        setTextColor(color)
    }

    private fun showState(s: JSONObject?) {
        if (s == null) {
            plan.text = "No training session"
            phase.setPill("Idle", Palette.MUTED)
            counter.text = "–"
            setInfo.text = ""
            restInfo.text = ""
            details.text = ""
            return
        }
        val cur = s.optJSONObject("current")
        val counts = cur?.optJSONObject("counts")
        val sets = cur?.optJSONObject("sets")
        val totals = s.optJSONObject("totals")
        plan.text = s.optJSONObject("plan")?.optString("title") ?: "Session"
        val sub = cur?.takeUnless { it.isNull("sub_state") }?.optString("sub_state")?.replace('_', ' ')
        phase.setPill(listOfNotNull(s.optString("phase"), sub).joinToString(" · "), if (s.optString("phase") == "active") Palette.OK else Palette.GOLD)
        if (counts != null) {
            unit.text = if (counts.optString("unit") == "seconds") "SECONDS" else "REPS"
            counter.text = "${counts.optInt("done")}/${counts.optInt("target")}"
        }
        setInfo.text = s.optJSONObject("position")?.optInt("set", 0)?.takeIf { it > 0 }
            ?.let { "SET $it/${sets?.optInt("target") ?: 0}" } ?: ""
        val timer = cur?.optJSONObject("timer")
        restInfo.text = if (timer != null && timer.optString("kind") == "rest") {
            val remaining = (timer.optLong("duration_ms") - timer.optLong("elapsed_ms")).coerceAtLeast(0)
            "REST ${(remaining + 999) / 1000} S"
        } else ""
        details.text = buildString {
            cur?.optString("phase_id")?.takeIf { it.isNotEmpty() && it != "null" }?.let { append("Phase ").append(it).append('\n') }
            append("Completed ${totals?.optInt("reps_completed") ?: 0} · other ${totals?.optInt("reps_other") ?: 0}")
            s.optJSONObject("last_rep")?.optJSONArray("measurements")?.let { ms ->
                append("\n\nLAST REPETITION")
                for (i in 0 until ms.length()) {
                    val x = ms.getJSONObject(i)
                    if (x.optString("grade") == "X") continue
                    val v = if (x.isNull("value")) x.optDouble("norm").let { "%.2f body-rel.".format(it) }
                    else x.optDouble("value").let { "%.0f %s".format(it, x.optString("unit")) }
                    append("\n").append(x.optString("metric").substringBefore('@')).append("  ").append(v)
                        .append("  [").append(x.optString("grade")).append(']')
                }
            }
        }
    }

    /** The engine's preview JPEG (at most 10 per second), never stored. */
    private fun pollPreview() {
        if (!running) return
        val id = camera.selected()
        if (id == null) {
            view.postDelayed({ pollPreview() }, 500)
            return
        }
        Engine.client.newCall(Engine.request(conn, "GET", "cameras/$id/preview.jpg")).enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) {
                view.postDelayed({ pollPreview() }, 1000)
            }

            override fun onResponse(call: Call, response: Response) {
                val bytes = response.body?.bytes()
                val bitmap = if (response.isSuccessful && bytes != null) BitmapFactory.decodeByteArray(bytes, 0, bytes.size) else null
                runOnUiThread {
                    if (bitmap != null) view.image = bitmap
                    view.postDelayed({ pollPreview() }, 150)
                }
            }
        })
    }

    override fun onDestroy() {
        running = false
        socket?.close(1000, "closed")
        super.onDestroy()
    }
}
