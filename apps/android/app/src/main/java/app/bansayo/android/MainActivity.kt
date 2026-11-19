package app.bansayo.android

import android.content.Intent
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.ComponentActivity
import org.json.JSONObject

/**
 * Home (ADR-042): a hero pose, the engine's connection state, and the four ways to use the phone.
 * Connectivity is configured only in [SettingsActivity]; this screen links there when it is
 * missing or failing.
 */
class MainActivity : ComponentActivity() {
    private val state by lazy { pill("Checking", Palette.GOLD) }
    private val summary by lazy { label().apply { setTextIsSelectable(false) } }
    private val notice by lazy { Notice(this) }
    private var check = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Pairing links used to open this screen; Settings owns them now.
        if (intent?.data != null) startActivity(Intent(this, SettingsActivity::class.java).setData(intent.data))
        screen(
            scroll(
                column(
                    header(),
                    hero(),
                    notice,
                    heading("Choose a mode"),
                    row(
                        tile(R.drawable.ic_camera, "Camera", "Stream this camera to the pose engine", Palette.ACCENT, CameraActivity::class.java),
                        tile(R.drawable.ic_viewer, "Live view", "Skeleton, reps, phases and cues", Palette.STRIKE, ViewerActivity::class.java),
                    ),
                    row(
                        tile(R.drawable.ic_train, "Train", "Sessions, history and review", Palette.GOLD, TrainingHistoryActivity::class.java),
                        tile(R.drawable.ic_admin, "Coach", "Athletes, plans and session control", Palette.ACCENT_DEEP, AdminActivity::class.java),
                    ),
                ),
            ),
        )
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        if (intent.data != null) startActivity(Intent(this, SettingsActivity::class.java).setData(intent.data))
    }

    override fun onResume() {
        super.onResume()
        refresh()
    }

    private fun header() = LinearLayout(this).apply {
        gravity = Gravity.CENTER_VERTICAL
        addView(
            column(
                eyebrow("Martial arts · motion AI"),
                TextView(context).apply {
                    text = "BANSAYO"
                    textSize = 32f
                    letterSpacing = 0.16f
                    typeface = Fonts.display
                    setTextColor(Palette.TEXT)
                },
                padding = 0, gap = 2,
            ),
            LinearLayout.LayoutParams(0, -2, 1f),
        )
        addView(iconButton(R.drawable.ic_settings, "Connection settings") { openSettings() })
    }

    private fun hero(): View {
        val pose = SkeletonView(this).apply {
            setLandmarks(SkeletonView.SIDE_KICK, 560 to 460, "coco17.body")
            strikeBones = SkeletonView.KICK_BONES
        }
        val shade = View(this).apply {
            background = GradientDrawable(
                GradientDrawable.Orientation.TOP_BOTTOM,
                intArrayOf(0, 0, Palette.alpha(Palette.BG, 235)),
            )
        }
        val text = column(
            TextView(this).apply {
                text = "Every strike, tracked."
                textSize = 24f
                typeface = Fonts.display
                setTextColor(Palette.TEXT)
            },
            LinearLayout(this).apply {
                gravity = Gravity.CENTER_VERTICAL
                addView(state)
                addView(summary.apply { setPadding(dp(10), 0, 0, 0); textSize = 13f })
            },
            padding = 18, gap = 8,
        )
        return FrameLayout(this).apply {
            background = rounded(Palette.SURFACE, 24, Palette.STROKE)
            addView(pose, FrameLayout.LayoutParams(-1, dp(200)))
            addView(shade, FrameLayout.LayoutParams(-1, -1))
            addView(text, FrameLayout.LayoutParams(-1, -2, Gravity.BOTTOM))
            layoutParams = LinearLayout.LayoutParams(-1, dp(280))
            post { roundCorners(dp(24).toFloat()) }
        }
    }

    private fun tile(icon: Int, name: String, about: String, color: Int, target: Class<*>) =
        column(
            FrameLayout(this).apply {
                background = rounded(Palette.alpha(color, 36), 14)
                addView(iconView(icon, color), FrameLayout.LayoutParams(dp(24), dp(24), Gravity.CENTER))
                layoutParams = LinearLayout.LayoutParams(dp(44), dp(44))
            },
            title(name, 18f),
            TextView(this).apply {
                text = about
                textSize = 13f
                setTextColor(Palette.MUTED)
                minLines = 2
            },
            padding = 16, gap = 10,
        ).apply {
            background = android.graphics.drawable.RippleDrawable(
                android.content.res.ColorStateList.valueOf(Palette.alpha(color, 70)),
                rounded(Palette.SURFACE, 20, Palette.STROKE),
                null,
            )
            isClickable = true
            contentDescription = "$name: $about"
            setOnClickListener {
                if (Connection.load(context) == null) {
                    notice.show("Connect to your engine first to use $name.", settings = true, tone = Palette.GOLD)
                } else {
                    startActivity(Intent(context, target))
                }
            }
        }

    private fun refresh() {
        val c = Connection.load(this)
        if (c == null) {
            state.setPill("Not paired", Palette.GOLD)
            summary.text = ""
            notice.show("Pair the phone with the Bansayo engine on your PC to start.", settings = true, tone = Palette.GOLD)
            return
        }
        val request = ++check
        state.setPill("Checking", Palette.GOLD)
        Engine.call(this, c, "GET", "state") { data, error ->
            if (request != check || isFinishing) return@call
            if (error != null) {
                state.setPill("Offline", Palette.STRIKE)
                summary.text = "${c.host}:${c.port}"
                notice.show(error, settings = true)
                return@call
            }
            val s = data as? JSONObject
            val cams = Engine.cameras(s).size
            state.setPill("Engine online", Palette.OK)
            summary.text = "$cams camera${if (cams == 1) "" else "s"} · ${s?.optJSONObject("athlete")?.optString("display_name") ?: "no athlete"}"
            notice.show(null)
        }
    }
}
