package io.github.yx14ng.rnote.viewer

import android.annotation.SuppressLint
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.drawable.GradientDrawable
import android.text.TextUtils
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/** What the library screen shows. */
internal sealed interface LibrarySection {
    data object All : LibrarySection
    data object Uncategorized : LibrarySection
    data class Folder(val name: String) : LibrarySection
    data class Tag(val id: String) : LibrarySection
    data object Trash : LibrarySection
}

/** The notebooks of [section]; the trash shows its entries as notebooks under `.rnote-library/trash/`. */
internal fun LibrarySnapshot.filter(section: LibrarySection): List<Notebook> = when (section) {
    LibrarySection.All -> notebooks
    LibrarySection.Uncategorized -> notebooks.filter { it.folder == null }
    is LibrarySection.Folder -> notebooks.filter { it.folder == section.name }
    is LibrarySection.Tag -> notebooks.filter { section.id in it.tags }
    LibrarySection.Trash -> trash.map { item ->
        Notebook(
            path = "$TRASH_PATH/${item.id}",
            name = item.name,
            folder = null,
            isXopp = item.originalPath.endsWith(".xopp"),
            modifiedMs = item.deletedAtMs,
            size = 0,
            tags = emptyList(),
            key = "trash-${item.id}",
        )
    }
}

/** Where the library keeps the trash, relative to its root. */
internal const val TRASH_PATH = ".rnote-library/trash"

/**
 * The library's left column: all notes, uncategorized, the folders and tags (each with a
 * `+` to create one), and the trash, each with how many notebooks it has.
 */
@SuppressLint("ViewConstructor") // Built in code only, with what it shows.
internal class LibrarySidebar(context: Context) : ScrollView(context) {
    var onSection: ((LibrarySection) -> Unit)? = null
    var onCreateFolder: (() -> Unit)? = null
    var onCreateTag: (() -> Unit)? = null
    var onFolderMenu: ((String, View) -> Unit)? = null
    var onTagMenu: ((LibraryTag, View) -> Unit)? = null

    private val column = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(0, dp(12f), 0, dp(12f))
    }

    init {
        setBackgroundColor(Graphite.BAR)
        isVerticalScrollBarEnabled = false
        addView(column, LayoutParams(MATCH_PARENT, WRAP_CONTENT))
    }

    fun show(snapshot: LibrarySnapshot, current: LibrarySection) {
        column.removeAllViews()
        fun row(section: LibrarySection, icon: Int?, dotColor: Int?, label: String, onLongClick: ((View) -> Unit)? = null) =
            column.addView(sectionRow(icon, dotColor, label, snapshot.filter(section).size, section == current, onLongClick) {
                onSection?.invoke(section)
            })

        row(LibrarySection.All, R.drawable.ic_page, null, context.getString(R.string.library_all))
        row(LibrarySection.Uncategorized, R.drawable.ic_open, null, context.getString(R.string.library_uncategorized))
        column.addView(header(R.string.library_folders) { onCreateFolder?.invoke() })
        for (folder in snapshot.folders.sortedWith(String.CASE_INSENSITIVE_ORDER)) {
            row(LibrarySection.Folder(folder), R.drawable.ic_open, null, folder) { view -> onFolderMenu?.invoke(folder, view) }
        }
        column.addView(header(R.string.library_tags) { onCreateTag?.invoke() })
        for (tag in snapshot.tags) {
            row(LibrarySection.Tag(tag.id), null, tag.color, tag.name) { view -> onTagMenu?.invoke(tag, view) }
        }
        column.addView(divider())
        row(LibrarySection.Trash, R.drawable.ic_delete, null, context.getString(R.string.library_trash))
    }

    private fun sectionRow(
        icon: Int?,
        dotColor: Int?,
        label: String,
        count: Int,
        current: Boolean,
        onLongClick: ((View) -> Unit)?,
        onClick: () -> Unit,
    ) = LinearLayout(context).apply {
        gravity = Gravity.CENTER_VERTICAL
        minimumHeight = dp(48f)
        setPadding(dp(18f), 0, dp(18f), 0)
        val tint = if (current) Graphite.ACCENT else Graphite.TEXT
        background = Graphite.pressable(context, if (current) Graphite.rounded(context, Graphite.RAISED, 0f) else null, 0f)
        isClickable = true
        contentDescription = label
        setOnClickListener { onClick() }
        if (onLongClick != null) {
            setOnLongClickListener { view -> onLongClick(view); true }
        }
        val marker = if (icon != null) {
            ImageView(context).apply {
                setImageResource(icon)
                imageTintList = ColorStateList.valueOf(tint)
            }
        } else {
            View(context).apply {
                background = GradientDrawable().apply { shape = GradientDrawable.OVAL; setColor(dotColor ?: tint) }
            }
        }
        val markerSize = if (icon != null) dp(20f) else dp(12f)
        addView(marker, LinearLayout.LayoutParams(markerSize, markerSize).apply { marginEnd = dp(16f) + (dp(20f) - markerSize) / 2; marginStart = (dp(20f) - markerSize) / 2 })
        addView(TextView(context).apply {
            text = label
            textSize = 15f
            setTextColor(tint)
            maxLines = 1
            ellipsize = TextUtils.TruncateAt.END
        }, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
        addView(TextView(context).apply {
            text = count.toString()
            textSize = 13f
            setTextColor(Graphite.TEXT_SECONDARY)
        })
    }

    private fun header(label: Int, onAdd: () -> Unit) = LinearLayout(context).apply {
        gravity = Gravity.CENTER_VERTICAL
        setPadding(0, dp(10f), dp(8f), 0)
        addView(Graphite.sectionTitle(context, label).apply { setPadding(dp(18f), 0, 0, 0) }, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
        addView(ImageButton(context).apply {
            setImageResource(R.drawable.ic_add)
            imageTintList = ColorStateList.valueOf(Graphite.ACCENT)
            background = Graphite.pressable(context, null, 14f)
            contentDescription = context.getString(if (label == R.string.library_folders) R.string.library_new_folder else R.string.library_new_tag)
            tooltipText = contentDescription
            setOnClickListener { onAdd() }
        }, LinearLayout.LayoutParams(dp(40f), dp(40f)))
    }

    private fun divider() = View(context).apply { setBackgroundColor(Graphite.DIVIDER) }.also { view ->
        view.layoutParams = LinearLayout.LayoutParams(MATCH_PARENT, dp(1f)).apply { setMargins(dp(18f), dp(10f), dp(18f), dp(10f)) }
    }

    private fun dp(value: Float): Int = Graphite.dp(context, value)
}
