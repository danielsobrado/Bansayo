package app.bansayo.android

import android.Manifest
import android.content.pm.PackageManager
import android.graphics.ImageFormat
import android.graphics.Rect
import android.graphics.YuvImage
import android.hardware.camera2.CaptureRequest
import android.os.Bundle
import android.util.Range
import android.util.Size
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.camera2.interop.Camera2CameraControl
import androidx.camera.camera2.interop.CaptureRequestOptions
import androidx.camera.camera2.interop.ExperimentalCamera2Interop
import androidx.camera.core.Camera
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.core.AspectRatio
import androidx.camera.core.resolutionselector.AspectRatioStrategy
import androidx.camera.core.resolutionselector.ResolutionSelector
import androidx.camera.core.resolutionselector.ResolutionStrategy
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString.Companion.toByteString
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicReference

/**
 * The phone as one of the engine's cameras (ADR-042): frames go as JPEG, one per binary
 * WebSocket message, to `/ingest/camera/{id}` of a camera configured with `network: true`. Pose
 * runs on the PC. Landscape, 1280x720, at the engine's rate; frames that would queue are skipped.
 */
class CameraActivity : ComponentActivity() {
    private lateinit var conn: Connection
    private val preview by lazy { PreviewView(this) }
    private val status by lazy { label() }
    private val slot by lazy { spinner(emptyList()) }
    private val exposure by lazy { spinner(listOf("Automatic exposure", "1/250 s", "1/500 s", "1/1000 s")) }
    private val iso by lazy { field("ISO for manual exposure", "800", number = true) }
    private val socket = AtomicReference<WebSocket?>(null)
    private val worker = Executors.newSingleThreadExecutor()
    private var camera: Camera? = null
    @Volatile private var sent = 0L
    @Volatile private var skipped = 0L
    @Volatile private var lastSentNs = 0L
    @Volatile private var frameSize = ""
    @Volatile private var periodNs = 1_000_000_000L / 30
    /** The slot's declared `resolution_px`: the engine accepts frames of exactly this size. */
    private var wanted = Size(1280, 720)

    private val permission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted) startCamera() else status.text = "Camera permission is needed to send frames."
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        conn = Connection.load(this) ?: return finish()
        val controls = column(
            heading("Phone camera"),
            label("Engine camera slot"),
            slot,
            exposure,
            iso,
            row(
                button("Start sending") { connect() },
                button("Stop") { disconnect() },
            ),
            button("Apply exposure") { applyExposure() },
            status,
        )
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            addView(FrameLayout(context).apply { addView(preview) }, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 3f))
            addView(scroll(controls), LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 2f))
        }
        setContentView(root)
        loadSlots()
        tick()
    }

    private fun loadSlots() {
        Engine.call(this, conn, "GET", "state") { data, error ->
            val slots = Engine.networkCameras(data as? JSONObject)
            slot.setItems(slots.map { Engine.id(it) })
            slots.firstOrNull()?.let { d ->
                val descriptor = d.getJSONObject("descriptor")
                descriptor.optDouble("nominal_rate_hz", 30.0).takeIf { it > 0 }?.let {
                    periodNs = (1e9 / it).toLong()
                }
                descriptor.optJSONObject("camera")?.optJSONArray("resolution_px")?.let {
                    wanted = Size(it.getInt(0), it.getInt(1))
                }
            }
            if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
                startCamera()
            } else {
                permission.launch(Manifest.permission.CAMERA)
            }
            if (error != null) status.text = error
            else if (slots.isEmpty()) status.text =
                "The engine has no phone camera slot. Add to its configuration, under a camera source:\n" +
                    "  - { id: PHONE1, network: true, resolution_px: [1280, 720], rate_hz: 30 }\nthen restart it."
        }
    }

    private fun startCamera() {
        val future = ProcessCameraProvider.getInstance(this)
        future.addListener({
            val provider = future.get()
            val ratio = if (wanted.width * 3 == wanted.height * 4) AspectRatio.RATIO_4_3 else AspectRatio.RATIO_16_9
            val selector = ResolutionSelector.Builder()
                .setAspectRatioStrategy(AspectRatioStrategy(ratio, AspectRatioStrategy.FALLBACK_RULE_AUTO))
                .setResolutionStrategy(
                    ResolutionStrategy(wanted, ResolutionStrategy.FALLBACK_RULE_CLOSEST_HIGHER_THEN_LOWER),
                )
                .build()
            val shown = Preview.Builder().setResolutionSelector(selector).build()
            shown.surfaceProvider = preview.surfaceProvider
            val analysis = ImageAnalysis.Builder()
                .setResolutionSelector(selector)
                .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                .setOutputImageFormat(ImageAnalysis.OUTPUT_IMAGE_FORMAT_YUV_420_888)
                .build()
            analysis.setAnalyzer(worker) { image -> image.use { send(it) } }
            provider.unbindAll()
            camera = provider.bindToLifecycle(this, CameraSelector.DEFAULT_BACK_CAMERA, shown, analysis)
        }, mainExecutor)
    }

    private fun send(image: ImageProxy) {
        frameSize = "${image.width}x${image.height}"
        val ws = socket.get() ?: return
        val now = System.nanoTime()
        if (now - lastSentNs < periodNs * 9 / 10) return
        // A frame that would wait behind others is stale by the time it arrives: skip it (08 §3).
        if (ws.queueSize() > 1_500_000) {
            skipped++
            return
        }
        lastSentNs = now
        if (ws.send(jpeg(image).toByteString())) sent++ else skipped++
    }

    private fun connect() {
        val id = slot.selected() ?: run {
            status.text = "No phone camera slot on the engine."
            return
        }
        disconnect()
        socket.set(
            Engine.socket(conn, "ingest/camera/$id", object : WebSocketListener() {
                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    socket.compareAndSet(webSocket, null)
                    runOnUiThread {
                        status.text = if (response?.code == 401) "The engine refused the pairing token: pair again."
                        else "Connection lost: ${t.message ?: response?.code}"
                    }
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    socket.compareAndSet(webSocket, null)
                }
            }),
        )
    }

    private fun disconnect() {
        socket.getAndSet(null)?.close(1000, "stopped")
    }

    /** Short manual exposure through Camera2 (08 §3.2); automatic otherwise. */
    @androidx.annotation.OptIn(ExperimentalCamera2Interop::class)
    private fun applyExposure() {
        val cam = camera ?: return
        val ns = when (exposure.selectedItemPosition) {
            1 -> 4_000_000L
            2 -> 2_000_000L
            3 -> 1_000_000L
            else -> null
        }
        val options = CaptureRequestOptions.Builder()
        if (ns == null) {
            options.setCaptureRequestOption(CaptureRequest.CONTROL_AE_MODE, CaptureRequest.CONTROL_AE_MODE_ON)
            options.setCaptureRequestOption(CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, Range(30, 30))
        } else {
            options.setCaptureRequestOption(CaptureRequest.CONTROL_AE_MODE, CaptureRequest.CONTROL_AE_MODE_OFF)
            options.setCaptureRequestOption(CaptureRequest.SENSOR_EXPOSURE_TIME, ns)
            options.setCaptureRequestOption(CaptureRequest.SENSOR_SENSITIVITY, iso.text.toString().toIntOrNull() ?: 800)
            options.setCaptureRequestOption(CaptureRequest.SENSOR_FRAME_DURATION, 33_333_333L)
        }
        Camera2CameraControl.from(cam.cameraControl).setCaptureRequestOptions(options.build())
        status.text = "Exposure: ${exposure.selected()}"
    }

    private fun tick() {
        status.postDelayed({
            if (isFinishing) return@postDelayed
            val ws = socket.get()
            val warn = if (frameSize.isNotEmpty() && frameSize != "${wanted.width}x${wanted.height}")
                "\nThis camera gives $frameSize; set resolution_px: [${frameSize.replace("x", ", ")}] for the slot on the engine."
            else ""
            status.text = (if (ws == null) "Not sending." else "Sending to ${slot.selected()}.") +
                " Frames sent $sent, skipped $skipped, size $frameSize.$warn"
            tick()
        }, 1000)
    }

    override fun onDestroy() {
        disconnect()
        worker.shutdown()
        super.onDestroy()
    }

    companion object {
        /** YUV_420_888 → NV21 → JPEG at quality 80, respecting row and pixel strides. */
        fun jpeg(image: ImageProxy): ByteArray {
            val (w, h) = image.width to image.height
            val nv21 = ByteArray(w * h * 3 / 2)
            val y = image.planes[0]
            var o = 0
            val yb = y.buffer
            for (r in 0 until h) {
                if (y.pixelStride == 1) {
                    yb.position(r * y.rowStride)
                    yb.get(nv21, o, w)
                    o += w
                } else {
                    for (c in 0 until w) nv21[o++] = yb.get(r * y.rowStride + c * y.pixelStride)
                }
            }
            val u = image.planes[1]
            val v = image.planes[2]
            for (r in 0 until h / 2) {
                for (c in 0 until w / 2) {
                    nv21[o++] = v.buffer.get(r * v.rowStride + c * v.pixelStride)
                    nv21[o++] = u.buffer.get(r * u.rowStride + c * u.pixelStride)
                }
            }
            val out = ByteArrayOutputStream(w * h / 4)
            YuvImage(nv21, ImageFormat.NV21, w, h, null).compressToJpeg(Rect(0, 0, w, h), 80, out)
            return out.toByteArray()
        }
    }
}
