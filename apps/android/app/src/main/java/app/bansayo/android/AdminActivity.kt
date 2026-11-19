package app.bansayo.android

import android.os.Bundle
import android.view.Gravity
import android.widget.CheckBox
import android.widget.LinearLayout
import androidx.activity.ComponentActivity
import org.json.JSONObject

/**
 * Coach console over the engine's existing API (ADR-042): athletes with motion-data consent and a
 * training session's plan and commands. Devices and camera exposure live in [SettingsActivity].
 */
class AdminActivity : ComponentActivity() {
    private lateinit var conn: Connection
    private val athletes by lazy { spinner(emptyList()) }
    private val newId by lazy { field("e.g. p1") }
    private val newName by lazy { field("A pseudonym is fine") }
    private val consent by lazy {
        CheckBox(this).apply {
            text = "Motion-data consent recorded (sessions are saved)"
            setTextColor(Palette.MUTED)
            buttonTintList = android.content.res.ColorStateList.valueOf(Palette.ACCENT)
        }
    }
    private val technique by lazy { spinner(emptyList()) }
    private val side by lazy { spinner(listOf("right", "left")) }
    private val sets by lazy { field("3", "3", number = true) }
    private val reps by lazy { field("10", "10", number = true) }
    private val rest by lazy { field("30", "30", number = true) }
    private val session by lazy { pill("No session", Palette.MUTED) }
    private val message by lazy { Notice(this) }
    private var techniques: List<JSONObject> = emptyList()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        conn = requireConnection() ?: return
        screen(
            scroll(
                column(
                    message,
                    heading("Athlete"),
                    card(
                        labelled("Active athlete", athletes),
                        button("Make active") { setActive() },
                    ),
                    card(
                        eyebrow("New athlete", Palette.GOLD),
                        row(labelled("Id", newId), labelled("Display name", newName)),
                        consent,
                        button("Create athlete") { createAthlete() },
                    ),
                    heading("Training session"),
                    card(
                        labelled("Technique", technique),
                        row(labelled("Side", side), labelled("Sets", sets)),
                        row(labelled("Reps per set", reps), labelled("Rest, s", rest)),
                        primaryButton("Create session") { createSession() },
                    ),
                    card(
                        LinearLayout(this).apply {
                            gravity = Gravity.CENTER_VERTICAL
                            addView(title("Session control", 18f), LinearLayout.LayoutParams(0, -2, 1f))
                            addView(session)
                        },
                        button("I read the safety notes") { safety() },
                        primaryButton("Start") { command("start") },
                        row(
                            button("Pause") { command("pause") },
                            button("Resume") { command("resume") },
                            dangerButton("End") { command("end") },
                        ),
                    ),
                ),
            ),
            title = "Coach",
        )
        refresh()
    }

    private fun report(error: String?, ok: String? = null) {
        message.show(error ?: ok, tone = if (error != null) Palette.STRIKE else Palette.OK)
    }

    private fun refresh() {
        Engine.call(this, conn, "GET", "state") { data, error ->
            error?.let { report(it) }
            val state = data as? JSONObject ?: return@call
            val phase = state.optJSONObject("training")?.optString("phase")
            session.setPill(phase ?: "No session", if (phase == null) Palette.MUTED else Palette.OK)
        }
        Engine.call(this, conn, "GET", "athletes") { data, error ->
            error?.let { report(it) }
            athletes.setItems(Engine.list(data).map { it.optString("id") })
        }
        Engine.call(this, conn, "GET", "content/index") { data, error ->
            error?.let { report(it) }
            techniques = Engine.list((data as? JSONObject)?.optJSONArray("entries"))
                .filter { it.optString("kind") == "techniques" && it.optBoolean("valid") }
            val keep = technique.selectedItemPosition
            technique.setItems(techniques.map { it.optJSONObject("names")?.optString("display") ?: it.optString("id") })
            if (keep in techniques.indices) technique.setSelection(keep)
        }
    }

    private fun setActive() {
        val id = athletes.selected() ?: return report("Create an athlete first.")
        Engine.call(this, conn, "PUT", "athletes/active", JSONObject().put("athlete_id", id)) { _, e ->
            report(e, "$id is the active athlete.")
        }
    }

    private fun createAthlete() {
        val id = newId.text.toString().trim()
        val name = newName.text.toString().trim().ifEmpty { id }
        val body = JSONObject().put("id", id).put("display_name", name).put("height_m", 1.75).put("default_stance", "orthodox")
        Engine.call(this, conn, "POST", "athletes", body) { _, e ->
            if (e != null) return@call report(e)
            val done = { report(null, "Created $id."); refresh() }
            if (consent.isChecked) {
                Engine.call(this, conn, "POST", "athletes/$id/consent", JSONObject().put("motion_data", true)) { _, e2 ->
                    if (e2 != null) report(e2) else done()
                }
            } else {
                done()
            }
        }
    }

    private fun selectedTechnique(): JSONObject? = techniques.getOrNull(technique.selectedItemPosition)

    private fun createSession() {
        val t = selectedTechnique() ?: return report("No technique is available on the engine.")
        val plan = JSONObject()
            .put("kind", "technique")
            .put("technique", t.optString("id"))
            .put("side", side.selected())
            .put("sets", sets.text.toString().toIntOrNull() ?: 1)
            .put("reps", reps.text.toString().toIntOrNull() ?: 10)
            .put("rest_s", rest.text.toString().toIntOrNull() ?: 30)
        Engine.call(this, conn, "POST", "training/sessions", JSONObject().put("plan", plan)) { data, e ->
            val s = data as? JSONObject
            report(e, s?.let { "Session ${it.optString("session_id")} is ${it.optString("phase")}. Clear the space, then Start." })
            refresh()
        }
    }

    /** Acknowledges the safety notes of the session's own exercise, whatever the picker shows. */
    private fun safety() {
        Engine.call(this, conn, "GET", "training/state") { data, e ->
            val ref = (data as? JSONObject)?.optJSONObject("current")?.optJSONObject("exercise")?.optString("ref")
            if (e != null || ref.isNullOrEmpty()) return@call report(e ?: "Create a session first.")
            val args = JSONObject().put("content_refs", org.json.JSONArray().put(ref))
            Engine.call(this, conn, "POST", "training/command", JSONObject().put("command", "ack_safety").put("args", args)) { _, e2 ->
                report(e2, "Safety notes acknowledged for ${ref.substringBefore('@')}.")
            }
        }
    }

    private fun command(c: String) {
        Engine.call(this, conn, "POST", "training/command", JSONObject().put("command", c)) { data, e ->
            report(e, (data as? JSONObject)?.let { "Session ${it.optString("phase")}." })
            refresh()
        }
    }
}
