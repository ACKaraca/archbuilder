//! The viewport panel with a header that names what it shows: the 3D view, or the plan view with
//! its scale and detail level, or the saved 3D view, section or elevation with its scale. The viewport itself is the shell's.

use arc_app::shell::{Panel, PanelContext, ViewportPanel};
use arc_ui::egui::Ui;

/// A message of the application.
struct Message {
    message_id: &'static str,
}

const HEADER_3D: Message = Message {
    message_id: "ab-viewport-3d",
};
const HEADER_MODEL: Message = Message {
    message_id: "ab-viewport-model",
};
const HEADER_PLAN: Message = Message {
    message_id: "ab-viewport-plan",
};

/// The viewport with its header.
pub struct HeaderViewport {
    inner: ViewportPanel,
}

impl HeaderViewport {
    /// A viewport whose clicks select through `select`.
    #[must_use]
    pub fn new(select: &str) -> Self {
        let mut inner = ViewportPanel::new();
        inner.commands.select = select.into();
        Self { inner }
    }
}

impl Panel for HeaderViewport {
    fn show(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        let text = cx.tab.and_then(|tab| {
            let snapshot = cx.app.tab(tab).ok()?.document().snapshot();
            let id = crate::plan_state::current_in(tab, &snapshot)?;
            let settings = archbuilder_model::view::settings_of(
                &snapshot,
                archbuilder_model::spatial::get(&snapshot, id)?.as_ref(),
            )?;
            let args = [
                ("name", arc_base::error::Value::Text(settings.name)),
                (
                    "scale",
                    arc_base::error::Value::Text(format!("{:.0}", settings.scale)),
                ),
                (
                    "detail",
                    arc_base::error::Value::Text(settings.detail.name().to_owned()),
                ),
            ];
            cx.localize.message(HEADER_PLAN.message_id, &args)
        });
        // A saved 3D view, section or elevation is named with its scale.
        let text = text.or_else(|| {
            let tab = cx.tab?;
            let snapshot = cx.app.tab(tab).ok()?.document().snapshot();
            let id = crate::plan_state::model_in(tab, &snapshot)?;
            let settings = archbuilder_model::model_view::settings(&snapshot, id)?;
            let args = [
                ("name", arc_base::error::Value::Text(settings.name)),
                (
                    "scale",
                    arc_base::error::Value::Text(format!("{:.0}", settings.scale)),
                ),
            ];
            cx.localize.message(HEADER_MODEL.message_id, &args)
        });
        let text = text.unwrap_or_else(|| {
            cx.localize
                .message(HEADER_3D.message_id, &[])
                .unwrap_or_else(|| HEADER_3D.message_id.to_owned())
        });
        ui.horizontal(|ui| {
            ui.strong(text);
        });
        self.inner.show(ui, cx);
    }
}
