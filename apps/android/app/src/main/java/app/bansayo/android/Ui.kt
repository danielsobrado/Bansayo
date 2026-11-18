package app.bansayo.android

import android.content.Context
import android.graphics.Typeface
import android.text.InputType
import android.view.View
import android.view.ViewGroup
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Spinner
import android.widget.TextView

/** Small builders for programmatic layouts: the app has a few screens and no layout XML. */
fun Context.dp(v: Int) = (v * resources.displayMetrics.density).toInt()

fun Context.column(vararg children: View): LinearLayout = LinearLayout(this).apply {
    orientation = LinearLayout.VERTICAL
    setPadding(dp(16), dp(16), dp(16), dp(16))
    children.forEach { addView(it) }
}

fun Context.row(vararg children: View): LinearLayout = LinearLayout(this).apply {
    orientation = LinearLayout.HORIZONTAL
    children.forEach {
        addView(it, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
    }
}

fun Context.scroll(content: View): ScrollView = ScrollView(this).apply { addView(content) }

fun Context.heading(text: String) = TextView(this).apply {
    this.text = text
    textSize = 20f
    setTypeface(typeface, Typeface.BOLD)
    setPadding(0, dp(16), 0, dp(4))
}

fun Context.label(text: String = "") = TextView(this).apply {
    this.text = text
    textSize = 15f
    setTextIsSelectable(true)
}

fun Context.button(text: String, onClick: () -> Unit) = Button(this).apply {
    this.text = text
    isAllCaps = false
    setOnClickListener { onClick() }
}

fun Context.field(hint: String, value: String = "", number: Boolean = false) = EditText(this).apply {
    this.hint = hint
    setText(value)
    isSingleLine = true
    inputType = if (number) InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL
    else InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
}

fun Context.spinner(items: List<String>) = Spinner(this).apply { setItems(items) }

fun Spinner.setItems(items: List<String>) {
    adapter = ArrayAdapter(context, android.R.layout.simple_spinner_dropdown_item, items)
}

fun Spinner.selected(): String? = selectedItem as? String
