package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.content.res.ColorStateList
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.PopupWindow
import android.widget.TextView
import io.github.yx14ng.rnote.viewer.NewNotebookOptions.dp

/** The menu of the library's "New" card: the ways to make a notebook. */
internal object CreateMenu {
    /** An entry: [hint] goes beside the label, [detail] under it; [startsGroup] adds a divider above. */
    class Item(
        val icon: Int,
        val label: Int,
        val hint: Int? = null,
        val detail: Int? = null,
        val startsGroup: Boolean = false,
        val onPick: () -> Unit,
    )

    fun show(context: Context, anchor: View, items: List<Item>) {
        val list = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            background = Graphite.rounded(context, Graphite.RAISED, Graphite.PANEL_RADIUS_DP, Graphite.OUTLINE)
            val pad = dp(context, 8f)
            setPadding(pad, pad, pad, pad)
            elevation = dp(context, 12f).toFloat()
        }
        val popup = PopupWindow(list, dp(context, MENU_WIDTH_DP), WRAP_CONTENT, true).apply {
            elevation = dp(context, 12f).toFloat()
        }
        for (item in items) {
            if (item.startsGroup) {
                list.addView(View(context).apply { setBackgroundColor(Graphite.DIVIDER) },
                    LinearLayout.LayoutParams(MATCH_PARENT, dp(context, 1f)).apply {
                        setMargins(dp(context, 12f), dp(context, 6f), dp(context, 12f), dp(context, 6f))
                    })
            }
            list.addView(entry(context, item) { popup.dismiss(); item.onPick() })
        }
        popup.showAsDropDown(anchor, anchor.width / 2, -anchor.height + dp(context, 52f), Gravity.START)
    }

    private fun entry(context: Context, item: Item, onClick: () -> Unit): View = LinearLayout(context).apply {
        gravity = Gravity.CENTER_VERTICAL
        minimumHeight = dp(context, if (item.detail != null) 64f else 56f)
        setPadding(dp(context, 16f), dp(context, 6f), dp(context, 16f), dp(context, 6f))
        background = Graphite.pressable(context, null, Graphite.BUTTON_RADIUS_DP)
        isClickable = true
        setOnClickListener { onClick() }
        addView(ImageView(context).apply {
            setImageResource(item.icon)
            imageTintList = ColorStateList.valueOf(Graphite.ICON)
        }, LinearLayout.LayoutParams(dp(context, 24f), dp(context, 24f)))
        val texts = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            addView(TextView(context).apply {
                setText(item.label)
                textSize = 17f
                setTextColor(Graphite.TEXT)
            })
            if (item.detail != null) {
                addView(TextView(context).apply {
                    setText(item.detail)
                    textSize = 13f
                    setTextColor(Graphite.TEXT_SECONDARY)
                })
            }
        }
        addView(texts, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f).apply { marginStart = dp(context, 16f) })
        if (item.hint != null) {
            addView(TextView(context).apply {
                setText(item.hint)
                textSize = 13f
                setTextColor(Graphite.TEXT_SECONDARY)
            })
        }
    }

    private const val MENU_WIDTH_DP = 340f
}
