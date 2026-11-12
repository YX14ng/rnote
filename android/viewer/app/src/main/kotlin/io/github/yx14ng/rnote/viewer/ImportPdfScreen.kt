package io.github.yx14ng.rnote.viewer

import android.app.AlertDialog
import android.app.Dialog
import android.content.Context
import android.graphics.Color
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.view.Gravity
import android.view.View
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import io.github.yx14ng.rnote.viewer.NewNotebookOptions.dp

/** What the "import PDF" screen chose; [firstPage] and [endPage] are 0-based, end excluded. */
internal class PdfChoice(
    val name: String,
    val folder: String?,
    val pageSize: PageSize,
    val firstPage: Int,
    val endPage: Int,
    val vector: Boolean,
)

/**
 * The "import PDF" screen: the file, the new notebook's name, folder and sheet width, which
 * pages and how they are kept (vector or image). [onImport] gets the choice.
 */
internal class ImportPdfScreen(
    private val context: Context,
    private val fileName: String,
    private val sizeBytes: Long,
    private val pageCount: Int,
    private val folders: List<String>,
    private var folder: String?,
    private var pageSize: PageSize,
    private val onImport: (PdfChoice) -> Unit,
) {
    private val dialog = Dialog(context, android.R.style.Theme_DeviceDefault_NoActionBar)
    private val name = EditText(context).apply { addTextChangedListener(changes()) }
    private val folderValue = NewNotebookOptions.dropDown(context) { chooseFolder() }
    private val widthValue = NewNotebookOptions.dropDown(context) { chooseWidth() }
    private val from = numberField("1")
    private val to = numberField(pageCount.toString())
    private val range = LinearLayout(context)
    private val importButton = TextView(context)
    private var allPages = true
    private var vector = true
    private lateinit var pagesChoice: (Int) -> Unit
    private val kinds = mutableListOf<View>()

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
                addView(fileSection())
                addView(optionsSection())
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
            setText(R.string.import_pdf_title)
            textSize = 20f
            setTextColor(Graphite.TEXT)
            gravity = Gravity.CENTER
        }, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
        addView(importButton.apply {
            setText(R.string.import_pdf_import)
            textSize = 17f
            setTextColor(Color.WHITE)
            gravity = Gravity.CENTER
            background = Graphite.pressable(context, Graphite.rounded(context, NewNotebookOptions.CHOSEN, 14f), 14f)
            setOnClickListener { confirm() }
        }, LinearLayout.LayoutParams(dp(context, 110f), dp(context, 48f)))
    }

    private fun fileSection(): View {
        val section = NewNotebookOptions.column(context, 28f, Graphite.RAISED)
        val page = PaperPreview(context).apply {
            paper = Color.WHITE
            line = Color.rgb(0xC8, 0xCC, 0xD6)
            template = Template.LINES
            spacingMm = 4.0
        }
        val fields = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            addView(caption(R.string.import_pdf_file))
            addView(TextView(context).apply {
                text = fileName
                textSize = 19f
                setTextColor(Graphite.TEXT)
                maxLines = 2
            })
            addView(TextView(context).apply {
                text = context.resources.getQuantityString(R.plurals.import_pdf_pages_size, pageCount, pageCount, megabytes())
                textSize = 15f
                setTextColor(Graphite.TEXT_SECONDARY)
                setPadding(0, dp(context, 4f), 0, dp(context, 10f))
            })
            addView(NewNotebookOptions.row(context, R.string.library_name, name.apply {
                setText(fileName.substringBeforeLast('.'))
                setTextColor(Graphite.TEXT)
                textSize = 16f
                isSingleLine = true
                background = null
                gravity = Gravity.END or Gravity.CENTER_VERTICAL
                minWidth = dp(context, 220f)
            }))
            addView(NewNotebookOptions.row(context, R.string.new_notebook_folder, folderValue))
            addView(NewNotebookOptions.row(context, R.string.import_pdf_width, widthValue))
            addView(caption(R.string.import_pdf_height_hint).apply { setPadding(0, dp(context, 8f), 0, 0) })
        }
        section.addView(LinearLayout(context).apply {
            addView(page, LinearLayout.LayoutParams(dp(context, 168f), dp(context, 238f)))
            addView(fields, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f).apply { marginStart = dp(context, 24f) })
        })
        return section
    }

    private fun optionsSection(): View {
        val section = NewNotebookOptions.column(context, 28f)
        val pages = NewNotebookOptions.choices(
            context,
            listOf(context.getString(R.string.import_pdf_all), context.getString(R.string.import_pdf_range)),
            0,
            segmented = true,
        ) { i -> allPages = i == 0; refresh() }
        pagesChoice = pages.second
        range.apply {
            gravity = Gravity.CENTER_VERTICAL
            addView(from, LinearLayout.LayoutParams(dp(context, 84f), dp(context, 48f)))
            addView(TextView(context).apply {
                setText(R.string.import_pdf_to)
                setTextColor(Graphite.TEXT_SECONDARY)
                setPadding(dp(context, 10f), 0, dp(context, 10f), 0)
            })
            addView(to, LinearLayout.LayoutParams(dp(context, 84f), dp(context, 48f)))
        }
        section.addView(LinearLayout(context).apply {
            gravity = Gravity.CENTER_VERTICAL
            addView(NewNotebookOptions.heading(context, R.string.new_notebook_pages))
            addView(pages.first, LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply { marginStart = dp(context, 18f) })
            addView(range, LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT).apply { marginStart = dp(context, 18f) })
        })
        section.addView(NewNotebookOptions.heading(context, R.string.import_pdf_kind), spaced(30f))
        section.addView(LinearLayout(context).apply {
            addView(kind(R.string.import_pdf_vector, R.string.import_pdf_vector_detail, recommended = true) { vector = true; refresh() },
                LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
            addView(kind(R.string.import_pdf_bitmap, R.string.import_pdf_bitmap_detail, recommended = false) { vector = false; refresh() },
                LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f).apply { marginStart = dp(context, 16f) })
        }, spaced(14f))
        section.addView(NewNotebookOptions.hint(context, R.string.import_pdf_eraser_hint), spaced(30f))
        return section
    }

    private fun kind(label: Int, detail: Int, recommended: Boolean, onPick: () -> Unit): View = LinearLayout(context).apply {
        orientation = LinearLayout.VERTICAL
        val pad = dp(context, 18f)
        setPadding(dp(context, 20f), pad, dp(context, 20f), pad)
        isClickable = true
        setOnClickListener { onPick() }
        addView(LinearLayout(context).apply {
            gravity = Gravity.CENTER_VERTICAL
            addView(TextView(context).apply {
                setText(label)
                textSize = 17f
                setTextColor(Graphite.TEXT)
                paint.isFakeBoldText = true
            })
            if (recommended) {
                addView(TextView(context).apply {
                    setText(R.string.import_pdf_recommended)
                    textSize = 12f
                    setTextColor(Color.rgb(0xA9, 0xCB, 0xF5))
                    gravity = Gravity.CENTER
                    setPadding(dp(context, 10f), 0, dp(context, 10f), 0)
                    background = Graphite.rounded(context, Color.rgb(0x2A, 0x3D, 0x5C), 12f)
                }, LinearLayout.LayoutParams(WRAP_CONTENT, dp(context, 24f)).apply { marginStart = dp(context, 10f) })
            }
        })
        addView(TextView(context).apply {
            setText(detail)
            textSize = 14f
            setTextColor(Graphite.TEXT_SECONDARY)
            setPadding(0, dp(context, 8f), 0, 0)
        })
        kinds += this
    }

    private fun refresh() {
        folderValue.text = folder ?: context.getString(R.string.library_uncategorized)
        widthValue.text = context.getString(pageSize.label)
        pagesChoice(if (allPages) 0 else 1)
        range.visibility = if (allPages) View.GONE else View.VISIBLE
        kinds.forEachIndexed { i, card ->
            val chosen = (i == 0) == vector
            card.background = Graphite.rounded(
                context,
                if (chosen) Color.rgb(0x1F, 0x2A, 0x3A) else Graphite.RAISED,
                16f,
                if (chosen) NewNotebookOptions.LINK else Graphite.OUTLINE,
            )
        }
        val valid = pages() != null && name.text.isNotBlank()
        importButton.isEnabled = valid
        importButton.alpha = if (valid) 1f else DISABLED_ALPHA
    }

    /** The 0-based pages chosen, end excluded, or null if the range typed isn't valid. */
    private fun pages(): IntRange? {
        if (allPages) return 0 until pageCount
        val first = from.text.toString().toIntOrNull() ?: return null
        val last = to.text.toString().toIntOrNull() ?: return null
        return if (first in 1..last && last <= pageCount) (first - 1) until last else null
    }

    private fun confirm() {
        val pages = pages() ?: return
        dialog.dismiss()
        onImport(PdfChoice(name.text.toString().trim(), folder, pageSize, pages.first, pages.last + 1, vector))
    }

    private fun chooseFolder() {
        val sorted = folders.sortedWith(String.CASE_INSENSITIVE_ORDER)
        val choices = listOf(context.getString(R.string.library_uncategorized)) + sorted
        AlertDialog.Builder(context)
            .setTitle(R.string.new_notebook_folder)
            .setItems(choices.toTypedArray()) { _, which -> folder = sorted.getOrNull(which - 1); refresh() }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun chooseWidth() {
        AlertDialog.Builder(context)
            .setTitle(R.string.import_pdf_width)
            .setItems(PageSize.entries.map { context.getString(it.label) }.toTypedArray()) { _, which ->
                pageSize = PageSize.entries[which]
                refresh()
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun numberField(value: String) = EditText(context).apply {
        setText(value)
        inputType = InputType.TYPE_CLASS_NUMBER
        isSingleLine = true
        gravity = Gravity.CENTER
        textSize = 17f
        setTextColor(Graphite.TEXT)
        background = Graphite.rounded(context, Graphite.SURFACE, 12f, Graphite.OUTLINE)
        addTextChangedListener(changes())
    }

    /** Checks the choice again on every edit of a field. */
    private fun changes() = object : TextWatcher {
        override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
        override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) = Unit
        override fun afterTextChanged(s: Editable?) {
            if (::pagesChoice.isInitialized) refresh()
        }
    }

    private fun caption(text: Int) = TextView(context).apply {
        setText(text)
        textSize = 13f
        setTextColor(Graphite.TEXT_SECONDARY)
        setPadding(0, 0, 0, dp(context, 6f))
    }

    private fun megabytes(): String = FormValues.formatNumber(Math.round(sizeBytes / 104857.6) / 10.0)

    private fun spaced(topDp: Float) = LinearLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT).apply { topMargin = dp(context, topDp) }

    private companion object {
        const val DISABLED_ALPHA = 0.4f
    }
}
