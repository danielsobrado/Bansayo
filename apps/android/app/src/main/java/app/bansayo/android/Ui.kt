package app.bansayo.android

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.Outline
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.LayerDrawable
import android.graphics.drawable.RippleDrawable
import android.os.Build
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.ViewOutlineProvider
import android.view.WindowInsets
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Spinner
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge

/**
 * The app's design system and small builders for programmatic layouts (no layout XML).
 * Look: a dark training hall lit by pose-tracking neon. Cyan is the AI / tracking color,
 * crimson marks strikes and attention, gold marks joints and grades.
 */
object Palette {
    val BG = Color.rgb(7, 8, 15)
    val SURFACE = Color.rgb(17, 19, 31)
    val SURFACE_HI = Color.rgb(26, 29, 46)
    val STROKE = Color.rgb(40, 44, 68)
    val ACCENT = Color.rgb(34, 230, 255)
    val ACCENT_DEEP = Color.rgb(64, 120, 255)
    val STRIKE = Color.rgb(255, 61, 110)
    val GOLD = Color.rgb(255, 194, 74)
    val OK = Color.rgb(61, 245, 160)
    val TEXT = Color.rgb(238, 241, 255)
    val MUTED = Color.rgb(140, 147, 181)

    fun alpha(color: Int, a: Int) = Color.argb(a, Color.red(color), Color.green(color), Color.blue(color))
}

object Fonts {
    val display: Typeface = Typeface.create("sans-serif-condensed", Typeface.BOLD)
    val medium: Typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
    val mono: Typeface = Typeface.create(Typeface.MONOSPACE, Typeface.BOLD)
}

fun Context.dp(v: Int) = (v * resources.displayMetrics.density).toInt()

fun Context.rounded(fill: Int, radius: Int = 16, stroke: Int? = null) = GradientDrawable().apply {
    setColor(fill)
    cornerRadius = dp(radius).toFloat()
    stroke?.let { setStroke(dp(1), it) }
}

private fun Context.pressable(shape: Drawable, ripple: Int = Palette.alpha(Palette.ACCENT, 60)) =
    RippleDrawable(ColorStateList.valueOf(ripple), shape, null)

private fun ViewGroup.MarginLayoutParams.gap(c: Context, top: Int) = apply { topMargin = c.dp(top) }

/** Vertical stack; children get breathing room between them. */
fun Context.column(vararg children: View, padding: Int = 20, gap: Int = 10): LinearLayout =
    LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(dp(padding), dp(padding), dp(padding), dp(padding))
        children.forEachIndexed { i, v -> add(v, if (i == 0) 0 else gap) }
    }

/** Adds [v] full-width (unless it already has a size) with [gap] dp above it. */
fun LinearLayout.add(v: View, gap: Int = 10) = addView(
    v,
    LinearLayout.LayoutParams(
        v.layoutParams?.width ?: ViewGroup.LayoutParams.MATCH_PARENT,
        v.layoutParams?.height ?: ViewGroup.LayoutParams.WRAP_CONTENT,
    ).gap(context, gap),
)

/** Equal-width horizontal row. */
fun Context.row(vararg children: View, gap: Int = 10): LinearLayout = LinearLayout(this).apply {
    orientation = LinearLayout.HORIZONTAL
    children.forEachIndexed { i, v ->
        addView(v, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply {
            if (i > 0) marginStart = dp(gap)
        })
    }
}

fun Context.scroll(content: View): ScrollView = ScrollView(this).apply {
    isFillViewport = true
    isVerticalScrollBarEnabled = false
    addView(content)
}

/** Section title: condensed, upper-case, with a short cyan rule. */
fun Context.heading(text: String) = TextView(this).apply {
    this.text = text.uppercase()
    textSize = 15f
    letterSpacing = 0.12f
    typeface = Fonts.display
    setTextColor(Palette.TEXT)
    setPadding(dp(12), dp(18), 0, dp(2))
    background = LayerDrawable(arrayOf(rounded(Palette.ACCENT, 2))).apply {
        setLayerSize(0, dp(3), dp(16))
        setLayerGravity(0, Gravity.START or Gravity.BOTTOM)
        setLayerInsetBottom(0, dp(4))
    }
}

/** Small letter-spaced caption above a value or block. */
fun Context.eyebrow(text: String, color: Int = Palette.ACCENT) = TextView(this).apply {
    this.text = text.uppercase()
    textSize = 11f
    letterSpacing = 0.2f
    typeface = Fonts.medium
    setTextColor(color)
}

fun Context.label(text: String = "") = TextView(this).apply {
    this.text = text
    textSize = 15f
    setLineSpacing(0f, 1.2f)
    setTextColor(Palette.MUTED)
    setTextIsSelectable(true)
}

fun Context.title(text: String, size: Float = 20f) = TextView(this).apply {
    this.text = text
    textSize = size
    typeface = Fonts.display
    setTextColor(Palette.TEXT)
}

/** Secondary action: outlined surface. */
fun Context.button(text: String, onClick: () -> Unit) = Button(this).apply {
    this.text = text
    isAllCaps = false
    textSize = 15f
    typeface = Fonts.medium
    setTextColor(Palette.TEXT)
    stateListAnimator = null
    minHeight = dp(48)
    setPadding(dp(16), dp(10), dp(16), dp(10))
    background = pressable(rounded(Palette.SURFACE_HI, 14, Palette.STROKE))
    setOnClickListener { onClick() }
}

/** The one main action of a screen or card: neon cyan → blue gradient. */
fun Context.primaryButton(text: String, onClick: () -> Unit) = button(text, onClick).apply {
    setTextColor(Palette.BG)
    typeface = Fonts.display
    letterSpacing = 0.04f
    background = pressable(
        GradientDrawable(GradientDrawable.Orientation.LEFT_RIGHT, intArrayOf(Palette.ACCENT, Palette.ACCENT_DEEP))
            .apply { cornerRadius = dp(14).toFloat() },
        Color.argb(80, 255, 255, 255),
    )
}

/** Destructive or "stop" action. */
fun Context.dangerButton(text: String, onClick: () -> Unit) = button(text, onClick).apply {
    setTextColor(Palette.STRIKE)
    background = pressable(rounded(Palette.alpha(Palette.STRIKE, 24), 14, Palette.alpha(Palette.STRIKE, 140)))
}

fun Context.field(hint: String, value: String = "", number: Boolean = false) = EditText(this).apply {
    this.hint = hint
    setText(value)
    isSingleLine = true
    textSize = 15f
    setTextColor(Palette.TEXT)
    setHintTextColor(Palette.alpha(Palette.MUTED, 150))
    minHeight = dp(50)
    setPadding(dp(16), dp(12), dp(16), dp(12))
    background = rounded(Palette.SURFACE_HI, 14, Palette.STROKE)
    inputType = if (number) InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL
    else InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
}

/** A labelled input: caption above the field. */
fun Context.labelled(caption: String, input: View) = column(eyebrow(caption, Palette.MUTED), input, padding = 0, gap = 6)

fun Context.spinner(items: List<String>) = Spinner(this).apply {
    minimumHeight = dp(50)
    background = LayerDrawable(arrayOf(rounded(Palette.SURFACE_HI, 14, Palette.STROKE), icon(R.drawable.ic_expand, Palette.ACCENT))).apply {
        setLayerSize(1, dp(22), dp(22))
        setLayerGravity(1, Gravity.END or Gravity.CENTER_VERTICAL)
        setLayerInsetEnd(1, dp(14))
    }
    setPadding(dp(6), 0, dp(40), 0)
    setPopupBackgroundDrawable(rounded(Palette.SURFACE_HI, 14, Palette.STROKE))
    setItems(items)
}

fun Spinner.setItems(items: List<String>) {
    adapter = ArrayAdapter(context, android.R.layout.simple_spinner_dropdown_item, items)
}

fun Spinner.selected(): String? = selectedItem as? String

fun Context.icon(res: Int, tint: Int): Drawable = getDrawable(res)!!.mutate().apply { setTint(tint) }

fun Context.iconView(res: Int, tint: Int, size: Int = 24) = ImageView(this).apply {
    setImageDrawable(icon(res, tint))
    layoutParams = LinearLayout.LayoutParams(dp(size), dp(size))
}

/** A raised panel. */
fun Context.card(vararg children: View, padding: Int = 18, gap: Int = 12): LinearLayout =
    column(*children, padding = padding, gap = gap).apply {
        background = rounded(Palette.SURFACE, 20, Palette.STROKE)
    }

/** Small status chip with a dot. */
fun Context.pill(text: String, color: Int) = TextView(this).apply {
    setPill(text, color)
    textSize = 12f
    typeface = Fonts.medium
    letterSpacing = 0.06f
    gravity = Gravity.CENTER_VERTICAL
    setPadding(dp(10), dp(5), dp(12), dp(5))
}

fun TextView.setPill(text: String, color: Int) {
    this.text = "●  " + text.uppercase()
    setTextColor(color)
    background = context.rounded(Palette.alpha(color, 30), 50, Palette.alpha(color, 110))
}

/** Clips a view to rounded corners (camera previews, skeleton view). */
fun View.roundCorners(radiusPx: Float) {
    outlineProvider = object : ViewOutlineProvider() {
        override fun getOutline(view: View, outline: Outline) = outline.setRoundRect(0, 0, view.width, view.height, radiusPx)
    }
    clipToOutline = true
}

fun Context.openSettings() = startActivity(Intent(this, SettingsActivity::class.java))

/**
 * A message banner. Connectivity problems get an "Open settings" link, so every screen points to
 * the one place where the connection is configured.
 */
class Notice(context: Context) : LinearLayout(context) {
    private val message = context.label().apply { setTextColor(Palette.TEXT) }
    private val link = TextView(context).apply {
        text = "Open connection settings  →"
        textSize = 14f
        typeface = Fonts.medium
        setTextColor(Palette.ACCENT)
        setPadding(0, context.dp(8), 0, 0)
        setOnClickListener { context.openSettings() }
    }

    init {
        orientation = VERTICAL
        setPadding(context.dp(16), context.dp(14), context.dp(16), context.dp(14))
        addView(message)
        addView(link)
        visibility = GONE
    }

    /** Shows [text]; a null or empty text hides the banner. */
    fun show(text: String?, settings: Boolean = Engine.isConnectivity(text), tone: Int = if (settings) Palette.STRIKE else Palette.ACCENT) {
        visibility = if (text.isNullOrEmpty()) GONE else VISIBLE
        message.text = text
        link.visibility = if (settings) VISIBLE else GONE
        background = LayerDrawable(arrayOf(context.rounded(Palette.alpha(tone, 26), 16, Palette.alpha(tone, 90)), context.rounded(tone, 2))).apply {
            setLayerWidth(1, context.dp(3))
            setLayerGravity(1, Gravity.START or Gravity.FILL_VERTICAL)
            setLayerInset(1, context.dp(1), context.dp(12), 0, context.dp(12))
        }
    }
}

/**
 * Sets the screen: dark edge-to-edge window, a top bar (back + title + optional settings) and
 * [content] padded clear of the system bars and keyboard.
 */
fun ComponentActivity.screen(content: View, title: String? = null, settingsAction: Boolean = true) {
    enableEdgeToEdge(SystemBarStyle.dark(Color.TRANSPARENT), SystemBarStyle.dark(Color.TRANSPARENT))
    val root = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setBackgroundColor(Palette.BG)
        if (title != null) addView(topBar(title, settingsAction))
        addView(content, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        setOnApplyWindowInsetsListener { v, insets ->
            if (Build.VERSION.SDK_INT >= 30) {
                val b = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.ime() or WindowInsets.Type.displayCutout())
                v.setPadding(b.left, b.top, b.right, b.bottom)
            } else {
                @Suppress("DEPRECATION")
                v.setPadding(insets.systemWindowInsetLeft, insets.systemWindowInsetTop, insets.systemWindowInsetRight, insets.systemWindowInsetBottom)
            }
            insets
        }
    }
    setContentView(root)
}

private fun Activity.topBar(text: String, settingsAction: Boolean) = LinearLayout(this).apply {
    orientation = LinearLayout.HORIZONTAL
    gravity = Gravity.CENTER_VERTICAL
    setPadding(dp(8), dp(6), dp(8), dp(6))
    addView(iconButton(R.drawable.ic_back, "Back") { finish() })
    addView(title(text, 20f).apply { letterSpacing = 0.03f; setPadding(dp(6), 0, 0, 0) }, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
    if (settingsAction) addView(iconButton(R.drawable.ic_settings, "Connection settings") { openSettings() })
}

fun Context.iconButton(res: Int, description: String, onClick: () -> Unit) = ImageButton(this).apply {
    setImageDrawable(icon(res, Palette.TEXT))
    contentDescription = description
    background = pressable(rounded(Color.TRANSPARENT, 24))
    layoutParams = LinearLayout.LayoutParams(dp(48), dp(48))
    setOnClickListener { onClick() }
}

/** The saved connection, or opens Settings and closes this screen when there is none yet. */
fun Activity.requireConnection(): Connection? = Connection.load(this) ?: run {
    openSettings()
    finish()
    null
}

/** A spacer-free frame that fills with [child] (for weighted areas). */
fun Context.frame(child: View) = FrameLayout(this).apply { addView(child) }
