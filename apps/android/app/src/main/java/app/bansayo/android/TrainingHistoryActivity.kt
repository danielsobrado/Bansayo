package app.bansayo.android

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.ScrollView
import androidx.activity.ComponentActivity
import org.json.JSONObject
import java.util.Locale

class TrainingHistoryActivity : ComponentActivity() {
    private lateinit var conn: Connection
    private lateinit var screen: ScrollView
    private val list by lazy { column() }
    private val details by lazy { label() }
    private val status by lazy { label() }
    private var detailRequest = 0
    private var refreshRequest = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        conn = Connection.load(this) ?: return finish()
        screen = scroll(column(
            heading("Train"), status,
            row(button("Refresh") { refresh() }, button("Open live viewer") {
                startActivity(Intent(this, ViewerActivity::class.java))
            }),
            details, heading("Saved sessions"), list,
        ))
        setContentView(screen)
    }

    override fun onResume() {
        super.onResume()
        if (::conn.isInitialized) refresh()
    }

    private fun refresh() {
        val request = ++refreshRequest
        detailRequest++
        details.text = ""
        list.removeAllViews()
        status.text = "Loading training…"
        Engine.call(this, conn, "GET", "state") { data, error ->
            if (request != refreshRequest || isFinishing || isDestroyed) return@call
            if (error != null) {
                status.text = error
                return@call
            }
            val state = data as? JSONObject
            val athlete = state?.optJSONObject("athlete")
            val current = state?.optJSONObject("training")
            status.text = listOf(
                athlete?.optString("display_name") ?: "Select an athlete in Admin.",
                current?.let { "${it.optString("title")} · ${it.optString("phase")}" } ?: "No live session",
            ).joinToString("\n")
            val id = athlete?.optString("id").orEmpty()
            if (id.isEmpty()) return@call
            Engine.call(this, conn, "GET", "training/sessions?athlete_id=${Uri.encode(id)}&limit=50") { sessions, problem ->
                if (request != refreshRequest || isFinishing || isDestroyed) return@call
                list.removeAllViews()
                val entries = Engine.list(sessions)
                if (entries.isEmpty()) list.addView(label(problem ?: "No saved sessions."))
                for (entry in entries) {
                    val title = "${entry.optString("title")} · ${entry.optString("started_utc")}"
                    list.addView(button(title) { openSession(entry) })
                }
            }
        }
    }

    private fun openSession(entry: JSONObject) {
        val id = Uri.encode(entry.optString("id"))
        val request = ++detailRequest
        details.text = "Loading session…"
        screen.smoothScrollTo(0, 0)
        Engine.call(this, conn, "GET", "training/sessions/$id") { value, error ->
            if (request != detailRequest || isFinishing || isDestroyed) return@call
            val session = value as? JSONObject
            val saved = session?.optJSONObject("session") ?: entry
            val summary = session?.optJSONObject("summary")
            val intro = buildString {
                appendLine(saved.optString("title"))
                appendLine("${saved.optString("status")} · ${saved.optString("origin")}")
                if (saved.optString("plan_kind") == "workout") {
                    val totals = summary?.optJSONObject("totals")
                    if (totals != null && totals.has("active_ms")) {
                        val seconds = totals.optLong("active_ms") / 1000
                        appendLine("Class time %d:%02d".format(seconds / 60, seconds % 60))
                    }
                    if (summary != null && summary.has("blocks_completed")) {
                        appendLine("${summary.optInt("blocks_completed")} blocks completed")
                    }
                } else {
                    appendLine("${saved.optInt("sets")} sets · ${saved.optInt("reps_completed")} completed reps")
                }
            }
            details.text = error ?: intro
            if (error != null || saved.optString("plan_kind") != "technique") return@call
            Engine.call(this, conn, "GET", "training/sessions/$id/clinic-review") reviewCall@{ review, problem ->
                if (request != detailRequest || isFinishing || isDestroyed) return@reviewCall
                val result = review as? JSONObject
                val first = result?.optJSONObject("first")
                val second = result?.optJSONObject("second")
                if (result == null || first == null || second == null) {
                    details.text = intro + (problem ?: "Two attempts from matching sets are needed for a comparison.")
                    return@reviewCall
                }
                details.text = buildString {
                    appendLine(intro)
                    appendLine("First completed attempt per set · ${result.optString("side")}")
                    for (row in Engine.list(result.optJSONArray("measurements"))) {
                        appendLine(metricName(row.optString("metric")))
                        appendLine("  Set ${first.optInt("set_index")}  ${metric(row.optJSONObject("first"))}")
                        appendLine("  Set ${second.optInt("set_index")}  ${metric(row.optJSONObject("second"))}")
                        if (!row.optBoolean("comparable")) appendLine("  Comparison unavailable")
                    }
                }
            }
        }
    }

    private fun metricName(id: String): String = when (id) {
        "kick.peak_ankle_height" -> "Peak foot height"
        "kick.retraction_time" -> "Retraction time"
        "kick.support_pivot" -> "Support-foot pivot"
        else -> id
    }

    private fun metric(m: JSONObject?): String {
        if (m == null) return "Unavailable"
        val grade = m.optString("grade")
        if (grade == "X") {
            val why = m.optJSONArray("reasons")?.optJSONObject(0)?.optString("message")
            return why ?: "Not assessed"
        }
        val value = m.optDouble("value")
        val norm = m.optDouble("norm")
        val text = if (value.isFinite()) {
            val unit = m.optString("unit")
            when (unit) {
                "m" -> String.format(Locale.US, "%.1f cm", value * 100)
                "rad" -> String.format(Locale.US, "%.1f°", Math.toDegrees(value))
                else -> String.format(Locale.US, "%.2f %s", value, unit)
            }
        } else if (norm.isFinite()) String.format(Locale.US, "%.2f body-relative", norm)
        else return "Unavailable"
        return "$text · grade $grade"
    }
}
