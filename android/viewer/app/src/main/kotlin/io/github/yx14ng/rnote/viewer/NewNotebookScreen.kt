package io.github.yx14ng.rnote.viewer

import android.app.AlertDialog
import android.app.Dialog
import android.content.Context
import android.content.res.ColorStateList
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Switch
import android.widget.TextView
import io.github.yx14ng.rnote.viewer.NewNotebookOptions.dp

/**
 * The "new notebook" screen, full screen over the library: previews of the cover and the
 * page, the title, folder, tags and cover, and the paper (format, pages, color, spacing,
 * template). It starts as the last notebook of the folder; [onCreate] gets what was chosen.
 */
internal class NewNotebookScreen(
    private val context: Context,
    private val snapshot: LibrarySnapshot,
    private var folder: String?,
    private var setup: NotebookSetup,
    private val onCreate: (folder: String?, title: String, setup: NotebookSetup, tags: List<String>) -> Unit,
) {
    private val dialog = Dialog(context, android.R.style.Theme_DeviceDefault_NoActionBar)
    private var tags: List<String> = emptyList()
    /** The cover color while the cover is off, so turning it back on keeps it. */
    private var lastCover = setup.cover ?: CoverColor.BLUE.argb

    private val cover = CoverView(context)
    private val noCover = TextView(context)
    private val paper = PaperPreview(context)
    private val title = EditText(context)
    private val folderValue = NewNotebookOptions.dropDown(context) { chooseFolder() }
    private val tagChips = LinearLayout(context).apply { gravity = Gravity.CENTER_VERTICAL }
    private val coverSwitch = Switch(context)
    private lateinit var coverColors: Pair<LinearLayout, (Int?) -> Unit>
    private val formatValue = NewNotebookOptions.dropDown(context) { chooseFormat() }
    private lateinit var layouts: (Int) -> Unit
    private lateinit var papers: (Int?) -> Unit
    private val paperLabel = TextView(context)
    private lateinit var spacings: (Int) -> Unit
    private val templates = Template.entries.map { PaperPreview(context) }
    private val templateLabels = Template.entries.map { TextView(context) }

    fun show() {
        dialog.setContentView(build())
        dialog.window?.setLayout(MATCH_PARENT, MATCH_PARENT)
        refresh()
        dialog.show()
    }

    private fun build(): View = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        setBackgroundColor(Graphite.SURFACE)
        addView(header(), LinearLayout.LayoutParams(MATCH_PARENT, dp(context, 72f)))
        addView(ScrollView(context).apply {
            addView(LinearLayout(context).apply {
                orientation = LinearLayout.VERTICAL
                addView(previewSection())
                addView(paperSection())
            })
        }, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
    }

    private fun header(): View = LinearLayout(context).apply {
        gravity = Gravity.CENTER_VERTICAL
        setBackgroundColor(Graphite.BAR)
        setPadding(dp(context, 20f), 0, dp(context, 20f), 0)
        addView(TextView(context).apply {
            setText(android.R.string.cancel)
            textSize = 17f
            setTextColor(NewNotebookOptions.LINK)
            gravity = Gravity.CENTER_VERTICAL
            background = Graphite.pressable(context, null, 12f)
            setOnClickListener { dialog.dismiss() }
        }, LinearLayout.LayoutParams(dp(context, 110f), dp(context, 48f)))
        addView(TextView(context).apply {
            setText(R.string.new_notebook_title)
            textSize = 20f
            setTextColor(Graphite.TEXT)
            gravity = Gravity.CENTER
        }, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
        addView(TextView(context).apply {
            setText(R.string.new_notebook_create)
            textSize = 17f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            background = Graphite.pressable(context, Graphite.rounded(context, NewNotebookOptions.CHOSEN, 14f), 14f)
            setOnClickListener {
                dialog.dismiss()
                onCreate(folder, title.text.toString(), setup, tags)
            }
        }, LinearLayout.LayoutParams(dp(context, 110f), dp(context, 48f)))
    }

    private fun previewSection(): View {
        val section = NewNotebookOptions.column(context, 28f, Graphite.RAISED)
        val previews = LinearLayout(context).apply {
            addView(captioned(FrameLayout(context).apply {
                addView(cover, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
                addView(noCover.apply {
                    setText(R.string.new_notebook_no_cover_preview)
                    textSize = 14f
                    setTextColor(Graphite.TEXT_SECONDARY)
                    gravity = Gravity.CENTER
                    val pad = dp(context, 16f)
                    setPadding(pad, pad, pad, pad)
                    background = GradientDrawable().apply {
                        cornerRadius = dp(context, 10f).toFloat()
                        setStroke(dp(context, 2f), Graphite.OUTLINE, dp(context, 6f).toFloat(), dp(context, 4f).toFloat())
                    }
                }, FrameLayout.LayoutParams(MATCH_PARENT, MATCH_PARENT))
            }, R.string.new_notebook_cover))
            addView(captioned(FrameLayout(context).apply {
                addView(paper, FrameLayout.LayoutParams(PREVIEW_WIDTH_DP.dpPx(), PREVIEW_HEIGHT_DP.dpPx(), Gravity.CENTER))
            }, R.string.new_notebook_page, widthDp = PREVIEW_HEIGHT_DP), LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply {
                marginStart = dp(context, 18f)
            })
        }
        val fields = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            addView(TextView(context).apply {
                setText(R.string.new_notebook_title_label)
                textSize = 13f
                setTextColor(Graphite.TEXT_SECONDARY)
                setPadding(dp(context, 2f), 0, 0, dp(context, 6f))
            })
            addView(title.apply {
                setHint(R.string.new_notebook_name_hint)
                setHintTextColor(Color.rgb(0x8D, 0x90, 0x9A))
                setTextColor(Graphite.TEXT)
                textSize = 17f
                isSingleLine = true
                inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
                background = Graphite.rounded(context, Graphite.SURFACE, 14f, Graphite.OUTLINE)
                setPadding(dp(context, 16f), 0, dp(context, 16f), 0)
                addTextChangedListener(object : TextWatcher {
                    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
                    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) = Unit
                    override fun afterTextChanged(s: Editable?) = refresh()
                })
            }, LinearLayout.LayoutParams(MATCH_PARENT, dp(context, 52f)))
            addView(NewNotebookOptions.row(context, R.string.new_notebook_folder, folderValue), spaced(10f))
            addView(NewNotebookOptions.row(context, R.string.library_tags, tagRow()))
            addView(NewNotebookOptions.row(context, R.string.new_notebook_cover, coverSwitch.apply {
                thumbTintList = ColorStateList.valueOf(Color.WHITE)
                trackTintList = ColorStateList(
                    arrayOf(intArrayOf(android.R.attr.state_checked), intArrayOf()),
                    intArrayOf(NewNotebookOptions.CHOSEN, Graphite.OUTLINE),
                )
                setOnCheckedChangeListener { _, on ->
                    setup = setup.copy(cover = if (on) lastCover else null)
                    refresh()
                }
            }))
            val covers = CoverColor.entries.map { it.argb to context.getString(it.label) }
            coverColors = NewNotebookOptions.swatches(context, covers, null) { i ->
                lastCover = CoverColor.entries[i].argb
                setup = setup.copy(cover = lastCover)
                refresh()
            }
            addView(coverColors.first, spaced(10f))
            addView(NewNotebookOptions.row(context, R.string.new_notebook_format, formatValue))
        }
        section.addView(LinearLayout(context).apply {
            addView(previews)
            addView(fields, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f).apply { marginStart = dp(context, 24f) })
        })
        return section
    }

    private fun paperSection(): View {
        val section = NewNotebookOptions.column(context, 28f)
        val layoutRow = NewNotebookOptions.choices(
            context, SetupLayout.entries.map { context.getString(it.label) }, 0, segmented = true,
        ) { i -> setup = setup.copy(layout = SetupLayout.entries[i]); refresh() }
        layouts = layoutRow.second
        section.addView(labeled(R.string.new_notebook_pages, layoutRow.first))
        val paperRow = NewNotebookOptions.swatches(
            context, BackgroundColor.entries.map { it.color to context.getString(it.label) }, null,
        ) { i -> setup = setup.withPaper(BackgroundColor.entries[i]); refresh() }
        papers = paperRow.second
        section.addView(labeled(R.string.new_notebook_paper, LinearLayout(context).apply {
            gravity = Gravity.CENTER_VERTICAL
            addView(paperRow.first)
            addView(paperLabel.apply { textSize = 14f; setTextColor(Graphite.TEXT_SECONDARY) },
                LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply { marginStart = dp(context, 14f) })
        }), spaced(22f))
        val spacingRow = NewNotebookOptions.choices(
            context, SPACINGS_MM.map { context.getString(R.string.new_notebook_spacing_mm, FormValues.formatNumber(it)) }, 0,
            segmented = false,
        ) { i -> setup = setup.copy(patternSizeMm = SPACINGS_MM[i]); refresh() }
        spacings = spacingRow.second
        section.addView(labeled(R.string.new_notebook_spacing, spacingRow.first), spaced(22f))
        section.addView(NewNotebookOptions.heading(context, R.string.new_notebook_template), spaced(28f))
        section.addView(LinearLayout(context).apply {
            Template.entries.forEachIndexed { i, template ->
                addView(LinearLayout(context).apply {
                    orientation = LinearLayout.VERTICAL
                    gravity = Gravity.CENTER_HORIZONTAL
                    isClickable = true
                    setOnClickListener { setup = setup.copy(template = template); refresh() }
                    addView(templates[i].apply { this.template = template }, LinearLayout.LayoutParams(MATCH_PARENT, dp(context, 230f)))
                    addView(templateLabels[i].apply { setText(template.label); textSize = 15f; setPadding(0, dp(context, 10f), 0, 0) })
                }, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f).apply { if (i > 0) marginStart = dp(context, 20f) })
            }
        }, spaced(14f))
        section.addView(NewNotebookOptions.hint(context, R.string.new_notebook_hint), spaced(28f))
        return section
    }

    private fun tagRow(): View = LinearLayout(context).apply {
        gravity = Gravity.CENTER_VERTICAL
        addView(tagChips)
        addView(ImageButton(context).apply {
            setImageResource(R.drawable.ic_add)
            imageTintList = ColorStateList.valueOf(Graphite.ICON)
            background = Graphite.pressable(context, null, 22f)
            contentDescription = context.getString(R.string.library_tags)
            setOnClickListener { chooseTags() }
        }, LinearLayout.LayoutParams(dp(context, 44f), dp(context, 44f)))
    }

    /** Shows the current choices everywhere. */
    private fun refresh() {
        val name = title.text.toString().trim().ifEmpty { context.getString(R.string.untitled) }
        val coverColor = setup.cover
        cover.visibility = if (coverColor != null) View.VISIBLE else View.GONE
        noCover.visibility = if (coverColor == null) View.VISIBLE else View.GONE
        cover.color = coverColor ?: lastCover
        cover.name = name
        if (coverSwitch.isChecked != (coverColor != null)) coverSwitch.isChecked = coverColor != null
        coverColors.first.visibility = if (coverColor != null) View.VISIBLE else View.GONE
        coverColors.second(CoverColor.entries.indexOfFirst { it.argb == coverColor }.takeIf { it >= 0 })
        folderValue.text = folder ?: context.getString(R.string.library_uncategorized)
        formatValue.text = formatLabel()
        tagChips.removeAllViews()
        for (tag in snapshot.tags.filter { it.id in tags }) tagChips.addView(chip(tag.name))
        val paperIndex = BackgroundColor.entries.indexOfFirst { it.color == setup.backgroundArgb }
        papers(paperIndex.takeIf { it >= 0 })
        paperLabel.text = BackgroundColor.entries.getOrNull(paperIndex)?.let { context.getString(it.label) }.orEmpty()
        layouts(setup.layout.ordinal)
        spacings(SPACINGS_MM.indexOfFirst { kotlin.math.abs(it - setup.patternSizeMm) < 0.01 })
        listOf(paper).plus(templates).forEach { preview ->
            preview.paper = setup.backgroundArgb
            preview.line = setup.patternArgb
            preview.spacingMm = setup.patternSizeMm
        }
        paper.template = setup.template
        paper.layoutParams = (paper.layoutParams as FrameLayout.LayoutParams).apply {
            width = (if (setup.landscape) PREVIEW_HEIGHT_DP else PREVIEW_WIDTH_DP).dpPx()
            height = (if (setup.landscape) PREVIEW_WIDTH_DP else PREVIEW_HEIGHT_DP).dpPx()
        }
        templates.forEachIndexed { i, preview ->
            val chosen = Template.entries[i] == setup.template
            preview.foreground = if (chosen) GradientDrawable().apply {
                cornerRadius = dp(context, 6f).toFloat()
                setStroke(dp(context, 3f), NewNotebookOptions.LINK)
            } else null
            templateLabels[i].setTextColor(if (chosen) NewNotebookOptions.LINK else Graphite.TEXT)
        }
    }

    private fun formatLabel(): String {
        val orientation = context.getString(if (setup.landscape) R.string.orientation_landscape else R.string.orientation_portrait)
        val size = setup.pageSize?.let { context.getString(it.label) }
            ?: context.getString(R.string.new_notebook_size_mm, FormValues.formatNumber(setup.pageWidthMm), FormValues.formatNumber(setup.pageHeightMm))
        return "$size · $orientation"
    }

    private fun chooseFolder() {
        val folders = snapshot.folders.sortedWith(String.CASE_INSENSITIVE_ORDER)
        val choices = listOf(context.getString(R.string.library_uncategorized)) + folders
        AlertDialog.Builder(context)
            .setTitle(R.string.new_notebook_folder)
            .setItems(choices.toTypedArray()) { _, which -> folder = folders.getOrNull(which - 1); refresh() }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun chooseFormat() {
        val options = PageSize.entries.flatMap { size -> listOf(size to false, size to true) }
        val labels = options.map { (size, landscape) ->
            val orientation = context.getString(if (landscape) R.string.orientation_landscape else R.string.orientation_portrait)
            "${context.getString(size.label)} · $orientation"
        }
        AlertDialog.Builder(context)
            .setTitle(R.string.new_notebook_format)
            .setItems(labels.toTypedArray()) { _, which ->
                val (size, landscape) = options[which]
                setup = setup.withPageSize(size, landscape)
                refresh()
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun chooseTags() {
        val all = snapshot.tags
        if (all.isEmpty()) {
            AlertDialog.Builder(context).setMessage(R.string.new_notebook_no_tags).setPositiveButton(android.R.string.ok, null).show()
            return
        }
        val checked = BooleanArray(all.size) { all[it].id in tags }
        AlertDialog.Builder(context)
            .setTitle(R.string.library_tags)
            .setMultiChoiceItems(all.map { it.name }.toTypedArray(), checked) { _, which, on -> checked[which] = on }
            .setPositiveButton(R.string.apply) { _, _ -> tags = all.filterIndexed { i, _ -> checked[i] }.map { it.id }; refresh() }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun chip(name: String) = TextView(context).apply {
        text = name
        textSize = 14f
        setTextColor(Graphite.TEXT)
        gravity = Gravity.CENTER_VERTICAL
        setPadding(dp(context, 12f), 0, dp(context, 12f), 0)
        background = Graphite.rounded(context, Graphite.OUTLINE, 16f)
        layoutParams = LinearLayout.LayoutParams(WRAP_CONTENT, dp(context, 32f)).apply { marginEnd = dp(context, 8f) }
    }

    /** [view] in a box [widthDp] wide (room for a landscape page) with [caption] below. */
    private fun captioned(view: View, caption: Int, widthDp: Float = PREVIEW_WIDTH_DP): View = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        gravity = Gravity.CENTER_HORIZONTAL
        addView(view, LinearLayout.LayoutParams(widthDp.dpPx(), PREVIEW_HEIGHT_DP.dpPx()))
        addView(TextView(context).apply {
            setText(caption)
            textSize = 14f
            setTextColor(Graphite.TEXT_SECONDARY)
            setPadding(0, dp(context, 10f), 0, 0)
        })
    }

    private fun labeled(label: Int, content: View): View = LinearLayout(context).apply {
        gravity = Gravity.CENTER_VERTICAL
        addView(NewNotebookOptions.heading(context, label))
        addView(content, LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply { marginStart = dp(context, 18f) })
    }

    private fun spaced(topDp: Float) = LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(context, topDp) }

    private fun Float.dpPx(): Int = dp(context, this)

    private companion object {
        const val PREVIEW_WIDTH_DP = 168f
        const val PREVIEW_HEIGHT_DP = 238f
    }
}
