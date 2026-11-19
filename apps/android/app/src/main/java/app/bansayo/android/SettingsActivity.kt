package app.bansayo.android

import android.content.Intent
import android.os.Bundle
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import org.json.JSONObject

/**
 * Everything about connectivity, in one place (ADR-042): pairing with the engine (QR link
 * `bansayo://pair?host=&port=&token=` or typed), the connection test, the engine's devices and
 * USB camera exposure. Other screens link here when the connection is missing or failing.
 */
class SettingsActivity : ComponentActivity() {
    private val host by lazy { field("e.g. 192.168.1.20") }
    private val port by lazy { field("7700", "7700", number = true) }
    private val token by lazy { field("Pairing token") }
    private val state by lazy { pill("Not configured", Palette.MUTED) }
    private val status by lazy { label() }
    private val notice by lazy { Notice(this) }
    private val devices by lazy { LinearLayout(this).apply { orientation = LinearLayout.VERTICAL } }
    private val camera by lazy { spinner(emptyList()) }
    private val exposure by lazy { field("e.g. 0.004", number = true) }
    private val gain by lazy { field("Optional", number = true) }
    private val cameraMessage by lazy { Notice(this) }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        screen(
            scroll(
                column(
                    heading("Engine connection"),
                    card(
                        LinearLayout(this).apply {
                            gravity = android.view.Gravity.CENTER_VERTICAL
                            addView(title("Pair with your PC", 18f), LinearLayout.LayoutParams(0, -2, 1f))
                            addView(state)
                        },
                        label(
                            "On the desktop open Devices → Phone pairing → Show pairing code, and scan the QR " +
                                "code with the phone's camera app. Or type the three values below.",
                        ),
                        labelled("PC address", host),
                        row(labelled("Port", port), labelled("Token", token)),
                        primaryButton("Test and save connection") { connect() },
                        status,
                    ),
                    notice,
                    card(
                        eyebrow("Network", Palette.GOLD),
                        label(
                            "Phone and PC must be on the same Wi-Fi. The link is not encrypted: use a trusted " +
                                "home network only.",
                        ),
                        padding = 16, gap = 6,
                    ),
                    heading("Engine devices"),
                    devices,
                    button("Refresh devices") { refreshDevices() },
                    heading("USB camera exposure"),
                    card(
                        labelled("Camera", camera),
                        row(labelled("Exposure, s", exposure), labelled("Gain", gain)),
                        button("Apply camera settings") { cameraSettings() },
                        cameraMessage,
                    ),
                ),
            ),
            title = "Settings",
            settingsAction = false,
        )
        status.visibility = android.view.View.GONE
        showDevices(null)
        Connection.load(this)?.let { show(it) }
        if (!handle(intent)) Connection.load(this)?.let { connect() }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    /** A pairing link from the desktop's QR code: fill in, save and test. */
    private fun handle(intent: Intent?): Boolean {
        val c = intent?.data?.let { Connection.fromPairingUri(it) } ?: return false
        show(c)
        connect()
        return true
    }

    private fun show(c: Connection) {
        host.setText(c.host)
        port.setText(c.port.toString())
        token.setText(c.token)
    }

    private fun connect() {
        val h = host.text.toString().trim()
        val p = port.text.toString().trim().toIntOrNull()
        if (h.isEmpty() || p == null) {
            notice.show("Enter the PC address and port.", settings = false, tone = Palette.GOLD)
            return
        }
        val c = Connection(h, p, token.text.toString().trim())
        Connection.save(this, c)
        state.setPill("Connecting", Palette.GOLD)
        notice.show(null)
        status.text = ""
        Engine.call(this, c, "GET", "state") { data, error ->
            if (error != null) {
                state.setPill("Offline", Palette.STRIKE)
                notice.show(error, settings = false, tone = Palette.STRIKE)
                showDevices(null)
                return@call
            }
            val s = data as? JSONObject
            state.setPill("Connected", Palette.OK)
            val phones = Engine.networkCameras(s).joinToString { Engine.id(it) }
            status.text = buildString {
                append("Engine ").append(s?.optJSONObject("engine")?.optString("state") ?: "?").append(" at $h:$p.\n")
                append(
                    if (phones.isEmpty()) "No phone camera slot is configured on the engine (network: true)."
                    else "Phone camera slots: $phones.",
                )
            }
            status.visibility = android.view.View.VISIBLE
            showDevices(s)
        }
    }

    private fun refreshDevices() {
        val c = Connection.load(this) ?: return notice.show("Save a connection first.", settings = false, tone = Palette.GOLD)
        Engine.call(this, c, "GET", "state") { data, error ->
            notice.show(error, settings = false, tone = Palette.STRIKE)
            showDevices(data as? JSONObject)
        }
    }

    private fun showDevices(state: JSONObject?) {
        devices.removeAllViews()
        val list = Engine.devices(state)
        if (list.isEmpty()) devices.add(label(if (state == null) "Connect to see the engine's devices." else "No devices."), 0)
        list.forEachIndexed { i, d -> devices.add(deviceRow(d), if (i == 0) 0 else 8) }
        camera.setItems(
            Engine.cameras(state).filter { it.getJSONObject("descriptor").optString("transport") == "usb" }.map { Engine.id(it) },
        )
    }

    private fun deviceRow(d: JSONObject): LinearLayout {
        val st = d.optString("state")
        val color = when (st) {
            "tracking", "streaming", "ready" -> Palette.OK
            "error", "lost", "offline" -> Palette.STRIKE
            else -> Palette.GOLD
        }
        val hz = d.optJSONObject("stream")?.optDouble("measured_rate_hz") ?: 0.0
        val exposureText = d.optJSONObject("camera_settings")?.let { s ->
            if (s.isNull("exposure_s")) "" else " · exposure %.4f s".format(s.optDouble("exposure_s"))
        }.orEmpty()
        val reasons = d.optJSONArray("reasons")?.let { r ->
            (0 until r.length()).joinToString("; ") { r.getJSONObject(it).optString("message") }
        }.orEmpty()
        val head = LinearLayout(this).apply {
            addView(title(Engine.id(d), 17f), LinearLayout.LayoutParams(0, -2, 1f))
            addView(pill(st.ifEmpty { "unknown" }, color))
        }
        val meta = TextView(this).apply {
            text = "%.1f Hz".format(hz) + exposureText
            typeface = Fonts.mono
            textSize = 13f
            setTextColor(Palette.MUTED)
        }
        return card(head, meta, padding = 14, gap = 6).apply {
            if (reasons.isNotEmpty()) add(label(reasons), 4)
        }
    }

    private fun cameraSettings() {
        val c = Connection.load(this) ?: return cameraMessage.show("Save a connection first.", settings = false)
        val id = camera.selected() ?: return cameraMessage.show("No USB camera on the engine.", settings = false, tone = Palette.GOLD)
        val body = JSONObject()
        exposure.text.toString().toDoubleOrNull()?.let { body.put("exposure_s", it) }
        gain.text.toString().toDoubleOrNull()?.let { body.put("gain", it) }
        Engine.call(this, c, "PUT", "cameras/$id/settings", body) { data, e ->
            val s = (data as? JSONObject)?.optJSONObject("camera_settings")
            cameraMessage.show(
                e ?: s?.let { "In effect: exposure ${it.opt("exposure_s")} s, gain ${it.opt("gain")}" },
                settings = false,
                tone = if (e != null) Palette.STRIKE else Palette.OK,
            )
            refreshDevices()
        }
    }
}
