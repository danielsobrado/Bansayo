package app.bansayo.android

import android.app.Activity
import android.content.Context
import android.net.Uri
import okhttp3.Call
import okhttp3.Callback
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener
import java.io.IOException
import java.util.concurrent.TimeUnit

/** Where the engine is and the pairing token (ADR-042). Kept in the app's private preferences. */
data class Connection(val host: String, val port: Int, val token: String) {
    val base get() = "http://$host:$port/api/v1/"
    val wsBase get() = "ws://$host:$port/api/v1/"

    companion object {
        private const val PREFS = "engine"

        fun load(ctx: Context): Connection? {
            val p = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            val host = p.getString("host", null) ?: return null
            return Connection(host, p.getInt("port", 7700), p.getString("token", "") ?: "")
        }

        fun save(ctx: Context, c: Connection) {
            ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
                .putString("host", c.host).putInt("port", c.port).putString("token", c.token).apply()
        }

        /** `bansayo://pair?host=192.168.1.20&port=7700&token=…`, as the desktop shows it. */
        fun fromPairingUri(uri: Uri): Connection? {
            if (uri.scheme != "bansayo" || uri.host != "pair") return null
            val host = uri.getQueryParameter("host") ?: return null
            val port = uri.getQueryParameter("port")?.toIntOrNull() ?: 7700
            val token = uri.getQueryParameter("token") ?: return null
            return Connection(host, port, token)
        }
    }
}

/** HTTP and WebSocket access to the engine's API with the bearer token of LAN mode. */
object Engine {
    val client: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(5, TimeUnit.SECONDS)
        .readTimeout(10, TimeUnit.SECONDS)
        .pingInterval(10, TimeUnit.SECONDS)
        .build()

    private val JSON = "application/json".toMediaType()
    const val UNREACHABLE = "Cannot reach the engine"
    const val REFUSED = "The engine refused the pairing token: pair again"

    /** Whether [error] is about the connection itself, so the fix is in Settings. */
    fun isConnectivity(error: String?) = error != null && (error.startsWith(UNREACHABLE) || error.startsWith(REFUSED))

    fun request(c: Connection, method: String, path: String, body: JSONObject? = null): Request {
        val b = Request.Builder().url(c.base + path).header("Authorization", "Bearer ${c.token}")
        return when (method) {
            "GET" -> b.get()
            else -> b.method(method, (body ?: JSONObject()).toString().toRequestBody(JSON))
        }.build()
    }

    /**
     * Calls the API and delivers `data` (a JSONObject, JSONArray or null) or an error message on
     * the activity's UI thread.
     */
    fun call(
        a: Activity,
        c: Connection,
        method: String,
        path: String,
        body: JSONObject? = null,
        done: (data: Any?, error: String?) -> Unit,
    ) {
        client.newCall(request(c, method, path, body)).enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) {
                a.runOnUiThread { done(null, "$UNREACHABLE at ${c.host}:${c.port}: ${e.message}") }
            }

            override fun onResponse(call: Call, response: Response) {
                val text = response.body?.string().orEmpty()
                val parsed = runCatching { JSONTokener(text).nextValue() }.getOrNull()
                val (data, error) = when {
                    response.code == 401 -> null to REFUSED
                    parsed is JSONObject && parsed.has("error") && !parsed.isNull("error") ->
                        null to (parsed.getJSONObject("error").optString("message", "Error ${response.code}"))
                    parsed is JSONObject -> parsed.opt("data") to null
                    response.isSuccessful -> text to null
                    else -> null to "HTTP ${response.code}"
                }
                a.runOnUiThread { done(if (data == JSONObject.NULL) null else data, error) }
            }
        })
    }

    fun socket(c: Connection, path: String, listener: WebSocketListener): WebSocket =
        client.newWebSocket(
            Request.Builder().url(c.wsBase + path).header("Authorization", "Bearer ${c.token}").build(),
            listener,
        )

    /** Device statuses from `/state`. */
    fun devices(state: JSONObject?): List<JSONObject> {
        val arr = state?.optJSONArray("devices") ?: return emptyList()
        return (0 until arr.length()).map { arr.getJSONObject(it) }
    }

    fun cameras(state: JSONObject?) = devices(state).filter {
        it.getJSONObject("descriptor").optString("kind") == "camera"
    }

    /** Cameras the engine expects frames from over the network (`network: true`). */
    fun networkCameras(state: JSONObject?) = cameras(state).filter {
        it.getJSONObject("descriptor").optString("transport") == "websocket"
    }

    fun id(device: JSONObject): String = device.getJSONObject("descriptor").getString("id")

    fun list(a: Any?): List<JSONObject> =
        (a as? JSONArray)?.let { arr -> (0 until arr.length()).map { arr.getJSONObject(it) } } ?: emptyList()
}
