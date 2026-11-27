package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Bitmap
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.util.LruCache
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.PopupMenu
import android.widget.TextView

/**
 * The column of sheets on the left of the editor: a thumbnail per sheet, the one on screen
 * marked. Tapping one goes there; each has a menu to insert a blank sheet or a PDF after it,
 * or remove it; holding one starts selecting several to remove together.
 */
@SuppressLint("ViewConstructor") // Built in code only, with what it shows.
internal class PagesPanel(
    context: Context,
    private val renderer: DocumentRenderer,
    private val listener: Listener,
) : LinearLayout(context) {
    /** What the column asks the editor for. */
    interface Listener {
        fun goToPage(page: Int, sheetHeight: Double)
        fun insertBlankPage(at: Int)
        fun duplicatePage(page: Int)
        fun movePage(from: Int, to: Int)
        fun insertPdf(at: Int)
        fun removePages(pages: Set<Int>, withContent: BooleanArray)
        fun closePages()
    }

    private val main = Handler(Looper.getMainLooper())
    // Not `count`: inside the adapter that name is BaseAdapter's own getCount().
    private var pageCount = 0
    private var sheetWidth = 1.0
    private var sheetHeight = 1.0
    private var withContent = BooleanArray(0)
    private var current = 0
    private val selected = sortedSetOf<Int>()

    // Thumbnails are redrawn after a change to their sheet, a few at a time as they scroll
    // into view; the old one shows until the new one is ready. Each sheet counts its changes.
    private var pageGenerations = IntArray(0)

    // Where the canvas is, so an edit only marks the sheets there as changed.
    private var viewTop = 0.0
    private var visibleHeight = 0.0
    private var editedOnly = false
    private val thumbnails = object : LruCache<Int, Thumbnail>(THUMBNAIL_CACHE_BYTES) {
        override fun sizeOf(key: Int, value: Thumbnail): Int = value.bitmap.byteCount
    }
    private val drawing = mutableSetOf<Int>()
    private val thumbnailWidth = Graphite.dp(context, THUMBNAIL_WIDTH_DP)

    private val title = TextView(context).apply {
        textSize = 16f
        setTextColor(Graphite.TEXT)
    }
    private val deleteButton = Graphite.iconButton(context, R.drawable.ic_delete, R.string.pages_remove) {
        if (selected.isNotEmpty()) listener.removePages(selected.toSet(), withContent)
    }.apply { imageTintList = ColorStateList.valueOf(Graphite.DANGER) }
    private val closeButton = Graphite.iconButton(context, R.drawable.ic_close, R.string.pages_close) {
        if (selected.isEmpty()) listener.closePages() else endSelecting()
    }
    private val adapter = PageAdapter()
    private val list = ListView(context).apply {
        divider = null
        isVerticalScrollBarEnabled = false
        adapter = this@PagesPanel.adapter
        setOnItemClickListener { _, _, position, _ ->
            if (selected.isNotEmpty()) toggle(position) else listener.goToPage(position, sheetHeight)
        }
        setOnItemLongClickListener { _, _, position, _ ->
            toggle(position)
            true
        }
    }

    init {
        orientation = VERTICAL
        setBackgroundColor(Graphite.BAR)
        elevation = Graphite.dp(context, 8f).toFloat()
        isClickable = true
        val pad = Graphite.dp(context, 8f)
        addView(LinearLayout(context).apply {
            orientation = HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(pad * 2, pad / 2, pad / 2, pad / 2)
            addView(title, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
            addView(deleteButton)
            addView(closeButton)
        }, LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.WRAP_CONTENT))
        addView(list, LayoutParams(LayoutParams.MATCH_PARENT, 0, 1f))
        showTitle()
    }

    /**
     * The document was edited: redraws the sheets on the canvas, where the pen was, or all of
     * them if their number changed.
     */
    fun refresh() {
        editedOnly = true
        main.removeCallbacks(refreshNow)
        main.postDelayed(refreshNow, REFRESH_DELAY_MS)
    }

    /** Reads every sheet again right away, e.g. when the column opens. */
    fun refreshNow() {
        editedOnly = false
        refreshNow.run()
    }

    private val refreshNow = Runnable {
        renderer.query({ document ->
            val pages = document.pageCount()
            val sheet = if (pages != null) document.sheetSize() else null
            Triple(pages, sheet, if (pages != null) document.pagesWithContent() else BooleanArray(0))
        }) { result ->
            result.onSuccess { (pages, sheet, content) ->
                if (pages == null || sheet == null) {
                    listener.closePages()
                    return@onSuccess
                }
                val sameSheets = pages == pageCount && sheet.first == sheetWidth && sheet.second == sheetHeight
                pageCount = pages
                sheetWidth = sheet.first
                sheetHeight = sheet.second
                withContent = content
                if (editedOnly && sameSheets) {
                    for (page in Pages.visible(viewTop, visibleHeight, sheetHeight, pageCount)) {
                        pageGenerations[page]++
                        drawing.remove(page)
                    }
                } else {
                    val before = pageGenerations
                    pageGenerations = IntArray(pageCount) { before.getOrElse(it) { 0 } + 1 }
                    drawing.clear()
                }
                selected.retainAll { it < pageCount }
                showTitle()
                adapter.notifyDataSetChanged()
            }
        }
    }

    /** Marks the sheet in the middle of [viewport], [visibleHeight] (document units) tall. */
    fun follow(viewport: Viewport, visibleHeight: Double) {
        viewTop = viewport.y
        this.visibleHeight = visibleHeight
        if (pageCount == 0) return
        val page = Pages.current(viewport.y, visibleHeight, sheetHeight, pageCount)
        if (page == current) return
        current = page
        adapter.notifyDataSetChanged()
        list.smoothScrollToPosition(page)
    }

    /** The selection was removed, or the user gave up on it. */
    fun endSelecting() {
        selected.clear()
        showTitle()
        adapter.notifyDataSetChanged()
    }

    private fun toggle(page: Int) {
        if (!selected.remove(page)) selected.add(page)
        showTitle()
        adapter.notifyDataSetChanged()
    }

    private fun showTitle() {
        val selecting = selected.isNotEmpty()
        title.text = if (selecting) {
            resources.getQuantityString(R.plurals.pages_selected, selected.size, selected.size)
        } else {
            context.getString(R.string.pages_title)
        }
        deleteButton.visibility = if (selecting) VISIBLE else GONE
    }

    private fun showMenu(anchor: View, page: Int) {
        PopupMenu(context, anchor).apply {
            menu.add(0, MENU_BLANK, 0, R.string.page_insert_blank_after)
            menu.add(0, MENU_PDF, 1, R.string.page_insert_pdf_after)
            menu.add(0, MENU_DUPLICATE, 2, R.string.page_duplicate)
            menu.add(0, MENU_UP, 3, R.string.page_move_up).isEnabled = page > 0
            menu.add(0, MENU_DOWN, 4, R.string.page_move_down).isEnabled = page < pageCount - 1
            menu.add(0, MENU_REMOVE, 5, R.string.page_remove)
            setOnMenuItemClickListener { item ->
                when (item.itemId) {
                    MENU_BLANK -> listener.insertBlankPage(page + 1)
                    MENU_PDF -> listener.insertPdf(page + 1)
                    MENU_DUPLICATE -> listener.duplicatePage(page)
                    MENU_UP -> listener.movePage(page, page - 1)
                    MENU_DOWN -> listener.movePage(page, page + 1)
                    MENU_REMOVE -> listener.removePages(setOf(page), withContent)
                }
                true
            }
        }.show()
    }

    /** Shows the thumbnail of [page] in [view], drawing it if it is missing or old. */
    private fun bindThumbnail(view: ImageView, page: Int) {
        val cached = thumbnails.get(page)
        view.setImageBitmap(cached?.bitmap)
        val changes = pageGenerations.getOrElse(page) { 0 }
        if ((cached == null || cached.generation != changes) && drawing.add(page)) {
            draw(page)
        }
    }

    private fun draw(page: Int) {
        val requested = pageGenerations.getOrElse(page) { 0 }
        val width = thumbnailWidth
        val zoom = width / sheetWidth
        val height = (sheetHeight * zoom).toInt().coerceAtLeast(1)
        val top = Pages.top(page, sheetHeight)
        renderer.query({ document ->
            Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888).also { bitmap ->
                document.renderThumbnail(bitmap, Viewport(0.0, top, zoom))
            }
        }) { result ->
            if (requested != pageGenerations.getOrElse(page) { -1 }) return@query
            drawing.remove(page)
            result.onSuccess { bitmap ->
                thumbnails.put(page, Thumbnail(bitmap, requested))
                adapter.notifyDataSetChanged()
            }
        }
    }

    private class Thumbnail(val bitmap: Bitmap, val generation: Int)

    private inner class PageAdapter : BaseAdapter() {
        override fun getCount(): Int = this@PagesPanel.pageCount

        override fun getItem(position: Int): Any = position

        override fun getItemId(position: Int): Long = position.toLong()

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val row = (convertView as? PageRow) ?: PageRow(context)
            row.bind(position)
            return row
        }
    }

    private inner class PageRow(context: Context) : LinearLayout(context) {
        private val image = ImageView(context).apply {
            scaleType = ImageView.ScaleType.FIT_XY
            setBackgroundColor(Color.WHITE)
        }
        private val frame = FrameLayout(context)
        private val check = ImageView(context).apply {
            setImageResource(R.drawable.ic_check)
            imageTintList = ColorStateList.valueOf(Color.WHITE)
            background = Graphite.rounded(context, Graphite.ACCENT, 12f)
            val pad = Graphite.dp(context, 3f)
            setPadding(pad, pad, pad, pad)
        }
        private val number = TextView(context).apply {
            textSize = 13f
            gravity = Gravity.CENTER_VERTICAL
        }
        private val more = ImageButton(context).apply {
            setImageResource(R.drawable.ic_more)
            imageTintList = ColorStateList.valueOf(Graphite.ICON)
            background = Graphite.pressable(context, null, 12f)
            contentDescription = context.getString(R.string.page_actions)
            // So the row stays clickable in the list.
            isFocusable = false
        }
        private var page = 0

        init {
            orientation = VERTICAL
            descendantFocusability = FOCUS_BLOCK_DESCENDANTS
            val pad = Graphite.dp(context, 8f)
            setPadding(pad * 2, pad, pad * 2, 0)
            val border = Graphite.dp(context, 3f)
            frame.setPadding(border, border, border, border)
            frame.addView(image, FrameLayout.LayoutParams(thumbnailWidth, FrameLayout.LayoutParams.WRAP_CONTENT))
            val checkSize = Graphite.dp(context, 24f)
            frame.addView(check, FrameLayout.LayoutParams(checkSize, checkSize, Gravity.TOP or Gravity.END).apply {
                setMargins(border * 2, border * 2, border * 2, border * 2)
            })
            addView(frame, LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT).apply {
                gravity = Gravity.CENTER_HORIZONTAL
            })
            addView(LinearLayout(context).apply {
                orientation = HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                addView(number, LayoutParams(0, LayoutParams.WRAP_CONTENT, 1f))
                addView(more, LayoutParams(Graphite.dp(context, 36f), Graphite.dp(context, 32f)))
            }, LayoutParams(thumbnailWidth + border * 2, LayoutParams.WRAP_CONTENT).apply {
                gravity = Gravity.CENTER_HORIZONTAL
            })
            more.setOnClickListener { showMenu(more, page) }
        }

        fun bind(position: Int) {
            page = position
            val height = (thumbnailWidth * sheetHeight / sheetWidth).toInt().coerceAtLeast(1)
            image.layoutParams = image.layoutParams.apply { this.height = height }
            bindThumbnail(image, position)
            val isSelected = position in selected
            val isCurrent = position == current
            frame.background = when {
                isSelected -> Graphite.rounded(context, Graphite.ACCENT, 6f)
                isCurrent -> Graphite.rounded(context, Graphite.BAR, 6f, Graphite.ACCENT)
                else -> null
            }
            check.visibility = if (isSelected) VISIBLE else GONE
            number.text = String.format(java.util.Locale.getDefault(), "%d", position + 1)
            number.setTextColor(if (isCurrent) Graphite.ACCENT else Graphite.TEXT_SECONDARY)
            more.visibility = if (selected.isEmpty()) VISIBLE else INVISIBLE
        }
    }

    companion object {
        /** How wide the column is. */
        const val WIDTH_DP = 176f

        private const val THUMBNAIL_WIDTH_DP = 128f
        private const val THUMBNAIL_CACHE_BYTES = 24 * 1024 * 1024
        private const val REFRESH_DELAY_MS = 400L
        private const val MENU_BLANK = 1
        private const val MENU_PDF = 2
        private const val MENU_REMOVE = 3
        private const val MENU_DUPLICATE = 4
        private const val MENU_UP = 5
        private const val MENU_DOWN = 6
    }
}
