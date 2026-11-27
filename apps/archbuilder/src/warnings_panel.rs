//! The warnings panel: the model's warnings register
//! ([`archbuilder_model::validate::register`]) of the active document, one row per finding with its
//! severity, its message and the elements it involves. The list is recomputed whenever the
//! document changes, so a problem that was fixed disappears; a filter shows one kind. A double
//! click on a row selects its elements and frames them in the view
//! ([`crate::shortcuts::ui::ZOOM_TO`]).

use arc_app::shell::{Action, Panel, PanelContext};
use arc_base::ObjectId;
use arc_doc::{Severity, Snapshot};
use arc_ui::egui::{self, Color32, RichText, Ui};
use arc_ui::{Intent, Localize};
use archbuilder_model::spatial;
use archbuilder_model::types;
use archbuilder_model::validate::{Warning, WarningKind, register};
use archbuilder_model::wall::Wall;

use crate::shortcuts::ui;

/// A message of the application.
struct Message {
    message_id: &'static str,
}

const COUNT: Message = Message {
    message_id: "ab-warnings-count",
};
const FILTERED: Message = Message {
    message_id: "ab-warnings-count-filtered",
};
const NONE: Message = Message {
    message_id: "ab-warnings-none",
};
const ALL: Message = Message {
    message_id: "ab-warnings-kind-all",
};
const VIEW_FAILED: Message = Message {
    message_id: "ab-warnings-view-failed",
};
const VIEW_LEFT_OUT: Message = Message {
    message_id: "ab-warnings-view-left-out",
};
const HINT: Message = Message {
    message_id: "ab-warnings-hint",
};
const ERROR: Message = Message {
    message_id: "ab-warnings-severity-error",
};
const WARNING: Message = Message {
    message_id: "ab-warnings-severity-warning",
};
const KINDS: [(WarningKind, Message); 7] = [
    (
        WarningKind::Stair,
        Message {
            message_id: "ab-warnings-kind-stair",
        },
    ),
    (
        WarningKind::Room,
        Message {
            message_id: "ab-warnings-kind-room",
        },
    ),
    (
        WarningKind::Wall,
        Message {
            message_id: "ab-warnings-kind-wall",
        },
    ),
    (
        WarningKind::Floor,
        Message {
            message_id: "ab-warnings-kind-floor",
        },
    ),
    (
        WarningKind::Ceiling,
        Message {
            message_id: "ab-warnings-kind-ceiling",
        },
    ),
    (
        WarningKind::Insert,
        Message {
            message_id: "ab-warnings-kind-insert",
        },
    ),
    (
        WarningKind::Type,
        Message {
            message_id: "ab-warnings-kind-type",
        },
    ),
];

fn say(localize: &dyn Localize, id: &str, args: &[(&str, arc_base::error::Value)]) -> String {
    localize.message(id, args).unwrap_or_else(|| id.to_owned())
}

fn kind_label(kind: WarningKind) -> &'static str {
    KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(ALL.message_id, |(_, m)| m.message_id)
}

/// The rectangle in plan around the elements `ids` with a margin, for framing them (see
/// [`extent`]).
#[must_use]
pub fn bounds(snapshot: &Snapshot, ids: &[ObjectId]) -> Option<([f64; 2], [f64; 2])> {
    let (lo, hi) = extent(snapshot, ids)?;
    // A margin, so the elements do not touch the edge of the view.
    let margin = ((hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.25).max(1.0);
    Some((
        [lo[0] - margin, lo[1] - margin],
        [hi[0] + margin, hi[1] + margin],
    ))
}

/// The most elements [`extent`] measures.
const MAX_MEASURED: usize = 20_000;

/// The rectangle in plan around the elements `ids`: walls by their footprint, floors by their
/// boundary, rooms by their point, stairs by their outline, doors and windows by their place in
/// their wall. `None` when none of them has a place.
#[must_use]
pub fn extent(snapshot: &Snapshot, ids: &[ObjectId]) -> Option<([f64; 2], [f64; 2])> {
    let mut points: Vec<[f64; 2]> = Vec::new();
    for id in ids.iter().take(MAX_MEASURED) {
        let Some(object) = spatial::get(snapshot, *id) else {
            continue;
        };
        match spatial::type_of(snapshot, *id).as_deref() {
            Some(types::WALL) => {
                if let Some(w) = Wall::from_values(&object.values) {
                    points.extend(w.plain_footprint());
                }
            }
            Some(types::FLOOR) => {
                if let Some(b) = archbuilder_model::floor::Floor::from_values(&object.values)
                    .and_then(|f| f.boundary_in(snapshot))
                {
                    points.extend(b.loops.into_iter().flatten());
                }
            }
            Some(types::ROOM) => {
                if let Some(r) = archbuilder_model::room::Room::from_values(&object.values) {
                    points.push(r.point);
                }
            }
            Some(types::STAIR) => {
                if let Some(p) = archbuilder_model::stair::Placed::of(snapshot, &object) {
                    points.extend(p.outline());
                }
            }
            Some(types::ROOF) => {
                if let Ok(d) = archbuilder_model::roof::Derived::of(snapshot, &object) {
                    points.extend(d.surface.outline.iter().copied());
                }
            }
            Some(types::ROOF_EXTRUSION) => {
                if let Ok(d) = archbuilder_model::roof_extrusion::Derived::of(snapshot, &object) {
                    points.extend(d.pieces().iter().flat_map(|p| p.points.iter().copied()));
                }
            }
            Some(types::CEILING) => {
                if let Some(b) = archbuilder_model::ceiling::Ceiling::from_values(&object.values)
                    .and_then(|c| c.shape.boundary_in(snapshot))
                {
                    points.extend(b.loops.into_iter().flatten());
                }
            }
            Some(types::SHAFT) => {
                if let Some(s) = archbuilder_model::opening::shaft_span(snapshot, &object) {
                    points.extend(s.outline);
                }
            }
            Some(types::VERTICAL_OPENING) => {
                if let Some(o) = archbuilder_model::opening::outline_of(
                    snapshot,
                    &object,
                    archbuilder_model::floor::VERTICAL_SLOTS,
                ) {
                    points.extend(o);
                }
            }
            Some(types::DOOR | types::WINDOW | types::WALL_OPENING) => {
                let host = object
                    .header
                    .owner
                    .and_then(|w| spatial::get(snapshot, w))
                    .and_then(|w| Wall::from_values(&w.values));
                let item = archbuilder_model::insert::Insert::of(snapshot, &object);
                if let (Some(w), Some(item)) = (host, item) {
                    let length = w.length().max(1e-9);
                    let along = |d: f64| {
                        [
                            w.start[0] + (w.end[0] - w.start[0]) * d / length,
                            w.start[1] + (w.end[1] - w.start[1]) * d / length,
                        ]
                    };
                    let (low, high) = item.extent();
                    points.extend([along(low), along(high)]);
                }
            }
            _ => {}
        }
    }
    let first = *points.first()?;
    let (mut lo, mut hi) = (first, first);
    for p in points {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    Some((lo, hi))
}

/// One row as shown.
struct Row {
    /// The kind of a finding of the model; `None` for a diagnostic of the open plan view's drawing,
    /// listed under every kind.
    kind: Option<WarningKind>,
    severity: Severity,
    message_id: &'static str,
    args: Vec<(&'static str, arc_base::error::Value)>,
    objects: Vec<ObjectId>,
    names: String,
}

impl Row {
    fn of(snapshot: &Snapshot, w: Warning) -> Self {
        let names = w
            .objects
            .iter()
            .filter_map(|id| spatial::get(snapshot, *id))
            .filter_map(|o| types::text(&o.values, 0).map(str::to_owned))
            .collect::<Vec<_>>()
            .join(", ");
        Self {
            kind: Some(w.kind),
            severity: w.severity,
            message_id: w.error.message_id(),
            args: arguments(&w.error),
            objects: w.objects,
            names,
        }
    }
}

/// The panel.
#[derive(Default)]
pub struct WarningsPanel {
    /// The rows of the snapshot they were found in.
    rows: Option<(Snapshot, Vec<Row>)>,
    /// The rows of the open plan view's drawing, with the drawing they came from.
    view_rows: Option<(std::sync::Arc<archbuilder_model::plan::Built>, Vec<Row>)>,
    /// The kind shown; every kind when `None`.
    filter: Option<WarningKind>,
}

impl WarningsPanel {
    /// The rows of the model's findings, found again when the document changed, and the rows of
    /// the open plan view's drawing. The drawing is built on the extraction workers:
    /// its diagnostics come when it does (a failed build is an error, an element it left out a
    /// warning).
    fn refresh(&mut self, tab: Option<arc_app::application::TabId>, snapshot: &Snapshot) {
        if !self.rows.as_ref().is_some_and(|(s, _)| s.ptr_eq(snapshot)) {
            let rows = register(snapshot)
                .into_iter()
                .map(|w| Row::of(snapshot, w))
                .collect();
            self.rows = Some((snapshot.clone(), rows));
        }
        let view = tab.and_then(|t| crate::plan_state::current_in(t, snapshot));
        let built = view.and_then(|v| crate::plan_state::peek(snapshot, v));
        let fresh = match (&self.view_rows, &built) {
            (Some((old, _)), Some(new)) => {
                std::sync::Arc::ptr_eq(&old.geometry, &new.geometry)
                    && std::sync::Arc::ptr_eq(&old.notes, &new.notes)
            }
            (None, None) => true,
            _ => false,
        };
        if !fresh {
            self.view_rows = built.zip(view).map(|(built, view)| {
                let rows = view_rows(snapshot, view, &built);
                (built, rows)
            });
        }
    }
}

impl Panel for WarningsPanel {
    fn show(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        let Some(tab) = cx.tab.and_then(|t| cx.app.tab(t).ok()) else {
            return;
        };
        let snapshot = tab.document().snapshot();
        self.refresh(cx.tab, &snapshot);
        let Some((_, rows)) = &self.rows else {
            return;
        };
        let view_rows = self
            .view_rows
            .as_ref()
            .map_or(&[][..], |(_, r)| r.as_slice());
        let rows: Vec<&Row> = view_rows.iter().chain(rows).collect();
        let shown: Vec<&Row> = rows
            .iter()
            .copied()
            .filter(|r| self.filter.is_none() || self.filter == r.kind)
            .collect();
        let localize = cx.localize;
        ui.horizontal(|ui| {
            let total = i64::try_from(rows.len()).unwrap_or(i64::MAX);
            let heading = if self.filter.is_some() {
                let seen = i64::try_from(shown.len()).unwrap_or(i64::MAX);
                say(
                    localize,
                    FILTERED.message_id,
                    &[
                        ("shown", arc_base::error::Value::Int(seen)),
                        ("count", arc_base::error::Value::Int(total)),
                    ],
                )
            } else {
                say(
                    localize,
                    COUNT.message_id,
                    &[("count", arc_base::error::Value::Int(total))],
                )
            };
            ui.strong(heading);
            let current = say(
                localize,
                self.filter.map_or(ALL.message_id, kind_label),
                &[],
            );
            egui::ComboBox::from_id_salt("ab-warnings-filter")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filter, None, say(localize, ALL.message_id, &[]));
                    for (kind, label) in &KINDS {
                        ui.selectable_value(
                            &mut self.filter,
                            Some(*kind),
                            say(localize, label.message_id, &[]),
                        );
                    }
                });
        });
        if shown.is_empty() {
            ui.label(say(localize, NONE.message_id, &[]));
            return;
        }
        ui.label(RichText::new(say(localize, HINT.message_id, &[])).weak());
        let mut zoom = None;
        egui::ScrollArea::vertical()
            .id_salt("ab-warnings-rows")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in shown {
                    let (label, color) = if row.severity == Severity::Error {
                        (ERROR.message_id, Color32::from_rgb(0xe0, 0x50, 0x50))
                    } else {
                        (WARNING.message_id, Color32::from_rgb(0xe0, 0xb0, 0x30))
                    };
                    let message = say(localize, row.message_id, &row.args);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(say(localize, label, &[])).color(color));
                        let text = if row.names.is_empty() {
                            message
                        } else {
                            format!("{message}  [{}]", row.names)
                        };
                        let response = ui.selectable_label(false, text);
                        if response.double_clicked() {
                            zoom = Some(row.objects.clone());
                        }
                    });
                }
            });
        if let Some(objects) = zoom.filter(|o| !o.is_empty()) {
            cx.actions.push(Action::Start(Intent::new(ui::ZOOM_TO).with(
                "objects",
                objects.iter().map(ToString::to_string).collect::<Vec<_>>(),
            )));
        }
    }
}

/// The rows of the diagnostics of the drawing of the plan view `view`.
fn view_rows(
    snapshot: &Snapshot,
    view: ObjectId,
    built: &archbuilder_model::plan::Built,
) -> Vec<Row> {
    built
        .diagnostics()
        .into_iter()
        .take(archbuilder_model::validate::MAX_WARNINGS)
        .map(|d| {
            let failed = d.object == view;
            let names = spatial::get(snapshot, d.object)
                .and_then(|o| types::text(&o.values, 0).map(str::to_owned))
                .unwrap_or_default();
            Row {
                kind: None,
                severity: if failed {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                // The drawing's own message catalog is not the application's: its stable code
                // is named in a message of ours.
                message_id: if failed {
                    VIEW_FAILED.message_id
                } else {
                    VIEW_LEFT_OUT.message_id
                },
                args: vec![("code", d.error.code().into())],
                objects: if failed { Vec::new() } else { vec![d.object] },
                names,
            }
        })
        .collect()
}

/// The arguments of `error`'s message.
fn arguments(error: &arc_base::Error) -> Vec<(&'static str, arc_base::error::Value)> {
    error
        .arguments()
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect()
}
