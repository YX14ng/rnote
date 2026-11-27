package io.github.yx14ng.rnote.viewer

import android.content.Context
import android.view.View
import android.widget.LinearLayout

/**
 * The menu, in sections: edit actions as large buttons on top, then file, document and
 * input. Opened from the top bar's menu icon; each row runs an [EditorMenu.Action].
 */
internal object MenuPanel {
    fun build(context: Context, state: MenuState, onAction: (EditorMenu.Action) -> Unit): View {
        val panel = PanelViews.column(context)
        val open = state.documentOpen

        if (open) {
            val edit = LinearLayout(context).apply { orientation = LinearLayout.HORIZONTAL }
            for ((icon, label, action) in listOf(
                Triple(R.drawable.ic_cut, R.string.action_cut, EditorMenu.Action.Cut),
                Triple(R.drawable.ic_copy, R.string.action_copy, EditorMenu.Action.Copy),
                Triple(R.drawable.ic_paste, R.string.action_paste, EditorMenu.Action.Paste),
            )) {
                val button = PanelViews.bigButton(context, icon, label) { onAction(action) }
                button.background = Graphite.pressable(
                    context,
                    Graphite.rounded(context, Graphite.RAISED, Graphite.BUTTON_RADIUS_DP),
                    Graphite.BUTTON_RADIUS_DP,
                )
                val gap = Graphite.dp(context, 3f)
                edit.addView(button, LinearLayout.LayoutParams(0, Graphite.dp(context, 60f), 1f).apply {
                    setMargins(gap, gap, gap, gap)
                })
            }
            panel.addView(edit)
            panel.addView(PanelViews.divider(context), PanelViews.dividerParams(context))
        }

        panel.addView(Graphite.sectionTitle(context, R.string.section_file))
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_new, R.string.action_new, "Ctrl+N", true) {
            onAction(EditorMenu.Action.New)
        })
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_open, R.string.action_open, "Ctrl+O", true) {
            onAction(EditorMenu.Action.Open)
        })
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_save, R.string.action_save, "Ctrl+S", open) {
            onAction(EditorMenu.Action.Save)
        })
        val saveAs = if (state.inLibrary) R.string.action_save_copy else R.string.action_save_as
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_save_as, saveAs, null, open) {
            onAction(EditorMenu.Action.SaveAs)
        })
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_page, R.string.action_export_pdf, null, open) {
            onAction(EditorMenu.Action.ExportPdf)
        })
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_share, R.string.action_share_pdf, null, open) {
            onAction(EditorMenu.Action.SharePdf)
        })

        if (open) {
            panel.addView(PanelViews.divider(context), PanelViews.dividerParams(context))
            panel.addView(Graphite.sectionTitle(context, R.string.section_document))
            panel.addView(PanelViews.menuRow(context, R.drawable.ic_page, R.string.page_and_background, null, true) {
                onAction(EditorMenu.Action.Document)
            })
            panel.addView(PanelViews.menuRow(context, R.drawable.ic_list, R.string.pages_menu, null, state.hasPages) {
                onAction(EditorMenu.Action.Pages)
            })
            panel.addView(PanelViews.menuRow(context, R.drawable.ic_text_format, R.string.action_text_format, null, true) {
                onAction(EditorMenu.Action.TextFormat)
            })
            panel.addView(PanelViews.menuRow(context, R.drawable.ic_image, R.string.action_insert_image, null, true) {
                onAction(EditorMenu.Action.InsertImage)
            })
            panel.addView(PanelViews.menuRow(context, R.drawable.ic_camera, R.string.action_take_photo, null, true) {
                onAction(EditorMenu.Action.TakePhoto)
            })

            panel.addView(PanelViews.divider(context), PanelViews.dividerParams(context))
            panel.addView(Graphite.sectionTitle(context, R.string.section_input))
            panel.addView(PanelViews.switchRow(context, R.string.action_finger_drawing, null, state.settings.fingerDrawing) {
                onAction(EditorMenu.Action.ToggleFingerDrawing)
            })
            panel.addView(PanelViews.switchRow(context, R.string.shape_recognition_short, R.string.shape_recognition_hint,
                state.settings.shapeRecognition) { onAction(EditorMenu.Action.ToggleShapeRecognition) })
            // The choice shows where shortcuts go; each tap moves to the next one.
            val penButton = context.getString(state.settings.penButton.label)
            panel.addView(PanelViews.menuRow(context, R.drawable.ic_pen_brush, R.string.pen_button, penButton, true) {
                onAction(EditorMenu.Action.NextPenButton)
            })
        }
        return panel
    }

    /** How wide the panel is. */
    const val PANEL_WIDTH_DP = 320f
}
