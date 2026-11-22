//! The type editor: a panel that edits the types of the document. It shows the type of the
//! selected wall, floor, door or window (else the kind and type chosen in the panel), with its
//! name and, for a wall type, its function flags; for a wall or floor type a table of its layers
//! (function, material, thickness, core marker; add, remove, move up and down), the total
//! thickness and a section strip of the layers drawn to scale with the core boundary; for a door
//! or window type its size, frame, operation and sill. Duplicate, rename and delete work on the
//! shown type; deleting a type in use asks for the type its objects get instead.
//!
//! Edits are kept in a draft that is checked as it is typed, with a message that says what to
//! change, and become one `archbuilder.type.set` on Apply, so every object of the type changes
//! with it in one undo step.

use arc_app::shell::{Action, Panel, PanelContext};
use arc_base::ObjectId;
use arc_doc::Snapshot;
use arc_ui::egui::{
    self, Color32, ComboBox, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};
use arc_ui::{Intent, Localize};
use archbuilder_commands::element_type::{TYPE_DELETE, TYPE_DUPLICATE, TYPE_SET};
use archbuilder_model::element_type::{
    self as et, Exposure, Function, InsertType, Layer, Layers, TypeKind, WallType,
};
use archbuilder_model::insert::Operation;
use archbuilder_model::spatial;

/// A message of the application.
struct Message {
    message_id: &'static str,
}

const KIND_LABELS: [(TypeKind, Message); 4] = [
    (
        TypeKind::Wall,
        Message {
            message_id: "ab-types-kind-wall",
        },
    ),
    (
        TypeKind::Floor,
        Message {
            message_id: "ab-types-kind-floor",
        },
    ),
    (
        TypeKind::Door,
        Message {
            message_id: "ab-types-kind-door",
        },
    ),
    (
        TypeKind::Window,
        Message {
            message_id: "ab-types-kind-window",
        },
    ),
];
const FUNCTION_LABELS: [(Function, Message); 5] = [
    (
        Function::Structure,
        Message {
            message_id: "ab-layer-structure",
        },
    ),
    (
        Function::Substrate,
        Message {
            message_id: "ab-layer-substrate",
        },
    ),
    (
        Function::Thermal,
        Message {
            message_id: "ab-layer-thermal",
        },
    ),
    (
        Function::Membrane,
        Message {
            message_id: "ab-layer-membrane",
        },
    ),
    (
        Function::Finish,
        Message {
            message_id: "ab-layer-finish",
        },
    ),
];
const OPERATION_LABELS: [(Operation, Message); 5] = [
    (
        Operation::SingleSwing,
        Message {
            message_id: "ab-op-single-swing",
        },
    ),
    (
        Operation::DoubleSwing,
        Message {
            message_id: "ab-op-double-swing",
        },
    ),
    (
        Operation::Sliding,
        Message {
            message_id: "ab-op-sliding",
        },
    ),
    (
        Operation::Fixed,
        Message {
            message_id: "ab-op-fixed",
        },
    ),
    (
        Operation::Casement,
        Message {
            message_id: "ab-op-casement",
        },
    ),
];
const TYPE_LABEL: Message = Message {
    message_id: "ab-types-type",
};
const NAME: Message = Message {
    message_id: "ab-types-name",
};
const RENAME: Message = Message {
    message_id: "ab-types-rename",
};
const DUPLICATE: Message = Message {
    message_id: "ab-types-duplicate",
};
const DELETE: Message = Message {
    message_id: "ab-types-delete",
};
const REPLACE_WITH: Message = Message {
    message_id: "ab-types-replace-with",
};
const IN_USE: Message = Message {
    message_id: "ab-types-in-use",
};
const EXTERIOR: Message = Message {
    message_id: "ab-types-exterior",
};
const INTERIOR: Message = Message {
    message_id: "ab-types-interior",
};
const LOAD_BEARING: Message = Message {
    message_id: "ab-types-load-bearing",
};
const ROOM_BOUNDING: Message = Message {
    message_id: "ab-types-room-bounding",
};
const LAYERS_HEADING: Message = Message {
    message_id: "ab-types-layers",
};
const CORE: Message = Message {
    message_id: "ab-types-core",
};
const FUNCTION: Message = Message {
    message_id: "ab-types-function",
};
const MATERIAL: Message = Message {
    message_id: "ab-types-material",
};
const THICKNESS: Message = Message {
    message_id: "ab-types-thickness-mm",
};
const ADD_LAYER: Message = Message {
    message_id: "ab-types-add-layer",
};
const TOTAL: Message = Message {
    message_id: "ab-types-total",
};
const APPLY: Message = Message {
    message_id: "ab-types-apply",
};
const REVERT: Message = Message {
    message_id: "ab-types-revert",
};
const WIDTH: Message = Message {
    message_id: "ab-types-width-mm",
};
const HEIGHT: Message = Message {
    message_id: "ab-types-height-mm",
};
const FRAME_WIDTH: Message = Message {
    message_id: "ab-types-frame-width-mm",
};
const FRAME_DEPTH: Message = Message {
    message_id: "ab-types-frame-depth-mm",
};
const OPERATION: Message = Message {
    message_id: "ab-types-operation",
};
const SILL: Message = Message {
    message_id: "ab-types-sill-mm",
};
const NUMBER_INVALID: Message = Message {
    message_id: "ab-types-number-invalid",
};
const CORE_GAP: Message = Message {
    message_id: "ab-types-core-gap",
};
const EXTERIOR_SIDE: Message = Message {
    message_id: "ab-types-exterior-side",
};
const TOP_SIDE: Message = Message {
    message_id: "ab-types-top-side",
};
const EMPTY: Message = Message {
    message_id: "ab-types-empty",
};

fn say(
    localize: &dyn Localize,
    message: &Message,
    args: &[(&str, arc_base::error::Value)],
) -> String {
    localize
        .message(message.message_id, args)
        .unwrap_or_else(|| message.message_id.to_owned())
}

/// One row of the layer table being edited: the texts as typed.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    function: Function,
    material: String,
    thickness: String,
    core: bool,
}

/// The values of the shown type as edited.
#[derive(Debug, Clone, PartialEq)]
struct Draft {
    id: ObjectId,
    name: String,
    rows: Vec<Row>,
    exposure: Exposure,
    load_bearing: bool,
    room_bounding: bool,
    sizes: [String; 5],
    operation: Operation,
}

fn mm(meters: f64) -> String {
    let v = (meters * 1000.0 * 100.0).round() / 100.0;
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v}")
    }
}

/// Millimeters typed as text, with a comma or a point as the decimal separator.
fn meters(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| v / 1000.0)
}

impl Draft {
    fn of(snapshot: &Snapshot, kind: TypeKind, id: ObjectId) -> Option<Self> {
        let object = spatial::get(snapshot, id)?;
        let mut draft = Self {
            id,
            name: et::name(&object).to_owned(),
            rows: Vec::new(),
            exposure: Exposure::Exterior,
            load_bearing: false,
            room_bounding: true,
            sizes: Default::default(),
            operation: Operation::SingleSwing,
        };
        if kind.layered() {
            let layers = Layers::from_values(&object.values)?;
            draft.rows = layers
                .layers
                .iter()
                .enumerate()
                .map(|(k, l)| Row {
                    function: l.function,
                    material: l.material.clone(),
                    thickness: mm(l.thickness),
                    core: (layers.core.0..layers.core.1).contains(&k),
                })
                .collect();
            if let Some(w) = WallType::from_values(&object.values) {
                draft.exposure = w.exposure;
                draft.load_bearing = w.load_bearing;
                draft.room_bounding = w.room_bounding;
            }
        } else {
            let t = InsertType::from_values(&object.values)?;
            draft.sizes = [t.width, t.height, t.frame_width, t.frame_depth, t.sill].map(mm);
            draft.operation = t.operation;
        }
        Some(draft)
    }

    /// The layers of the table, or the reason they cannot be applied.
    fn layers(&self, localize: &dyn Localize) -> Result<Layers, String> {
        let mut layers = Vec::with_capacity(self.rows.len());
        for (k, row) in self.rows.iter().enumerate() {
            let thickness = meters(&row.thickness).ok_or_else(|| {
                say(
                    localize,
                    &NUMBER_INVALID,
                    &[(
                        "layer",
                        arc_base::error::Value::Int(i64::try_from(k + 1).unwrap_or(0)),
                    )],
                )
            })?;
            layers.push(Layer::new(row.function, &row.material, thickness));
        }
        let marked: Vec<usize> = (0..self.rows.len())
            .filter(|k| self.rows[*k].core)
            .collect();
        let core = match (marked.first(), marked.last()) {
            (Some(&a), Some(&b)) if b - a + 1 == marked.len() => (a, b + 1),
            (Some(_), Some(_)) => return Err(say(localize, &CORE_GAP, &[])),
            _ => (0, 0),
        };
        let layers = Layers { layers, core };
        match layers.problems().first() {
            Some(error) => {
                let args: Vec<(&str, arc_base::error::Value)> = error
                    .arguments()
                    .iter()
                    .map(|(k, v)| (*k, v.clone()))
                    .collect();
                Err(localize
                    .message(error.message_id(), &args)
                    .unwrap_or_else(|| error.code().to_owned()))
            }
            None => Ok(layers),
        }
    }

    /// The parameters of `archbuilder.type.set` for the draft, or the reason it cannot apply.
    fn params(&self, kind: TypeKind, localize: &dyn Localize) -> Result<serde_json::Value, String> {
        let mut params = serde_json::json!({"type": self.id.to_string(), "name": self.name.trim()});
        let Some(map) = params.as_object_mut() else {
            return Err(String::new());
        };
        if kind.layered() {
            let layers = self.layers(localize)?;
            map.insert(
                "layers".into(),
                layers
                    .layers
                    .iter()
                    .map(|l| {
                        serde_json::json!({"function": l.function.name(), "material": l.material, "thickness": l.thickness})
                    })
                    .collect(),
            );
            map.insert(
                "core".into(),
                serde_json::json!([layers.core.0, layers.core.1]),
            );
            if kind == TypeKind::Wall {
                map.insert("exposure".into(), self.exposure.name().into());
                map.insert("load_bearing".into(), self.load_bearing.into());
                map.insert("room_bounding".into(), self.room_bounding.into());
            }
        } else {
            for (name, text) in ["width", "height", "frame_width", "frame_depth", "sill"]
                .iter()
                .zip(&self.sizes)
            {
                let value = meters(text).filter(|v| *v >= 0.0).ok_or_else(|| {
                    say(
                        localize,
                        &NUMBER_INVALID,
                        &[("layer", arc_base::error::Value::Int(0))],
                    )
                })?;
                map.insert((*name).into(), value.into());
            }
            map.insert("operation".into(), self.operation.name().into());
        }
        Ok(params)
    }
}

/// The color of a layer in the section strip.
fn color(function: Function) -> Color32 {
    match function {
        Function::Structure => Color32::from_rgb(150, 150, 160),
        Function::Substrate => Color32::from_rgb(190, 170, 130),
        Function::Thermal => Color32::from_rgb(230, 210, 90),
        Function::Membrane => Color32::from_rgb(70, 140, 220),
        Function::Finish => Color32::from_rgb(215, 215, 205),
    }
}

/// The section strip: the layers to scale across the panel's width, the core boundary as two
/// red lines, a membrane as a blue line.
fn strip(ui: &mut Ui, rows: &[Row], localize: &dyn Localize, kind: TypeKind) {
    let thick: Vec<f64> = rows
        .iter()
        .map(|r| meters(&r.thickness).unwrap_or(0.0).max(0.0))
        .collect();
    let total: f64 = thick.iter().sum();
    let side = if kind == TypeKind::Wall {
        &EXTERIOR_SIDE
    } else {
        &TOP_SIDE
    };
    ui.weak(say(localize, side, &[]));
    let width = ui.available_width().max(60.0);
    let (response, painter) = ui.allocate_painter(Vec2::new(width, 46.0), Sense::hover());
    let rect = response.rect;
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::GRAY),
        StrokeKind::Inside,
    );
    if total <= 0.0 {
        return;
    }
    let scale = f64::from(rect.width()) / total;
    let mut x = rect.left();
    let mut core_lines = Vec::new();
    for (k, (row, t)) in rows.iter().zip(&thick).enumerate() {
        #[allow(clippy::cast_possible_truncation, reason = "pixels")]
        let w = (t * scale) as f32;
        let start = x;
        if row.function == Function::Membrane {
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                Stroke::new(2.0, color(Function::Membrane)),
            );
        } else {
            let cell =
                Rect::from_min_max(egui::pos2(x, rect.top()), egui::pos2(x + w, rect.bottom()));
            painter.rect_filled(cell, 0.0, color(row.function));
            painter.rect_stroke(
                cell,
                0.0,
                Stroke::new(1.0, Color32::from_gray(60)),
                StrokeKind::Inside,
            );
        }
        x += w;
        let core_before = k > 0 && rows[k - 1].core;
        if row.core && !core_before {
            core_lines.push(start);
        }
        let core_after = rows.get(k + 1).is_some_and(|r| r.core);
        if row.core && !core_after {
            core_lines.push(x);
        }
    }
    for x in core_lines {
        painter.line_segment(
            [
                egui::pos2(x, rect.top() - 3.0),
                egui::pos2(x, rect.bottom() + 3.0),
            ],
            Stroke::new(2.5, Color32::from_rgb(220, 50, 50)),
        );
    }
}

/// The type editor panel.
#[derive(Default)]
pub struct TypePanel {
    kind: Option<TypeKind>,
    chosen: Option<ObjectId>,
    /// The draft as it was read and as edited.
    draft: Option<(Draft, Draft)>,
    replacement: Option<ObjectId>,
    /// The selection the panel last followed, so a new selection shows its type.
    followed: Vec<ObjectId>,
    /// After Duplicate: the kind and the types it had, so the panel shows the copy once it exists.
    copying: Option<(TypeKind, Vec<ObjectId>)>,
}

fn combo_text<T: PartialEq + Copy>(
    items: &[(T, Message)],
    value: T,
    localize: &dyn Localize,
) -> String {
    items
        .iter()
        .find(|(v, _)| *v == value)
        .map(|(_, m)| say(localize, m, &[]))
        .unwrap_or_default()
}

impl Panel for TypePanel {
    fn show(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        self.body(ui, cx);
    }
}

impl TypePanel {
    #[allow(clippy::too_many_lines, reason = "the panel's rows, top to bottom")]
    fn body(&mut self, ui: &mut Ui, cx: &mut PanelContext<'_>) {
        let (Some(view), Some(tab)) = (cx.view.as_deref(), cx.tab) else {
            return;
        };
        let Ok(tab) = cx.app.tab(tab) else {
            return;
        };
        let snapshot = tab.document().snapshot();
        let localize = cx.localize;
        // A new selection shows the type of its first wall, floor, door or window.
        if view.selection() != self.followed.as_slice() {
            self.followed = view.selection().to_vec();
            if let Some((kind, Some(id))) = crate::tool_type::shown(&snapshot, view.selection()) {
                self.kind = Some(kind);
                self.chosen = Some(id);
            }
        }
        // The copy a Duplicate made is the type shown next, so a rename edits the copy.
        if let Some((kind, before)) = &self.copying {
            let copy = et::of_kind(&snapshot, *kind)
                .into_iter()
                .find(|t| !before.contains(&t.id));
            if let Some(copy) = copy {
                self.kind = Some(*kind);
                self.chosen = Some(copy.id);
                self.copying = None;
                self.draft = None;
            }
        }
        let kind = self.kind.unwrap_or(TypeKind::Wall);
        let all = et::of_kind(&snapshot, kind);
        let id = self
            .chosen
            .filter(|id| all.iter().any(|t| t.id == *id))
            .or_else(|| all.first().map(|t| t.id));
        let mut actions: Vec<Intent> = Vec::new();
        ui.horizontal(|ui| {
            ComboBox::from_id_salt("archbuilder-types-kind")
                .selected_text(combo_text(&KIND_LABELS, kind, localize))
                .show_ui(ui, |ui| {
                    for (k, m) in &KIND_LABELS {
                        if ui
                            .selectable_label(*k == kind, say(localize, m, &[]))
                            .clicked()
                        {
                            self.kind = Some(*k);
                            self.chosen = None;
                        }
                    }
                });
            ui.label(say(localize, &TYPE_LABEL, &[]));
            let current = all
                .iter()
                .find(|t| Some(t.id) == id)
                .map(|t| et::name(t).to_owned())
                .unwrap_or_default();
            ComboBox::from_id_salt("archbuilder-types-type")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for t in &all {
                        if ui.selectable_label(Some(t.id) == id, et::name(t)).clicked() {
                            self.chosen = Some(t.id);
                        }
                    }
                });
        });
        let Some(id) = id else {
            ui.weak(say(localize, &EMPTY, &[]));
            return;
        };
        // The draft follows the document until it is edited.
        let Some(fresh) = Draft::of(&snapshot, kind, id) else {
            return;
        };
        let follow = self
            .draft
            .as_ref()
            .is_none_or(|(base, draft)| base.id != id || (draft == base && *base != fresh));
        if follow {
            self.draft = Some((fresh.clone(), fresh.clone()));
        }
        let Some((base, draft)) = self.draft.as_mut() else {
            return;
        };
        let base = base.clone();
        let users = et::instances(&snapshot, kind, id).len();
        let mut copying = None;
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(say(localize, &NAME, &[]));
            ui.add(TextEdit::singleline(&mut draft.name).desired_width(180.0));
            if ui.button(say(localize, &RENAME, &[])).clicked() {
                actions.push(
                    Intent::new(TYPE_SET)
                        .with("type", id.to_string())
                        .with("name", draft.name.trim()),
                );
            }
            if ui.button(say(localize, &DUPLICATE, &[])).clicked() {
                actions.push(Intent::new(TYPE_DUPLICATE).with("type", id.to_string()));
                copying = Some((kind, all.iter().map(|t| t.id).collect()));
            }
        });
        ui.horizontal(|ui| {
            let count = arc_base::error::Value::Int(i64::try_from(users).unwrap_or(0));
            ui.weak(say(localize, &IN_USE, &[("count", count)]));
            let others: Vec<_> = all.iter().filter(|t| t.id != id).collect();
            if users > 0 && !others.is_empty() {
                ui.label(say(localize, &REPLACE_WITH, &[]));
                let replacement = self
                    .replacement
                    .filter(|r| others.iter().any(|t| t.id == *r))
                    .unwrap_or(others[0].id);
                let name = others
                    .iter()
                    .find(|t| t.id == replacement)
                    .map(|t| et::name(t).to_owned())
                    .unwrap_or_default();
                ComboBox::from_id_salt("archbuilder-types-replacement")
                    .selected_text(name)
                    .show_ui(ui, |ui| {
                        for t in &others {
                            if ui
                                .selectable_label(t.id == replacement, et::name(t))
                                .clicked()
                            {
                                self.replacement = Some(t.id);
                            }
                        }
                    });
                if ui.button(say(localize, &DELETE, &[])).clicked() {
                    actions.push(
                        Intent::new(TYPE_DELETE)
                            .with("type", id.to_string())
                            .with("replacement", replacement.to_string()),
                    );
                }
            } else if ui.button(say(localize, &DELETE, &[])).clicked() {
                actions.push(Intent::new(TYPE_DELETE).with("type", id.to_string()));
            }
        });
        // Checked on the draft as edited so far; Apply stays in view above the details.
        let edited = base != *draft;
        let params = draft.params(kind, localize);
        if let Err(reason) = &params {
            ui.colored_label(Color32::from_rgb(230, 90, 80), reason);
        }
        ui.horizontal(|ui| {
            let apply = ui.add_enabled(
                edited && params.is_ok(),
                egui::Button::new(say(localize, &APPLY, &[])),
            );
            if apply.clicked()
                && let Ok(serde_json::Value::Object(map)) = &params
            {
                let mut intent = Intent::new(TYPE_SET);
                intent.params = map.clone();
                actions.push(intent);
            }
            if ui
                .add_enabled(edited, egui::Button::new(say(localize, &REVERT, &[])))
                .clicked()
            {
                *draft = base.clone();
            }
        });
        ui.separator();
        // The details scroll; the name and the actions above stay in view.
        egui::ScrollArea::both()
            .id_salt("archbuilder-types-details")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if kind == TypeKind::Wall {
                    ui.horizontal(|ui| {
                        ui.radio_value(
                            &mut draft.exposure,
                            Exposure::Exterior,
                            say(localize, &EXTERIOR, &[]),
                        );
                        ui.radio_value(
                            &mut draft.exposure,
                            Exposure::Interior,
                            say(localize, &INTERIOR, &[]),
                        );
                        ui.checkbox(&mut draft.load_bearing, say(localize, &LOAD_BEARING, &[]));
                        ui.checkbox(&mut draft.room_bounding, say(localize, &ROOM_BOUNDING, &[]));
                    });
                }
                if kind.layered() {
                    ui.label(RichText::new(say(localize, &LAYERS_HEADING, &[])).strong());
                    let mut swap: Option<(usize, usize)> = None;
                    let mut remove: Option<usize> = None;
                    egui::Grid::new("archbuilder-types-layers")
                        .striped(true)
                        .num_columns(7)
                        .show(ui, |ui| {
                            ui.label(say(localize, &CORE, &[]));
                            ui.label(say(localize, &FUNCTION, &[]));
                            ui.label(say(localize, &MATERIAL, &[]));
                            ui.label(say(localize, &THICKNESS, &[]));
                            ui.label("");
                            ui.label("");
                            ui.label("");
                            ui.end_row();
                            let count = draft.rows.len();
                            for (k, row) in draft.rows.iter_mut().enumerate() {
                                ui.checkbox(&mut row.core, "");
                                ComboBox::from_id_salt(("archbuilder-types-function", k))
                                    .selected_text(combo_text(
                                        &FUNCTION_LABELS,
                                        row.function,
                                        localize,
                                    ))
                                    .show_ui(ui, |ui| {
                                        for (f, m) in &FUNCTION_LABELS {
                                            ui.selectable_value(
                                                &mut row.function,
                                                *f,
                                                say(localize, m, &[]),
                                            );
                                        }
                                    });
                                ui.add_sized(
                                    [150.0, 20.0],
                                    TextEdit::singleline(&mut row.material),
                                );
                                ui.add_sized(
                                    [60.0, 20.0],
                                    TextEdit::singleline(&mut row.thickness),
                                );
                                if ui.add_enabled(k > 0, egui::Button::new("↑")).clicked() {
                                    swap = Some((k - 1, k));
                                }
                                if ui
                                    .add_enabled(k + 1 < count, egui::Button::new("↓"))
                                    .clicked()
                                {
                                    swap = Some((k, k + 1));
                                }
                                if ui.add_enabled(count > 1, egui::Button::new("✕")).clicked() {
                                    remove = Some(k);
                                }
                                ui.end_row();
                            }
                        });
                    if let Some((a, b)) = swap {
                        draft.rows.swap(a, b);
                    }
                    if let Some(k) = remove {
                        draft.rows.remove(k);
                    }
                    if ui
                        .add_enabled(
                            draft.rows.len() < et::MAX_LAYERS,
                            egui::Button::new(say(localize, &ADD_LAYER, &[])),
                        )
                        .clicked()
                    {
                        draft.rows.push(Row {
                            function: Function::Finish,
                            material: String::new(),
                            thickness: "10".to_owned(),
                            core: false,
                        });
                    }
                    let total: f64 = draft.rows.iter().filter_map(|r| meters(&r.thickness)).sum();
                    ui.label(say(localize, &TOTAL, &[("mm", mm(total).into())]));
                    strip(ui, &draft.rows, localize, kind);
                } else {
                    let labels = [&WIDTH, &HEIGHT, &FRAME_WIDTH, &FRAME_DEPTH, &SILL];
                    egui::Grid::new("archbuilder-types-sizes")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for (label, text) in labels.iter().zip(draft.sizes.iter_mut()) {
                                ui.label(say(localize, label, &[]));
                                ui.add(TextEdit::singleline(text).desired_width(70.0));
                                ui.end_row();
                            }
                            ui.label(say(localize, &OPERATION, &[]));
                            let insert = kind.insert();
                            ComboBox::from_id_salt("archbuilder-types-operation")
                                .selected_text(combo_text(
                                    &OPERATION_LABELS,
                                    draft.operation,
                                    localize,
                                ))
                                .show_ui(ui, |ui| {
                                    for (o, m) in &OPERATION_LABELS {
                                        if insert.is_some_and(|i| o.allowed(i)) {
                                            ui.selectable_value(
                                                &mut draft.operation,
                                                *o,
                                                say(localize, m, &[]),
                                            );
                                        }
                                    }
                                });
                            ui.end_row();
                        });
                }
            });
        if !actions.is_empty() {
            // After an edit the draft follows the document again.
            self.draft = None;
        }
        if copying.is_some() {
            self.copying = copying;
        }
        cx.actions.extend(actions.into_iter().map(Action::Intent));
    }
}
