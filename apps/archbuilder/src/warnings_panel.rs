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
const HINT: Message = Message {
    message_id: "ab-warnings-hint",
};
const ERROR: Message = Message {
    message_id: "ab-warnings-severity-error",
};
const WARNING: Message = Message {
    message_id: "ab-warnings-severity-warning",
};
const KINDS: [(WarningKind, Message); 6] = [
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

/// The rectangle in plan around the elements `ids`, for framing them: walls by their footprint,
/// floors by their boundary, rooms by their point, stairs by their outline, doors and windows by
/// their place in their wall. `None` when none of them has a place.
#[must_use]
pub fn bounds(snapshot: &Snapshot, ids: &[ObjectId]) -> Option<([f64; 2], [f64; 2])> {
    let mut points: Vec<[f64; 2]> = Vec::new();
    for id in ids.iter().take(1000) {
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
            Some(types::DOOR | types::WINDOW) => {
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
    // A margin, so the elements do not touch the edge of the view.
    let margin = ((hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.25).max(1.0);
    Some((
        [lo[0] - margin, lo[1] - margin],
        [hi[0] + margin, hi[1] + margin],
    ))
}

/// One row as shown.
struct Row {
    kind: WarningKind,
    severity: Severity,
    error: arc_base::Error,
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
            kind: w.kind,
            severity: w.severity,
            error: w.error,
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
    /// The kind shown; every kind when `None`.
    filter: Option<WarningKind>,
}

impl Panel for WarningsPanel {
    fn show(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        let Some(tab) = cx.tab.and_then(|t| cx.app.tab(t).ok()) else {
            return;
        };
        let snapshot = tab.document().snapshot();
        if !self.rows.as_ref().is_some_and(|(s, _)| s.ptr_eq(&snapshot)) {
            let rows = register(&snapshot)
                .into_iter()
                .map(|w| Row::of(&snapshot, w))
                .collect();
            self.rows = Some((snapshot, rows));
        }
        let Some((_, rows)) = &self.rows else {
            return;
        };
        let shown: Vec<&Row> = rows
            .iter()
            .filter(|r| self.filter.is_none_or(|k| k == r.kind))
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
                    let args: Vec<(&str, arc_base::error::Value)> = row
                        .error
                        .arguments()
                        .iter()
                        .map(|(k, v)| (*k, v.clone()))
                        .collect();
                    let message = say(localize, row.error.message_id(), &args);
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
