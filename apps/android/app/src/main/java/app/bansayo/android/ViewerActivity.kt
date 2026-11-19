package app.bansayo.android

import android.graphics.BitmapFactory
import android.os.Bundle
import android.view.ViewGroup
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
    private val training by lazy { label("No training session.") }
    private val notice by lazy { label() }
    private var socket: WebSocket? = null
    private var texts: JSONObject = JSONObject()
    @Volatile private var running = true

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        conn = Connection.load(this) ?: return finish()
        training.textSize = 18f
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(column(camera).apply { setPadding(dp(8), dp(8), dp(8), 0) })
            addView(view, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 3f))
            addView(scroll(column(training, notice)), LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 2f))
        }
        setContentView(root)
        Engine.call(this, conn, "GET", "state") { data, error ->
            val ids = Engine.cameras(data as? JSONObject).map { Engine.id(it) }
            camera.setItems(ids)
            if (error != null) notice.text = error
            else if (ids.isEmpty()) notice.text = "The engine has no camera."
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
                    notice.text = if (response?.code == 401) "The engine refused the pairing token: pair again."
                    else "Stream lost (${t.message}); reconnecting…"
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
                val cue = e.optJSONObject("data")?.optJSONObject("cue")
                if (cue != null) {
                    val observed = cue.optJSONObject("observed")?.optString("key")
                    val suggestion = cue.optJSONObject("suggestion")?.optString("key")
                    notice.text = listOfNotNull(observed, suggestion).joinToString("\n") { text(it) }
                }
                when (e.optString("type")) {
                    "session_paused" -> notice.text = "Paused: ${e.optJSONObject("data")?.optString("cause")}"
                    "tracking_lost" -> notice.text = "Tracking lost — step back into view."
                    "set_completed" -> {
                        val focus = Engine.list(e.optJSONObject("data")?.optJSONArray("cues")).firstOrNull {
                            !it.optBoolean("resolved") && !it.optString("id").startsWith("improved.")
                        }
                        notice.text = focus?.optJSONObject("suggestion")?.optString("key")?.let { text(it) }
                            ?: "Set complete"
                    }
                }
            }
            "notice" -> notice.text = m.optString("message")
        }
    }

    private fun showState(s: JSONObject?) {
        if (s == null) {
            training.text = "No training session."
            return
        }
        val cur = s.optJSONObject("current")
        val counts = cur?.optJSONObject("counts")
        val sets = cur?.optJSONObject("sets")
        val totals = s.optJSONObject("totals")
        training.text = buildString {
            append(s.optJSONObject("plan")?.optString("title") ?: "Session").append(" — ").append(s.optString("phase"))
            cur?.takeUnless { it.isNull("sub_state") }?.optString("sub_state")?.let { append(" · ").append(it.replace('_', ' ')) }
            cur?.optString("phase_id")?.takeIf { it.isNotEmpty() && it != "null" }?.let { append(" · ").append(it) }
            append('\n')
            if (counts != null) {
                val unit = if (counts.optString("unit") == "seconds") "Seconds" else "Reps"
                append("$unit ${counts.optInt("done")} / ${counts.optInt("target")}")
            }
            s.optJSONObject("position")?.optInt("set", 0)?.takeIf { it > 0 }?.let {
                append("   Set $it of ${sets?.optInt("target") ?: 0}")
            }
            val timer = cur?.optJSONObject("timer")
            if (timer != null && timer.optString("kind") == "rest") {
                val remaining = (timer.optLong("duration_ms") - timer.optLong("elapsed_ms")).coerceAtLeast(0)
                append("\nRest ${(remaining + 999) / 1000} s")
            }
            append("\nCompleted ${totals?.optInt("reps_completed") ?: 0}, other ${totals?.optInt("reps_other") ?: 0}")
            s.optJSONObject("last_rep")?.optJSONArray("measurements")?.let { ms ->
                append("\nLast repetition:")
                for (i in 0 until ms.length()) {
                    val x = ms.getJSONObject(i)
                    if (x.optString("grade") == "X") continue
                    val v = if (x.isNull("value")) x.optDouble("norm").let { "%.2f body-rel.".format(it) }
                    else x.optDouble("value").let { "%.0f %s".format(it, x.optString("unit")) }
                    append("\n  ").append(x.optString("metric").substringBefore('@')).append(": ").append(v)
                        .append(" (").append(x.optString("grade")).append(')')
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
