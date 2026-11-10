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
        val panel = PanelViews.column(context, widthDp = PANEL_WIDTH_DP)
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
        panel.addView(PanelViews.menuRow(context, R.drawable.ic_save_as, R.string.action_save_as, null, open) {
            onAction(EditorMenu.Action.SaveAs)
        })

        if (open) {
            panel.addView(PanelViews.divider(context), PanelViews.dividerParams(context))
            panel.addView(Graphite.sectionTitle(context, R.string.section_document))
            panel.addView(PanelViews.menuRow(context, R.drawable.ic_page, R.string.page_and_background, null, true) {
                onAction(EditorMenu.Action.Document)
            })
            panel.addView(PanelViews.menuRow(context, R.drawable.ic_text_format, R.string.action_text_format, null, true) {
                onAction(EditorMenu.Action.TextFormat)
            })

            panel.addView(PanelViews.divider(context), PanelViews.dividerParams(context))
            panel.addView(Graphite.sectionTitle(context, R.string.section_input))
            panel.addView(PanelViews.switchRow(context, R.string.action_finger_drawing, null, state.settings.fingerDrawing) {
                onAction(EditorMenu.Action.ToggleFingerDrawing)
            })
            panel.addView(PanelViews.switchRow(context, R.string.shape_recognition_short, R.string.shape_recognition_hint,
                state.settings.shapeRecognition) { onAction(EditorMenu.Action.ToggleShapeRecognition) })
        }
        return panel
    }

    private const val PANEL_WIDTH_DP = 320f
}
