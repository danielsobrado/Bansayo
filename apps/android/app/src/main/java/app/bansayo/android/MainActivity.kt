package app.bansayo.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import org.json.JSONObject

/**
 * Pairing and connection (ADR-042). Opened from the desktop's pairing QR code
 * (`bansayo://pair?host=&port=&token=`) or filled in by hand; then the three screens.
 */
class MainActivity : ComponentActivity() {
    private val host by lazy { field("PC address, e.g. 192.168.1.20") }
    private val port by lazy { field("Port", "7700", number = true) }
    private val token by lazy { field("Pairing token") }
    private val status by lazy { label() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Connection.load(this)?.let { show(it) }
        setContentView(
            scroll(
                column(
                    heading("Bansayo"),
                    label(
                        "Pair with the engine on your PC: open Devices → Phone pairing on the desktop and " +
                            "scan its QR code with the phone's camera, or type the three values. " +
                            "Phone and PC must be on the same Wi-Fi. The link has no encryption: " +
                            "use a trusted home network only.",
                    ),
                    host, port, token,
                    button("Connect") { connect() },
                    status,
                    heading("Use the phone as"),
                    button("Camera — send this camera to the engine") { open(CameraActivity::class.java) },
                    button("Viewer — live pose, counts and cues") { open(ViewerActivity::class.java) },
                    button("Admin — athletes, sessions, cameras") { open(AdminActivity::class.java) },
                ),
            ),
        )
        handle(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    private fun handle(intent: Intent?) {
        val c = intent?.data?.let { Connection.fromPairingUri(it) } ?: return
        show(c)
        connect()
    }

    private fun show(c: Connection) {
        host.setText(c.host)
        port.setText(c.port.toString())
        token.setText(c.token)
    }

    private fun current(): Connection? {
        val h = host.text.toString().trim()
        val p = port.text.toString().trim().toIntOrNull()
        if (h.isEmpty() || p == null) {
            status.text = "Enter the PC address and port."
            return null
        }
        return Connection(h, p, token.text.toString().trim())
    }

    private fun connect() {
        val c = current() ?: return
        Connection.save(this, c)
        status.text = "Connecting…"
        Engine.call(this, c, "GET", "state") { data, error ->
            if (error != null) {
                status.text = error
                return@call
            }
            val state = data as? JSONObject
            val engine = state?.optJSONObject("engine")
            val cams = Engine.cameras(state).joinToString { Engine.id(it) + " (" + it.optString("state") + ")" }
            val phones = Engine.networkCameras(state).joinToString { Engine.id(it) }
            status.text = buildString {
                append("Connected. Engine ").append(engine?.optString("state") ?: "?").append(".\n")
                append("Cameras: ").append(cams.ifEmpty { "none" }).append(".\n")
                append(
                    if (phones.isEmpty()) "No phone camera is configured on the engine (network: true)."
                    else "Phone camera slots: $phones.",
                )
            }
        }
    }

    private fun open(cls: Class<*>) {
        if (current()?.also { Connection.save(this, it) } == null) return
        startActivity(Intent(this, cls))
    }
}
