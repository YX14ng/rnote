package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.Outline
import android.graphics.drawable.GradientDrawable
import android.text.TextUtils
import android.text.format.DateFormat
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.view.ViewOutlineProvider
import android.widget.BaseAdapter
import android.widget.FrameLayout
import android.widget.GridView
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import java.util.Date

/**
 * The notebooks of a library section: a "New" card first, then a card per notebook with its
 * thumbnail, name and date, as a grid or as a list.
 */
@SuppressLint("ViewConstructor") // Built in code only, with what it shows.
internal class NotebookGrid(context: Context, private val thumbnails: ThumbnailCache) : GridView(context) {
    /** A card was tapped; the view is the card. */
    var onOpen: ((Notebook, View) -> Unit)? = null
    /** The "New" card was tapped; the view is the card, to anchor a menu. */
    var onNew: ((View) -> Unit)? = null
    var onMenu: ((Notebook, View) -> Unit)? = null

    /** Paths of the selected notebooks while selecting several; null when not selecting. */
    var selection: Set<String>? = null
        set(value) {
            field = value
            adapter.notifyDataSetChanged()
        }

    private var notebooks: List<Notebook> = emptyList()
    private var listMode = false
    private var offerNew = true
    private val adapter = Adapter()

    init {
        val gap = dp(14f)
        setPadding(gap, gap, gap, gap)
        clipToPadding = false
        horizontalSpacing = gap
        verticalSpacing = gap
        stretchMode = STRETCH_COLUMN_WIDTH
        selector = GradientDrawable() // The cards show their own pressed state.
        setAdapter(adapter)
    }

    /** Shows [notebooks]; [offerNew] adds the "New" card (not in the trash). */
    fun show(notebooks: List<Notebook>, listMode: Boolean, offerNew: Boolean) {
        this.notebooks = notebooks
        this.offerNew = offerNew
        if (this.listMode != listMode) {
            this.listMode = listMode
            updateColumns()
        }
        adapter.notifyDataSetChanged()
    }

    // The columns follow the width; set while measuring, so the first layout already has them.
    override fun onMeasure(widthMeasureSpec: Int, heightMeasureSpec: Int) {
        updateColumns(MeasureSpec.getSize(widthMeasureSpec))
        super.onMeasure(widthMeasureSpec, heightMeasureSpec)
    }

    private fun updateColumns(widthPx: Int = width) {
        val widthDp = widthPx / resources.displayMetrics.density
        val columns = if (listMode) 1 else (widthDp / CARD_WIDTH_DP).toInt().coerceIn(2, MAX_COLUMNS)
        if (columns != numColumns) {
            numColumns = columns
        }
    }

    private inner class Adapter : BaseAdapter() {
        override fun getCount() = notebooks.size + if (offerNew) 1 else 0

        override fun getItem(position: Int): Notebook? =
            notebooks.getOrNull(if (offerNew) position - 1 else position)

        override fun getItemId(position: Int) = position.toLong()

        override fun getViewTypeCount() = 2

        override fun getItemViewType(position: Int) = if (getItem(position) == null) 0 else 1

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val notebook = getItem(position) ?: return newCard()
            val card = (convertView as? Card)?.takeIf { it.listMode == listMode } ?: Card(listMode)
            card.bind(notebook)
            return card
        }
    }

    private fun newCard(): View = LinearLayout(context).apply {
        orientation = if (listMode) LinearLayout.HORIZONTAL else LinearLayout.VERTICAL
        gravity = Gravity.CENTER
        minimumHeight = if (listMode) dp(64f) else dp(CARD_WIDTH_DP * PAGE_RATIO)
        background = GradientDrawable().apply {
            cornerRadius = dp(12f).toFloat()
            setStroke(dp(1.5f), Graphite.ACCENT, dp(6f).toFloat(), dp(4f).toFloat())
        }
        foreground = Graphite.pressable(context, null, 12f)
        isClickable = true
        contentDescription = context.getString(R.string.library_new)
        setOnClickListener { card -> onNew?.invoke(card) }
        addView(ImageView(context).apply {
            setImageResource(R.drawable.ic_add)
            imageTintList = ColorStateList.valueOf(Graphite.ACCENT)
        }, LinearLayout.LayoutParams(dp(36f), dp(36f)))
        addView(TextView(context).apply {
            setText(R.string.library_new)
            setTextColor(Graphite.ACCENT)
            textSize = 15f
            setPadding(dp(8f), dp(6f), dp(8f), 0)
        }, LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT))
    }

    /** A notebook: thumbnail over name and date, or beside them in the list. */
    private inner class Card(val listMode: Boolean) : LinearLayout(context) {
        private val cover = ImageView(context).apply {
            scaleType = ImageView.ScaleType.FIT_CENTER
            background = Graphite.rounded(context, COVER_COLOR, 10f)
            outlineProvider = object : ViewOutlineProvider() {
                override fun getOutline(view: View, outline: Outline) {
                    outline.setRoundRect(0, 0, view.width, view.height, dp(10f).toFloat())
                }
            }
            clipToOutline = true
        }
        private val check = ImageView(context).apply {
            setImageResource(R.drawable.ic_check)
            imageTintList = ColorStateList.valueOf(Color.WHITE)
            background = GradientDrawable().apply { shape = GradientDrawable.OVAL; setColor(Graphite.ACCENT) }
            val pad = dp(4f)
            setPadding(pad, pad, pad, pad)
        }
        private val colorCover = CoverView(context)
        private val coverFrame = FrameLayout(context).apply {
            addView(cover, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
            addView(colorCover, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
            addView(check, FrameLayout.LayoutParams(dp(26f), dp(26f), Gravity.TOP or Gravity.END).apply {
                setMargins(dp(6f), dp(6f), dp(6f), dp(6f))
            })
        }
        private val name = TextView(context).apply {
            textSize = 15f
            setTextColor(Graphite.TEXT)
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
        }
        private val detail = TextView(context).apply { textSize = 12f }
        private val menu = ImageButton(context).apply {
            setImageResource(R.drawable.ic_chevron_down)
            imageTintList = ColorStateList.valueOf(Graphite.ICON)
            background = Graphite.pressable(context, null, 14f)
            contentDescription = context.getString(R.string.library_notebook_menu)
        }
        private var notebook: Notebook? = null

        init {
            orientation = if (listMode) HORIZONTAL else VERTICAL
            gravity = if (listMode) Gravity.CENTER_VERTICAL else Gravity.CENTER_HORIZONTAL
            foreground = Graphite.pressable(context, null, 12f)
            isClickable = true
            setOnClickListener { card -> notebook?.let { onOpen?.invoke(it, card) } }
            menu.setOnClickListener { view -> notebook?.let { onMenu?.invoke(it, view) } }
            val texts = LinearLayout(context).apply {
                orientation = VERTICAL
                addView(name)
                addView(detail)
            }
            if (listMode) {
                addView(coverFrame, LayoutParams(dp(48f), dp(64f)))
                addView(texts, LayoutParams(0, WRAP_CONTENT, 1f).apply { marginStart = dp(14f) })
                addView(menu, LayoutParams(dp(40f), dp(40f)))
            } else {
                val coverHeight = (CARD_WIDTH_DP * PAGE_RATIO)
                addView(coverFrame, LayoutParams(MATCH_PARENT, dp(coverHeight)))
                val row = LinearLayout(context).apply {
                    gravity = Gravity.CENTER_VERTICAL
                    addView(texts, LayoutParams(0, WRAP_CONTENT, 1f))
                    addView(menu, LayoutParams(dp(36f), dp(36f)))
                }
                addView(row, LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(6f) })
            }
        }

        fun bind(notebook: Notebook) {
            this.notebook = notebook
            name.text = notebook.name
            if (notebook.isXopp) {
                detail.setText(R.string.library_xopp_import)
                detail.setTextColor(Graphite.ACCENT)
            } else {
                detail.text = DateFormat.getDateFormat(context).format(Date(notebook.modifiedMs))
                detail.setTextColor(Graphite.TEXT_SECONDARY)
            }
            contentDescription = notebook.name
            val selected = selection
            check.visibility = if (selected != null && notebook.path in selected) VISIBLE else GONE
            cover.alpha = if (selected != null && notebook.path !in selected) UNSELECTED_ALPHA else 1f
            menu.visibility = if (selected == null) VISIBLE else INVISIBLE
            val coverColor = notebook.cover
            colorCover.visibility = if (coverColor != null) VISIBLE else GONE
            cover.visibility = if (coverColor != null) INVISIBLE else VISIBLE
            colorCover.alpha = cover.alpha
            if (coverColor != null) {
                colorCover.color = coverColor
                colorCover.name = if (listMode) "" else notebook.name
            } else {
                thumbnails.load(notebook, cover)
            }
        }
    }

    private fun dp(value: Float): Int = Graphite.dp(context, value)

    private companion object {
        const val CARD_WIDTH_DP = 190f
        const val MAX_COLUMNS = 6
        /** Height over width of an A4 page, for covers. */
        const val PAGE_RATIO = 1.414f
        const val UNSELECTED_ALPHA = 0.55f
        val COVER_COLOR = Color.rgb(0x33, 0x35, 0x3C)
    }
}
