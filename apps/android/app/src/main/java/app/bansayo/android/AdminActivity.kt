package app.bansayo.android

import android.os.Bundle
import android.widget.CheckBox
import androidx.activity.ComponentActivity
import org.json.JSONObject

/**
 * Administration over the engine's existing API (ADR-042): device status, athletes with
 * motion-data consent, a training session's plan and commands, and USB camera exposure.
 */
class AdminActivity : ComponentActivity() {
    private lateinit var conn: Connection
    private val devices by lazy { label() }
    private val athletes by lazy { spinner(emptyList()) }
    private val newId by lazy { field("New athlete id, e.g. p1") }
    private val newName by lazy { field("Display name (a pseudonym is fine)") }
    private val consent by lazy { CheckBox(this).apply { text = "Motion-data consent recorded (sessions are saved)" } }
    private val technique by lazy { spinner(emptyList()) }
    private val side by lazy { spinner(listOf("right", "left")) }
    private val sets by lazy { field("Sets", "3", number = true) }
    private val reps by lazy { field("Repetitions per set", "10", number = true) }
    private val rest by lazy { field("Rest between sets, s", "30", number = true) }
    private val session by lazy { label() }
    private val camera by lazy { spinner(emptyList()) }
    private val exposure by lazy { field("Exposure, s (e.g. 0.004)", number = true) }
    private val gain by lazy { field("Gain (camera units, optional)", number = true) }
    private val message by lazy { label() }
    private var techniques: List<JSONObject> = emptyList()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        conn = Connection.load(this) ?: return finish()
        setContentView(
            scroll(
                column(
                    message,
                    heading("Devices"),
                    devices,
                    button("Refresh") { refresh() },
                    heading("Athlete"),
                    athletes,
                    button("Make active") { setActive() },
                    newId, newName, consent,
                    button("Create athlete") { createAthlete() },
                    heading("Training session"),
                    technique, side, sets, reps, rest,
                    button("Create session") { createSession() },
                    row(
                        button("I read the safety notes") { safety() },
                        button("Start") { command("start") },
                    ),
                    row(
                        button("Pause") { command("pause") },
                        button("Resume") { command("resume") },
                        button("End") { command("end") },
                    ),
                    session,
                    heading("USB camera exposure"),
                    camera, exposure, gain,
                    button("Apply camera settings") { cameraSettings() },
                ),
            ),
        )
        refresh()
    }

    private fun report(error: String?, ok: String? = null) {
        message.text = error ?: ok ?: ""
    }

    private fun refresh() {
        Engine.call(this, conn, "GET", "state") { data, error ->
            report(error)
            val state = data as? JSONObject ?: return@call
            devices.text = Engine.devices(state).joinToString("\n") { d ->
                val stream = d.optJSONObject("stream")
                val reasons = d.optJSONArray("reasons")?.let { r ->
                    (0 until r.length()).joinToString("; ") { r.getJSONObject(it).optString("message") }
                }.orEmpty()
                val settings = d.optJSONObject("camera_settings")?.let { s ->
                    if (s.isNull("exposure_s")) "" else " · exposure %.4f s".format(s.optDouble("exposure_s"))
                }.orEmpty()
                "${Engine.id(d)}: ${d.optString("state")}, %.1f Hz".format(stream?.optDouble("measured_rate_hz") ?: 0.0) +
                    settings + (if (reasons.isEmpty()) "" else "\n   $reasons")
            }.ifEmpty { "No devices." }
            camera.setItems(
                Engine.cameras(state).filter { it.getJSONObject("descriptor").optString("transport") == "usb" }.map { Engine.id(it) },
            )
            session.text = state.optJSONObject("training")?.let { "Session ${it.optString("session_id")}: ${it.optString("phase")}" }
                ?: "No session."
        }
        Engine.call(this, conn, "GET", "athletes") { data, error ->
            report(error)
            athletes.setItems(Engine.list(data).map { it.optString("id") })
        }
        Engine.call(this, conn, "GET", "content/index") { data, error ->
            report(error)
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

    private fun cameraSettings() {
        val id = camera.selected() ?: return report("No USB camera on the engine.")
        val body = JSONObject()
        exposure.text.toString().toDoubleOrNull()?.let { body.put("exposure_s", it) }
        gain.text.toString().toDoubleOrNull()?.let { body.put("gain", it) }
        Engine.call(this, conn, "PUT", "cameras/$id/settings", body) { data, e ->
            val s = (data as? JSONObject)?.optJSONObject("camera_settings")
            report(e, s?.let { "In effect: exposure ${it.opt("exposure_s")} s, gain ${it.opt("gain")}" })
        }
    }
}
