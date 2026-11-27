//! Curtain walls: walls whose type is a curtain type, laid out as a grid of panels and mullions.
//!
//! A curtain wall is a wall record ([`crate::wall`]) whose wall type carries the curtain fields
//! ([`CurtainType`]): a grid rule for the vertical lines (which divide the baseline) and one for the
//! horizontal lines (which divide the height), the system panel that fills a cell, and three
//! mullion specifications (interior, border and corner). The wall record adds the only per-instance
//! authored data: the grid edits ([`GridEdit`]: lines added, moved or removed, whole or in a
//! segment) and the per-cell panel overrides ([`CellOverride`]).
//!
//! Nothing computed is stored. [`Derived`] reads the record and its type and derives the lines,
//! the cells, the panels and the mullions, and [`Derived::solids`] their solids. Every line has a
//! key that does not depend on its position (a generated line is named by its rule and index, an
//! added line by its edit), every cell has a key made of the keys of the four lines that bound it,
//! and every panel and mullion has a key made of those. The identifier of a generated child is
//! [`child_id`] of the wall and the key, so it is the same after every regeneration. An override
//! or an edit whose line or cell does not exist any more is kept and reported
//! ([`Layout::orphans`]), never rebound to a neighbor.
//!
//! A curtain wall inside a basic wall cuts an opening in it ([`embedded`]): derived, so it follows
//! either wall when it moves.

use arc_base::id::Identifier;
use arc_base::math::{Point2, Point3, Vector3};
use arc_base::{Error, Limits, ObjectId, Result, Tolerance};
use arc_doc::{
    Bound, Cardinality, EnumVariant, FieldDecl, Finding, Object, OnDelete, Severity, Snapshot,
    TypeKey, Value, ValueType,
};
use arc_geom::{PlaneFrame, Polyhedron, extrude, prism};

use crate::element_type::{InsertType, TypeKind, type_of};
use crate::error::codes;
use crate::floor::Boundary;
use crate::insert::{Facing, Hand, Insert, Kind, Operation, Role};
use crate::spatial;
use crate::types::{self, DOOR_TYPE, WINDOW_TYPE, wall};
use crate::wall::Wall;

/// The most interior lines of one direction a grid has.
pub const MAX_LINES: usize = 150;
/// The most grid edits one wall holds.
pub const MAX_EDITS: usize = 1_000;
/// The most panel overrides one wall holds.
pub const MAX_OVERRIDES: usize = 2_000;
/// Lines closer than this to each other or to a border are one line, in meters.
const TOL: f64 = 1.0e-3;
/// The chord of the prisms; every outline is straight.
const CHORD: f64 = 0.01;
/// The segments of a circular mullion's outline.
const CIRCLE: usize = 16;
/// The role of the panels of a curtain wall in [`child_id`].
pub const PANEL_ROLE: u32 = 0x1_0020;
/// The role of the mullions of a curtain wall in [`child_id`].
pub const MULLION_ROLE: u32 = 0x1_0021;

coded! {
    /// How a direction of the grid is divided.
    RuleKind {
        /// Lines at a fixed distance from each other.
        #[default]
        FixedDistance = 0, "fixed_distance";
        /// A fixed number of equally spaced interior lines.
        FixedNumber = 1, "fixed_number";
        /// Equally spaced lines, as few as keep every cell at most the value wide.
        MaxSpacing = 2, "max_spacing";
        /// Equally spaced lines, as many as keep every cell at least the value wide.
        MinSpacing = 3, "min_spacing";
    }
}

coded! {
    /// Where a fixed-distance grid starts counting.
    Justify {
        /// From the start of the baseline or the base.
        #[default]
        Start = 0, "start";
        /// Outward from the middle.
        Center = 1, "center";
        /// From the end of the baseline or the top.
        End = 2, "end";
    }
}

coded! {
    /// What fills a cell.
    PanelKind {
        /// A glass pane.
        #[default]
        Glazed = 0, "glazed";
        /// An opaque panel.
        Solid = 1, "solid";
        /// Nothing: an open cell.
        Empty = 2, "empty";
        /// A door that fills the cell (override only).
        Door = 3, "door";
        /// A window that fills the cell (override only).
        Window = 4, "window";
    }
}

coded! {
    /// The section of a mullion.
    Profile {
        /// No mullion at this position.
        #[default]
        None = 0, "none";
        /// A rectangle, width across the grid line and depth across the wall.
        Rectangular = 1, "rectangular";
        /// A circle whose diameter is the width.
        Circular = 2, "circular";
    }
}

coded! {
    /// Which direction's mullions run through a crossing.
    Priority {
        /// The vertical mullions are continuous.
        #[default]
        Vertical = 0, "vertical";
        /// The horizontal mullions are continuous.
        Horizontal = 1, "horizontal";
    }
}

coded! {
    /// The direction of a grid line.
    Direction {
        /// A vertical line: it divides the baseline.
        #[default]
        Vertical = 0, "vertical";
        /// A horizontal line: it divides the height.
        Horizontal = 1, "horizontal";
    }
}

coded! {
    /// What a grid edit does.
    EditKind {
        /// Adds a line.
        #[default]
        Add = 0, "add";
        /// Removes a line, or a segment of it.
        Remove = 1, "remove";
        /// Moves a line: the line stays where it is put when the rule changes.
        Move = 2, "move";
    }
}

/// Field positions of `archbuilder.wall_type` that belong to the curtain part, appended in schema
/// 1.2 after [`types::layered_type`]'s own.
pub mod slot {
    /// Whether the type is a curtain type.
    pub const CURTAIN: usize = 9;
    /// The vertical grid rule: kind, value, justification and offset (four fields).
    pub const VERTICAL: usize = 10;
    /// The horizontal grid rule (four fields).
    pub const HORIZONTAL: usize = 14;
    /// The system panel.
    pub const PANEL: usize = 18;
    /// The panel's thickness.
    pub const PANEL_THICKNESS: usize = 19;
    /// The panel's material.
    pub const PANEL_MATERIAL: usize = 20;
    /// The interior mullion: profile, width and depth (three fields).
    pub const INTERIOR: usize = 21;
    /// The border mullion (three fields).
    pub const BORDER: usize = 24;
    /// The corner mullion (three fields).
    pub const CORNER: usize = 27;
    /// Which direction's mullions are continuous.
    pub const PRIORITY: usize = 30;
}

/// Field positions of one grid edit record.
pub mod grid_edit {
    /// The key of the edit, unique in its wall.
    pub const KEY: usize = 0;
    /// The direction.
    pub const DIRECTION: usize = 1;
    /// What the edit does.
    pub const KIND: usize = 2;
    /// The key of the line a move or a removal is about.
    pub const LINE: usize = 3;
    /// The position of an added line or the destination of a moved one.
    pub const AT: usize = 4;
    /// The start of the segment the edit is limited to, along the other axis.
    pub const FROM: usize = 5;
    /// The end of the segment.
    pub const UNTIL: usize = 6;
}

/// Field positions of one cell override record.
pub mod cell_override {
    /// The key of the cell.
    pub const CELL: usize = 0;
    /// What fills the cell.
    pub const PANEL: usize = 1;
    /// The door or window type of a door or window panel.
    pub const TYPE: usize = 2;
}

// ----- authored data ----------------------------------------------------------------------------

/// How one direction of the grid is laid out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridRule {
    /// The kind of rule.
    pub kind: RuleKind,
    /// A distance in meters, or the number of lines for [`RuleKind::FixedNumber`].
    pub value: f64,
    /// Where a fixed distance starts counting.
    pub justify: Justify,
    /// The shift of the origin of a fixed distance, in meters.
    pub offset: f64,
}

impl GridRule {
    /// The rule of the four fields at `at`.
    fn read(values: &[Option<Value>], at: usize) -> Self {
        Self {
            kind: RuleKind::of_field(values, at),
            value: match values.get(at + 1) {
                Some(Some(Value::F64(v))) => *v,
                _ => 1.0,
            },
            justify: Justify::of_field(values, at + 2),
            offset: types::meters(values, at + 3).unwrap_or(0.0),
        }
    }

    fn write(&self) -> [Option<Value>; 4] {
        [
            Some(Value::Enum(self.kind.code())),
            Some(Value::F64(self.value)),
            Some(Value::Enum(self.justify.code())),
            Some(types::length(self.offset)),
        ]
    }

    /// Whether the rule can lay lines out: a positive finite value, and at most [`MAX_LINES`] for
    /// a fixed number.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.value.is_finite()
            && self.value > 0.0
            && self.offset.is_finite()
            && (self.kind != RuleKind::FixedNumber || self.value <= MAX_LINES as f64)
    }

    /// The interior lines the rule gives over `extent` meters: key and position, from the start.
    #[must_use]
    pub fn lines(&self, extent: f64) -> Vec<(String, f64)> {
        let mut out: Vec<(String, f64)> = Vec::new();
        if !(extent > 2.0 * TOL && self.valid()) {
            return out;
        }
        let inside = |p: f64| p > TOL && p < extent - TOL;
        let equal = |count: usize| -> Vec<(String, f64)> {
            (1..=count)
                .map(|i| (format!("n{i}"), extent * i as f64 / (count + 1) as f64))
                .collect()
        };
        match self.kind {
            RuleKind::FixedDistance => {
                let (prefix, origin, sign) = match self.justify {
                    Justify::Start => ("s", self.offset, 1.0),
                    Justify::End => ("e", extent - self.offset, -1.0),
                    Justify::Center => ("c", extent / 2.0 + self.offset, 1.0),
                };
                let negative = if self.justify == Justify::Center {
                    -(MAX_LINES as i64)
                } else {
                    0
                };
                for k in negative..=(MAX_LINES as i64) {
                    let p = origin + sign * self.value * k as f64;
                    if inside(p) {
                        out.push((format!("{prefix}{k}"), p));
                    }
                }
            }
            RuleKind::FixedNumber => out = equal(count_of(self.value.round())),
            RuleKind::MaxSpacing => {
                out = equal(count_of((extent / self.value - 1.0e-9).ceil() - 1.0));
            }
            RuleKind::MinSpacing => {
                out = equal(count_of((extent / self.value + 1.0e-9).floor() - 1.0));
            }
        }
        out.sort_by(|a, b| a.1.total_cmp(&b.1));
        out.truncate(MAX_LINES);
        out
    }
}

/// A number of lines from a rounded value, at most [`MAX_LINES`].
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to the range of the line count first"
)]
fn count_of(v: f64) -> usize {
    if v.is_finite() {
        v.clamp(0.0, MAX_LINES as f64) as usize
    } else {
        0
    }
}

/// The section of the mullions at one kind of position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MullionSpec {
    /// The section.
    pub profile: Profile,
    /// The width across the grid line, or the diameter, in meters.
    pub width: f64,
    /// The depth across the wall, in meters.
    pub depth: f64,
}

impl MullionSpec {
    fn read(values: &[Option<Value>], at: usize) -> Self {
        Self {
            profile: Profile::of_field(values, at),
            width: types::meters(values, at + 1).unwrap_or(0.05),
            depth: types::meters(values, at + 2).unwrap_or(0.1),
        }
    }

    fn write(&self) -> [Option<Value>; 3] {
        [
            Some(Value::Enum(self.profile.code())),
            Some(types::length(self.width)),
            Some(types::length(self.depth)),
        ]
    }

    /// The width the mullion takes in the grid; zero without one.
    fn extent(&self) -> f64 {
        if self.profile == Profile::None {
            0.0
        } else {
            self.width
        }
    }
}

/// The curtain part of a wall type.
#[derive(Debug, Clone, PartialEq)]
pub struct CurtainType {
    /// The rule of the vertical lines.
    pub vertical: GridRule,
    /// The rule of the horizontal lines.
    pub horizontal: GridRule,
    /// The system panel of a cell without an override; glazed, solid or empty.
    pub panel: PanelKind,
    /// The thickness of a glazed or solid panel, in meters.
    pub panel_thickness: f64,
    /// The material of the panel.
    pub material: String,
    /// The mullion on the lines between cells.
    pub interior: MullionSpec,
    /// The mullion on the four borders.
    pub border: MullionSpec,
    /// The mullion where two curtain walls meet at a corner.
    pub corner: MullionSpec,
    /// Which direction's mullions are continuous at a crossing.
    pub priority: Priority,
}

impl CurtainType {
    /// The default curtain type: 1.5 m by 3 m cells of glass in 50 mm mullions.
    #[must_use]
    pub fn standard() -> Self {
        let mullion = MullionSpec {
            profile: Profile::Rectangular,
            width: 0.05,
            depth: 0.15,
        };
        Self {
            vertical: GridRule {
                kind: RuleKind::FixedDistance,
                value: 1.5,
                justify: Justify::Start,
                offset: 0.0,
            },
            horizontal: GridRule {
                kind: RuleKind::FixedDistance,
                value: 3.0,
                justify: Justify::Start,
                offset: 0.0,
            },
            panel: PanelKind::Glazed,
            panel_thickness: 0.024,
            material: "Glass".to_owned(),
            interior: mullion,
            border: mullion,
            corner: MullionSpec {
                width: 0.1,
                ..mullion
            },
            priority: Priority::Vertical,
        }
    }

    /// The curtain type of a wall type record's values; `None` for a type that is not one.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        if !matches!(values.get(slot::CURTAIN), Some(Some(Value::Bool(true)))) {
            return None;
        }
        Some(Self {
            vertical: GridRule::read(values, slot::VERTICAL),
            horizontal: GridRule::read(values, slot::HORIZONTAL),
            panel: PanelKind::of_field(values, slot::PANEL),
            panel_thickness: types::meters(values, slot::PANEL_THICKNESS).unwrap_or(0.024),
            material: types::text(values, slot::PANEL_MATERIAL)
                .unwrap_or_default()
                .to_owned(),
            interior: MullionSpec::read(values, slot::INTERIOR),
            border: MullionSpec::read(values, slot::BORDER),
            corner: MullionSpec::read(values, slot::CORNER),
            priority: Priority::of_field(values, slot::PRIORITY),
        })
    }

    /// The values of the curtain fields, from [`slot::CURTAIN`] on.
    #[must_use]
    pub fn values(&self) -> Vec<Option<Value>> {
        let mut out = vec![Some(Value::Bool(true))];
        out.extend(self.vertical.write());
        out.extend(self.horizontal.write());
        out.extend([
            Some(Value::Enum(self.panel.code())),
            Some(types::length(self.panel_thickness)),
            Some(Value::String(self.material.clone())),
        ]);
        out.extend(self.interior.write());
        out.extend(self.border.write());
        out.extend(self.corner.write());
        out.push(Some(Value::Enum(self.priority.code())));
        out
    }

    /// The deepest mullion: the thickness the type's layers give the wall.
    #[must_use]
    pub fn depth(&self) -> f64 {
        [self.interior, self.border, self.corner]
            .iter()
            .filter(|m| m.profile != Profile::None)
            .map(|m| m.depth)
            .fold(self.panel_thickness, f64::max)
    }

    /// What breaks the rules of the type: an unusable grid rule, a panel that is a door or a
    /// window, a mullion without a size.
    #[must_use]
    pub fn problems(&self) -> Vec<Error> {
        let mut out = Vec::new();
        if !self.vertical.valid() || !self.horizontal.valid() {
            out.push(Error::new(codes::CURTAIN_RULE));
        }
        if matches!(self.panel, PanelKind::Door | PanelKind::Window) {
            out.push(Error::new(codes::CURTAIN_PANEL));
        }
        let bad = |m: &MullionSpec| {
            m.profile != Profile::None
                && !(m.width.is_finite()
                    && m.width > 0.0
                    && m.width <= 1.0
                    && m.depth.is_finite()
                    && m.depth > 0.0
                    && m.depth <= types::THICKNESS_RANGE.1)
        };
        if [self.interior, self.border, self.corner].iter().any(bad)
            || !(self.panel_thickness.is_finite()
                && self.panel_thickness > 0.0
                && self.panel_thickness <= 1.0)
        {
            out.push(Error::new(codes::CURTAIN_MULLION));
        }
        out
    }
}

/// The curtain fields appended to a wall type in schema 1.2.
#[must_use]
pub fn type_fields() -> Vec<FieldDecl> {
    let variants = |items: Vec<(&'static str, u32)>| {
        ValueType::Enum(
            items
                .into_iter()
                .map(|(n, c)| EnumVariant::new(n, c))
                .collect(),
        )
    };
    let coded_field = |name: String, items: Vec<(&'static str, u32)>| {
        FieldDecl::new(&name, variants(items))
            .optional()
            .in_aspect("curtain")
            .since_minor(2)
    };
    let length = |name: String, max: f64| {
        types::length_field(&name, Some(types::real((0.0, max))))
            .optional()
            .in_aspect("curtain")
            .since_minor(2)
    };
    let rule = |prefix: &str| {
        vec![
            coded_field(
                format!("{prefix}_rule"),
                RuleKind::ALL.iter().map(|k| (k.name(), k.code())).collect(),
            ),
            FieldDecl::new(&format!("{prefix}_value"), ValueType::F64)
                .optional()
                .with_bound(Bound::Real {
                    min: 0.0,
                    max: 100_000.0,
                })
                .in_aspect("curtain")
                .since_minor(2),
            coded_field(
                format!("{prefix}_justify"),
                Justify::ALL.iter().map(|k| (k.name(), k.code())).collect(),
            ),
            types::length_field(
                &format!("{prefix}_offset"),
                Some(types::real((-1000.0, 1000.0))),
            )
            .optional()
            .in_aspect("curtain")
            .since_minor(2),
        ]
    };
    let mullion = |prefix: &str| {
        vec![
            coded_field(
                format!("{prefix}_profile"),
                Profile::ALL.iter().map(|k| (k.name(), k.code())).collect(),
            ),
            length(format!("{prefix}_width"), 1.0),
            length(format!("{prefix}_depth"), types::THICKNESS_RANGE.1),
        ]
    };
    let mut fields = vec![
        FieldDecl::new("curtain", ValueType::Bool)
            .optional()
            .in_aspect("curtain")
            .since_minor(2),
    ];
    fields.extend(rule("vertical"));
    fields.extend(rule("horizontal"));
    fields.push(coded_field(
        "panel".to_owned(),
        PanelKind::ALL
            .iter()
            .map(|k| (k.name(), k.code()))
            .collect(),
    ));
    fields.push(length("panel_thickness".to_owned(), 1.0));
    fields.push(
        FieldDecl::new("panel_material", ValueType::String)
            .optional()
            .with_bound(Bound::Length {
                max: types::NAME_BYTES,
            })
            .in_aspect("curtain")
            .since_minor(2),
    );
    fields.extend(mullion("interior"));
    fields.extend(mullion("border"));
    fields.extend(mullion("corner"));
    fields.push(coded_field(
        "priority".to_owned(),
        Priority::ALL.iter().map(|k| (k.name(), k.code())).collect(),
    ));
    fields
}

/// The findings of a wall type record that is a curtain type.
#[must_use]
pub fn type_findings(values: &[Option<Value>]) -> Vec<Finding> {
    CurtainType::from_values(values)
        .map(|t| {
            t.problems()
                .into_iter()
                .map(|error| Finding {
                    severity: Severity::Error,
                    error,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// One authored edit of a curtain wall's grid.
#[derive(Debug, Clone, PartialEq)]
pub struct GridEdit {
    /// The key of the edit, unique in its wall.
    pub key: i64,
    /// The direction of the line.
    pub direction: Direction,
    /// What the edit does.
    pub kind: EditKind,
    /// The key of the line a move or removal is about.
    pub line: String,
    /// The position of an added line or the destination of a moved one, in meters from the
    /// start of the baseline (vertical) or from the base (horizontal).
    pub at: f64,
    /// The segment the edit is limited to, along the other axis, when it is.
    pub span: Option<(f64, f64)>,
}

impl GridEdit {
    fn read(fields: &[Option<Value>]) -> Option<Self> {
        let key = match fields.get(grid_edit::KEY)? {
            Some(Value::I64(k)) => *k,
            _ => return None,
        };
        let from = types::meters(fields, grid_edit::FROM);
        let until = types::meters(fields, grid_edit::UNTIL);
        Some(Self {
            key,
            direction: Direction::of_field(fields, grid_edit::DIRECTION),
            kind: EditKind::of_field(fields, grid_edit::KIND),
            line: types::text(fields, grid_edit::LINE)
                .unwrap_or_default()
                .to_owned(),
            at: types::meters(fields, grid_edit::AT).unwrap_or(0.0),
            span: from.zip(until).map(|(a, b)| (a.min(b), a.max(b))),
        })
    }

    /// The record value of the edit.
    #[must_use]
    pub fn value(&self) -> Value {
        Value::Record(vec![
            Some(Value::I64(self.key)),
            Some(Value::Enum(self.direction.code())),
            Some(Value::Enum(self.kind.code())),
            Some(Value::String(self.line.clone())),
            Some(types::length(self.at)),
            self.span.map(|s| types::length(s.0)),
            self.span.map(|s| types::length(s.1)),
        ])
    }

    /// The edits of a wall record's values.
    #[must_use]
    pub fn list(values: &[Option<Value>]) -> Vec<Self> {
        match values.get(wall::GRID_EDITS) {
            Some(Some(Value::List(items))) => items
                .iter()
                .take(MAX_EDITS)
                .filter_map(|i| match i {
                    Value::Record(f) => Self::read(f),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The list value of `edits`.
    #[must_use]
    pub fn list_value(edits: &[Self]) -> Value {
        Value::List(edits.iter().map(Self::value).collect())
    }
}

/// The per-cell choice of a panel, kept by the cell's key.
#[derive(Debug, Clone, PartialEq)]
pub struct CellOverride {
    /// The key of the cell.
    pub cell: String,
    /// What fills the cell.
    pub panel: PanelKind,
    /// The door or window type of a door or window panel.
    pub type_id: Option<ObjectId>,
}

impl CellOverride {
    fn read(fields: &[Option<Value>]) -> Option<Self> {
        let cell = types::text(fields, cell_override::CELL)?.to_owned();
        Some(Self {
            cell,
            panel: PanelKind::of_field(fields, cell_override::PANEL),
            type_id: match fields.get(cell_override::TYPE) {
                Some(Some(Value::Reference(id))) => Some(*id),
                _ => None,
            },
        })
    }

    /// The record value of the override.
    #[must_use]
    pub fn value(&self) -> Value {
        Value::Record(vec![
            Some(Value::String(self.cell.clone())),
            Some(Value::Enum(self.panel.code())),
            self.type_id.map(Value::Reference),
        ])
    }

    /// The overrides of a wall record's values.
    #[must_use]
    pub fn list(values: &[Option<Value>]) -> Vec<Self> {
        match values.get(wall::CELL_OVERRIDES) {
            Some(Some(Value::List(items))) => items
                .iter()
                .take(MAX_OVERRIDES)
                .filter_map(|i| match i {
                    Value::Record(f) => Self::read(f),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The list value of `overrides`.
    #[must_use]
    pub fn list_value(overrides: &[Self]) -> Value {
        Value::List(overrides.iter().map(Self::value).collect())
    }
}

/// The field declaration of the grid edits of a wall, appended in schema 1.`minor`.
#[must_use]
pub fn grid_edits_field(minor: u32) -> FieldDecl {
    let variants = |items: Vec<(&'static str, u32)>| {
        ValueType::Enum(
            items
                .into_iter()
                .map(|(n, c)| EnumVariant::new(n, c))
                .collect(),
        )
    };
    let optional_length = |name: &str| {
        FieldDecl::new(name, ValueType::Quantity(arc_base::Dimension::LENGTH))
            .optional()
            .with_bound(types::real(types::COORDINATE_RANGE))
    };
    FieldDecl::new(
        "grid_edits",
        ValueType::List(Box::new(ValueType::Record(vec![
            FieldDecl::new("key", ValueType::I64).with_default(Value::I64(0)),
            FieldDecl::new(
                "direction",
                variants(
                    Direction::ALL
                        .iter()
                        .map(|d| (d.name(), d.code()))
                        .collect(),
                ),
            )
            .with_default(Value::Enum(0)),
            FieldDecl::new(
                "kind",
                variants(EditKind::ALL.iter().map(|d| (d.name(), d.code())).collect()),
            )
            .with_default(Value::Enum(0)),
            FieldDecl::new("line", ValueType::String)
                .with_default(Value::String(String::new()))
                .with_bound(Bound::Length { max: 128 }),
            types::length_field("at", Some(types::real(types::COORDINATE_RANGE))),
            optional_length("from"),
            optional_length("until"),
        ]))),
    )
    .with_bound(Bound::Length {
        max: MAX_EDITS as u64,
    })
    .optional()
    .in_aspect("curtain")
    .since_minor(minor)
}

/// The field declaration of the panel overrides of a wall, appended in schema 1.`minor`.
#[must_use]
pub fn cell_overrides_field(minor: u32) -> FieldDecl {
    let mut spec = types::reference_to(DOOR_TYPE);
    spec.targets = [DOOR_TYPE, WINDOW_TYPE]
        .iter()
        .filter_map(|k| TypeKey::new(*k).ok())
        .collect();
    spec.rule.on_delete = OnDelete::Clear;
    spec.rule.cardinality = Cardinality::Optional;
    FieldDecl::new(
        "cell_overrides",
        ValueType::List(Box::new(ValueType::Record(vec![
            FieldDecl::new("cell", ValueType::String)
                .with_default(Value::String(String::new()))
                .with_bound(Bound::Length { max: 128 }),
            FieldDecl::new(
                "panel",
                ValueType::Enum(
                    PanelKind::ALL
                        .iter()
                        .map(|k| EnumVariant::new(k.name(), k.code()))
                        .collect(),
                ),
            )
            .with_default(Value::Enum(0)),
            FieldDecl::new("type", ValueType::Reference(spec)).optional(),
        ]))),
    )
    .with_bound(Bound::Length {
        max: MAX_OVERRIDES as u64,
    })
    .optional()
    .in_aspect("curtain")
    .since_minor(minor)
}

// ----- identity ---------------------------------------------------------------------------------

/// The identifier of the generated child of `parent` named `key` in `role`: a function of the two
/// alone, so a panel or mullion has the same identifier after every regeneration. A version 8
/// identifier, which never equals an object's own.
#[must_use]
pub fn child_id(parent: ObjectId, role: u32, key: &str) -> u128 {
    fn mix(mut x: u64) -> u64 {
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^ (x >> 31)
    }
    let base = parent.as_u128();
    let (high, low) = (
        u64::try_from(base >> 64).unwrap_or(0),
        u64::try_from(base & u128::from(u64::MAX)).unwrap_or(0),
    );
    let mut h = mix(high ^ u64::from(role));
    for b in key.bytes() {
        h = mix(h ^ u64::from(b)).rotate_left(7);
    }
    let first = mix(h ^ low);
    let second = mix(first ^ low.rotate_left(32) ^ u64::from(role));
    let raw = (u128::from(first) << 64) | u128::from(second);
    (raw & !(0xF << 76) & !(0b11 << 62)) | (0x8 << 76) | (0b10 << 62)
}

// ----- the grid ---------------------------------------------------------------------------------

/// One grid line: its key, position and the stretches along it where it is absent or present.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The key, independent of the position.
    pub key: String,
    /// The position: along the baseline for a vertical line, above the base for a horizontal one.
    pub at: f64,
    /// Whether the line is a border of the wall.
    pub border: bool,
    /// Where the line is removed, as ranges along the other axis.
    pub removed: Vec<(f64, f64)>,
    /// The range an added line is limited to.
    pub only: Option<(f64, f64)>,
}

impl Line {
    /// Whether the line is present at `t` along the other axis.
    #[must_use]
    pub fn present_at(&self, t: f64) -> bool {
        self.border
            || (self.only.is_none_or(|(a, b)| a <= t && t <= b)
                && !self.removed.iter().any(|(a, b)| *a <= t && t <= *b))
    }
}

/// The lines of one direction over `extent`, borders included: the rule's lines changed by the
/// edits of `direction`, in order. The keys of the edits that found no line are added to
/// `orphans`.
fn lines(
    rule: &GridRule,
    edits: &[GridEdit],
    direction: Direction,
    extent: f64,
    border_keys: (&str, &str),
    orphans: &mut Vec<i64>,
) -> Vec<Line> {
    let mut found: Vec<Line> = rule
        .lines(extent)
        .into_iter()
        .map(|(key, at)| Line {
            key,
            at,
            border: false,
            removed: Vec::new(),
            only: None,
        })
        .collect();
    for edit in edits.iter().filter(|e| e.direction == direction) {
        match edit.kind {
            EditKind::Add => found.push(Line {
                key: format!("a{}", edit.key),
                at: edit.at,
                border: false,
                removed: Vec::new(),
                only: edit.span,
            }),
            EditKind::Move => match found.iter_mut().find(|l| l.key == edit.line) {
                Some(line) => line.at = edit.at,
                None => orphans.push(edit.key),
            },
            EditKind::Remove => match (found.iter().position(|l| l.key == edit.line), edit.span) {
                (Some(k), None) => {
                    found.remove(k);
                }
                (Some(k), Some(span)) => found[k].removed.push(span),
                (None, _) => orphans.push(edit.key),
            },
        }
    }
    found.retain(|l| l.at.is_finite() && l.at > TOL && l.at < extent - TOL);
    found.sort_by(|a, b| a.at.total_cmp(&b.at).then_with(|| a.key.cmp(&b.key)));
    found.dedup_by(|b, a| (b.at - a.at).abs() < TOL);
    found.truncate(MAX_LINES);
    let border = |key: &str, at: f64| Line {
        key: key.to_owned(),
        at,
        border: true,
        removed: Vec::new(),
        only: None,
    };
    let mut out = vec![border(border_keys.0, 0.0)];
    out.extend(found);
    out.push(border(border_keys.1, extent));
    out
}

/// One cell of the grid: a rectangle bounded by four lines, possibly several grid cells merged
/// by removed segments.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// The key: the keys of the lines that bound it.
    pub key: String,
    /// The range along the baseline.
    pub u: (f64, f64),
    /// The range above the base.
    pub w: (f64, f64),
    /// The first and one past the last column of the grid it covers.
    pub columns: (usize, usize),
    /// The first and one past the last row of the grid it covers.
    pub rows: (usize, usize),
}

/// What no longer finds its line or cell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Orphans {
    /// The keys of grid edits whose line is gone.
    pub edits: Vec<i64>,
    /// The keys of cell overrides whose cell is gone.
    pub overrides: Vec<String>,
}

/// The lines and cells of a curtain wall.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The vertical lines from the start border to the end border.
    pub vertical: Vec<Line>,
    /// The horizontal lines from the base border to the top border.
    pub horizontal: Vec<Line>,
    /// The cells, row by row from the base.
    pub cells: Vec<Cell>,
    /// The edits and overrides that found nothing.
    pub orphans: Orphans,
}

impl Layout {
    /// The layout of a `length` by `height` wall under `ty`, edited by `edits`.
    #[must_use]
    pub fn of(ty: &CurtainType, edits: &[GridEdit], length: f64, height: f64) -> Self {
        let mut orphans = Orphans::default();
        let vertical = lines(
            &ty.vertical,
            edits,
            Direction::Vertical,
            length,
            ("start", "end"),
            &mut orphans.edits,
        );
        let horizontal = lines(
            &ty.horizontal,
            edits,
            Direction::Horizontal,
            height,
            ("base", "top"),
            &mut orphans.edits,
        );
        let cells = cells(&vertical, &horizontal);
        Self {
            vertical,
            horizontal,
            cells,
            orphans,
        }
    }

    /// The center of grid column `c`.
    fn mid_u(&self, c: usize) -> f64 {
        (self.vertical[c].at + self.vertical[c + 1].at) / 2.0
    }

    /// The center of grid row `r`.
    fn mid_w(&self, r: usize) -> f64 {
        (self.horizontal[r].at + self.horizontal[r + 1].at) / 2.0
    }

    /// Whether vertical line `i` is present in row `r`.
    fn v_present(&self, i: usize, r: usize) -> bool {
        self.vertical[i].present_at(self.mid_w(r))
    }

    /// Whether horizontal line `k` is present in column `c`.
    fn h_present(&self, k: usize, c: usize) -> bool {
        self.horizontal[k].present_at(self.mid_u(c))
    }
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// The cells of the grid: each grid cell, merged with the neighbor across an absent segment. A
/// merged group that is not a rectangle is not merged.
fn cells(vertical: &[Line], horizontal: &[Line]) -> Vec<Cell> {
    let (nc, nr) = (vertical.len() - 1, horizontal.len() - 1);
    let at = |c: usize, r: usize| r * nc + c;
    let mut parent: Vec<usize> = (0..nc * nr).collect();
    for r in 0..nr {
        let mid_w = (horizontal[r].at + horizontal[r + 1].at) / 2.0;
        for c in 1..nc {
            if !vertical[c].present_at(mid_w) {
                let (a, b) = (find(&mut parent, at(c - 1, r)), find(&mut parent, at(c, r)));
                parent[a] = b;
            }
        }
    }
    for c in 0..nc {
        let mid_u = (vertical[c].at + vertical[c + 1].at) / 2.0;
        for r in 1..nr {
            if !horizontal[r].present_at(mid_u) {
                let (a, b) = (find(&mut parent, at(c, r - 1)), find(&mut parent, at(c, r)));
                parent[a] = b;
            }
        }
    }
    // Each group's bounding box (columns, rows) and size; a group that does not fill it is split.
    type Group = (usize, usize, usize, usize, usize);
    let mut boxes: std::collections::BTreeMap<usize, Group> = std::collections::BTreeMap::new();
    for r in 0..nr {
        for c in 0..nc {
            let root = find(&mut parent, at(c, r));
            let e = boxes.entry(root).or_insert((c, c + 1, r, r + 1, 0));
            *e = (
                e.0.min(c),
                e.1.max(c + 1),
                e.2.min(r),
                e.3.max(r + 1),
                e.4 + 1,
            );
        }
    }
    let mut out: Vec<Cell> = Vec::new();
    let mut push = |c0: usize, c1: usize, r0: usize, r1: usize| {
        out.push(Cell {
            key: format!(
                "{}-{}.{}-{}",
                vertical[c0].key, vertical[c1].key, horizontal[r0].key, horizontal[r1].key
            ),
            u: (vertical[c0].at, vertical[c1].at),
            w: (horizontal[r0].at, horizontal[r1].at),
            columns: (c0, c1),
            rows: (r0, r1),
        });
    };
    for r in 0..nr {
        for c in 0..nc {
            let root = find(&mut parent, at(c, r));
            let Some(&(c0, c1, r0, r1, count)) = boxes.get(&root) else {
                continue;
            };
            if count == (c1 - c0) * (r1 - r0) {
                if (c, r) == (c0, r0) {
                    push(c0, c1, r0, r1);
                }
            } else {
                push(c, c + 1, r, r + 1);
            }
        }
    }
    out
}

// ----- the derived wall -------------------------------------------------------------------------

/// Which position a mullion is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MullionRole {
    /// On a line between cells.
    Interior,
    /// On a border of the wall.
    Border,
    /// At a corner where two curtain walls meet.
    Corner,
}

/// One mullion: a run of a grid line, or a corner.
#[derive(Debug, Clone, PartialEq)]
pub struct Mullion {
    /// The key.
    pub key: String,
    /// Interior, border or corner.
    pub role: MullionRole,
    /// Whether it stands upright (otherwise it runs along the baseline).
    pub vertical: bool,
    /// Where its axis is: along the baseline for an upright one, above the base for a horizontal
    /// one.
    pub at: f64,
    /// The stretch it covers: above the base for an upright one, along the baseline otherwise.
    pub span: (f64, f64),
    /// Its section.
    pub spec: MullionSpec,
    /// Whether it is centered on the baseline instead of the middle of the wall's thickness.
    pub on_baseline: bool,
}

impl Mullion {
    /// The length of the mullion.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.span.1 - self.span.0
    }
}

/// One panel: what fills a cell and its clear size between the mullions.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelInfo {
    /// The key: the cell's.
    pub key: String,
    /// What fills the cell.
    pub kind: PanelKind,
    /// The door or window type of a door or window panel, with its name.
    pub type_ref: Option<(ObjectId, String)>,
    /// Whether the cell has an override.
    pub overridden: bool,
    /// The cell.
    pub cell: Cell,
    /// The clear range along the baseline.
    pub u: (f64, f64),
    /// The clear range above the base.
    pub w: (f64, f64),
    /// The door or window that fills it, as an insert of the wall.
    pub insert: Option<Insert>,
}

impl PanelInfo {
    /// The clear width.
    #[must_use]
    pub fn width(&self) -> f64 {
        (self.u.1 - self.u.0).max(0.0)
    }

    /// The clear height.
    #[must_use]
    pub fn height(&self) -> f64 {
        (self.w.1 - self.w.0).max(0.0)
    }

    /// The clear area.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }
}

/// What a solid of a curtain wall is, which decides its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// A glass pane.
    Glass,
    /// An opaque panel.
    Solid,
    /// A mullion.
    Mullion,
    /// The frame of a door or window panel.
    Frame,
    /// The leaf of a door panel.
    Leaf,
    /// The handle of a door panel.
    Handle,
}

/// One solid of a curtain wall with the identifier its child has.
#[derive(Debug, Clone)]
pub struct Solid {
    /// The key of the panel or mullion it belongs to.
    pub key: String,
    /// The identifier of that child: [`child_id`].
    pub id: u128,
    /// What it is.
    pub class: Class,
    /// The solid.
    pub solid: Polyhedron,
}

/// A curtain wall read from the document with everything derived from it.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The wall.
    pub id: ObjectId,
    /// The wall's authored data.
    pub wall: Wall,
    /// The elevation of its storey's level.
    pub level: f64,
    /// The type.
    pub ty: CurtainType,
    /// The grid.
    pub layout: Layout,
    /// The panels, in the order of the cells.
    pub panels: Vec<PanelInfo>,
    /// The mullions.
    pub mullions: Vec<Mullion>,
}

/// The curtain type of a wall record, when it is a curtain wall.
#[must_use]
pub fn type_of_wall(snapshot: &Snapshot, object: &Object) -> Option<CurtainType> {
    CurtainType::from_values(&type_of(snapshot, TypeKind::Wall, &object.values)?.values)
}

/// Whether the wall record is a curtain wall.
#[must_use]
pub fn is_curtain(snapshot: &Snapshot, object: &Object) -> bool {
    type_of_wall(snapshot, object).is_some()
}

/// Which ends of a curtain wall meet another curtain wall at a corner, and whether this wall
/// owns that corner's mullion (the wall with the lowest identifier does).
fn corners(snapshot: &Snapshot, object: &Object, wall: &Wall) -> [(bool, bool); 2] {
    let mut out = [(false, true); 2];
    let Some(storey) = object.header.owner else {
        return out;
    };
    let direction = |w: &Wall, from_start: bool| {
        let (a, b) = if from_start {
            (w.start, w.end)
        } else {
            (w.end, w.start)
        };
        let l = w.length().max(f64::MIN_POSITIVE);
        [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
    };
    for other in spatial::walls(snapshot, storey) {
        if other.id == object.id || !is_curtain(snapshot, &other) {
            continue;
        }
        let Some(theirs) = Wall::from_values(&other.values) else {
            continue;
        };
        for (e, end) in [wall.start, wall.end].into_iter().enumerate() {
            for (f, there) in [theirs.start, theirs.end].into_iter().enumerate() {
                if (end[0] - there[0]).hypot(end[1] - there[1]) > TOL {
                    continue;
                }
                let (a, b) = (direction(wall, e == 0), direction(&theirs, f == 0));
                if (a[0] * b[1] - a[1] * b[0]).abs() > 0.01 {
                    out[e].0 = true;
                    out[e].1 &= object.id < other.id;
                }
            }
        }
    }
    out
}

impl Derived {
    /// The curtain wall of a record; `None` when it is not one or cannot be read.
    #[must_use]
    pub fn of(snapshot: &Snapshot, object: &Object) -> Option<Self> {
        let ty = type_of_wall(snapshot, object)?;
        let wall = Wall::of(snapshot, object)?;
        let level = object
            .header
            .owner
            .and_then(|s| spatial::get(snapshot, s))
            .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
        let edits = GridEdit::list(&object.values);
        let overrides = CellOverride::list(&object.values);
        let mut layout = Layout::of(&ty, &edits, wall.length(), wall.height);
        let joined = corners(snapshot, object, &wall);
        let mullions = mullions(&ty, &layout, &wall, joined);
        let panels = panels(snapshot, &ty, &layout, &overrides, joined);
        layout.orphans.overrides = overrides
            .iter()
            .filter(|o| !layout.cells.iter().any(|c| c.key == o.cell))
            .map(|o| o.cell.clone())
            .collect();
        Some(Self {
            id: object.id,
            wall,
            level,
            ty,
            layout,
            panels,
            mullions,
        })
    }

    /// The solids of the panels and mullions.
    ///
    /// # Errors
    /// The solid kernel's errors for numbers it cannot make solids of.
    pub fn solids(&self) -> Result<Vec<Solid>> {
        let axes = crate::insert::Axes::new(&self.wall, self.level)?;
        let mid = (axes.low + axes.high) / 2.0;
        let mut out = Vec::new();
        let mut add = |key: String, role: u32, class: Class, solid: Polyhedron| {
            out.push(Solid {
                id: child_id(self.id, role, &key),
                key,
                class,
                solid,
            });
        };
        for p in &self.panels {
            let key = format!("p:{}", p.key);
            match (p.kind, &p.insert) {
                (PanelKind::Empty, _) => {}
                (PanelKind::Door | PanelKind::Window, Some(item)) => {
                    for (k, (role, solid)) in crate::insert::pieces(&self.wall, self.level, item)
                        .unwrap_or_default()
                        .into_iter()
                        .enumerate()
                    {
                        let class = match role {
                            Role::Frame => Class::Frame,
                            Role::Panel => Class::Leaf,
                            Role::Glass => Class::Glass,
                            Role::Handle => Class::Handle,
                        };
                        add(format!("{key}#{k}"), PANEL_ROLE, class, solid);
                    }
                }
                (kind, _) => {
                    if p.width() < TOL || p.height() < TOL {
                        continue;
                    }
                    let half = self.ty.panel_thickness.min(axes.high - axes.low) / 2.0;
                    let solid = boxed(
                        &axes,
                        p.u,
                        (mid - half, mid + half),
                        (axes.base + p.w.0, axes.base + p.w.1),
                    )?;
                    let class = if kind == PanelKind::Solid {
                        Class::Solid
                    } else {
                        Class::Glass
                    };
                    add(key, PANEL_ROLE, class, solid);
                }
            }
        }
        for m in &self.mullions {
            if m.length() < TOL {
                continue;
            }
            let across = if m.on_baseline { 0.0 } else { mid };
            let (w, d) = (m.spec.width, m.spec.depth);
            let solid = match (m.vertical, m.spec.profile) {
                (_, Profile::None) => continue,
                (true, Profile::Rectangular) => boxed(
                    &axes,
                    (m.at - w / 2.0, m.at + w / 2.0),
                    (across - d / 2.0, across + d / 2.0),
                    (axes.base + m.span.0, axes.base + m.span.1),
                )?,
                (true, Profile::Circular) => prism_of(
                    &circle(|a, b| axes.at(m.at + a, across + b), w / 2.0),
                    axes.base + m.span.0,
                    m.span.1 - m.span.0,
                )?,
                (false, Profile::Rectangular) => boxed(
                    &axes,
                    m.span,
                    (across - d / 2.0, across + d / 2.0),
                    (axes.base + m.at - w / 2.0, axes.base + m.at + w / 2.0),
                )?,
                (false, Profile::Circular) => {
                    // A cylinder along the baseline: the circle in the plane across it.
                    let tolerance = Tolerance::DEFAULT;
                    let origin = axes.at(m.span.0, 0.0);
                    let frame = PlaneFrame::new(
                        Point3::new(origin.x, origin.y, axes.base + m.at),
                        Vector3::new(axes.n[0], axes.n[1], 0.0),
                        Vector3::z(),
                        &tolerance,
                    )?;
                    let ring = circle(|a, b| Point2::new(across + a, b), w / 2.0);
                    let region = Boundary::polygons(vec![ring_points(&ring)]).region()?;
                    extrude(
                        &region,
                        &frame,
                        m.span.1 - m.span.0,
                        CHORD,
                        &tolerance,
                        &Limits::DEFAULT,
                    )?
                }
            };
            add(format!("m:{}", m.key), MULLION_ROLE, Class::Mullion, solid);
        }
        Ok(out)
    }

    /// The number of panels that are not empty and of mullions.
    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        (
            self.panels
                .iter()
                .filter(|p| p.kind != PanelKind::Empty)
                .count(),
            self.mullions.len(),
        )
    }

    /// The door and window panels as inserts of the wall, for the plan symbols.
    #[must_use]
    pub fn leaves(&self) -> Vec<(&str, Insert)> {
        self.panels
            .iter()
            .filter_map(|p| p.insert.map(|i| (p.key.as_str(), i)))
            .collect()
    }
}

fn ring_points(ring: &[Point2]) -> Vec<[f64; 2]> {
    ring.iter().map(|p| [p.x, p.y]).collect()
}

fn circle(place: impl Fn(f64, f64) -> Point2, radius: f64) -> Vec<Point2> {
    (0..CIRCLE)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / CIRCLE as f64;
            place(radius * a.cos(), radius * a.sin())
        })
        .collect()
}

fn prism_of(ring: &[Point2], z: f64, height: f64) -> Result<Polyhedron> {
    let region = Boundary::polygons(vec![ring_points(ring)]).region()?;
    prism(
        &region,
        z,
        height,
        CHORD,
        &Tolerance::DEFAULT,
        &Limits::DEFAULT,
    )
}

/// A box: `x` along the baseline, `y` across it (to the left), `z` absolute.
fn boxed(
    axes: &crate::insert::Axes,
    x: (f64, f64),
    y: (f64, f64),
    z: (f64, f64),
) -> Result<Polyhedron> {
    let ring = [
        axes.at(x.0, y.0),
        axes.at(x.1, y.0),
        axes.at(x.1, y.1),
        axes.at(x.0, y.1),
    ];
    prism_of(&ring, z.0, z.1 - z.0)
}

/// How far the mullion of line `i` of `lines` reaches below and above its position: a border
/// mullion lies inside the wall, an interior one is centered on its line.
fn reach(lines: &[Line], i: usize, border: &MullionSpec, interior: &MullionSpec) -> (f64, f64) {
    if i == 0 {
        (0.0, border.extent())
    } else if i + 1 == lines.len() {
        (border.extent(), 0.0)
    } else {
        (interior.extent() / 2.0, interior.extent() / 2.0)
    }
}

/// The mullions: the runs of the continuous direction, the pieces of the other between them, and
/// the corners.
#[allow(clippy::too_many_lines, reason = "one pass per direction")]
fn mullions(
    ty: &CurtainType,
    layout: &Layout,
    wall: &Wall,
    joined: [(bool, bool); 2],
) -> Vec<Mullion> {
    let (nv, nh) = (layout.vertical.len(), layout.horizontal.len());
    let (nc, nr) = (nv - 1, nh - 1);
    let spec_of = |lines: &[Line], i: usize| -> MullionSpec {
        if lines[i].border {
            ty.border
        } else {
            ty.interior
        }
    };
    let skip_corner = |i: usize| -> bool {
        ty.corner.profile != Profile::None
            && ((i == 0 && joined[0].0) || (i + 1 == nv && joined[1].0))
    };
    let r_v = |i| reach(&layout.vertical, i, &ty.border, &ty.interior);
    let r_h = |i| reach(&layout.horizontal, i, &ty.border, &ty.interior);
    // Whether an upright mullion stands at the node of vertical line `i` and horizontal line `k`.
    let v_at = |i: usize, k: usize| -> bool {
        spec_of(&layout.vertical, i).profile != Profile::None
            && !skip_corner(i)
            && ((k > 0 && layout.v_present(i, k - 1)) || (k < nr && layout.v_present(i, k)))
    };
    let h_at = |k: usize, i: usize| -> bool {
        spec_of(&layout.horizontal, k).profile != Profile::None
            && ((i > 0 && layout.h_present(k, i - 1)) || (i < nc && layout.h_present(k, i)))
    };
    let role = |l: &Line| {
        if l.border {
            MullionRole::Border
        } else {
            MullionRole::Interior
        }
    };
    let mut out = Vec::new();
    let vertical_first = ty.priority == Priority::Vertical;
    for i in 0..nv {
        let spec = spec_of(&layout.vertical, i);
        if spec.profile == Profile::None || skip_corner(i) {
            continue;
        }
        let (lo, hi) = r_v(i);
        let at = layout.vertical[i].at + (hi - lo) / 2.0;
        let mut r = 0;
        while r < nr {
            if !layout.v_present(i, r) {
                r += 1;
                continue;
            }
            let from = r;
            if vertical_first {
                while r < nr && layout.v_present(i, r) {
                    r += 1;
                }
            } else {
                r += 1;
            }
            let (mut a, mut b) = (layout.horizontal[from].at, layout.horizontal[r].at);
            if !vertical_first {
                if h_at(from, i) {
                    a += r_h(from).1;
                }
                if h_at(r, i) {
                    b -= r_h(r).0;
                }
            }
            out.push(Mullion {
                key: format!(
                    "v:{}:{}-{}",
                    layout.vertical[i].key, layout.horizontal[from].key, layout.horizontal[r].key
                ),
                role: role(&layout.vertical[i]),
                vertical: true,
                at,
                span: (a, b),
                spec,
                on_baseline: false,
            });
        }
    }
    for k in 0..nh {
        let spec = spec_of(&layout.horizontal, k);
        if spec.profile == Profile::None {
            continue;
        }
        let (lo, hi) = r_h(k);
        let at = layout.horizontal[k].at + (hi - lo) / 2.0;
        let mut c = 0;
        while c < nc {
            if !layout.h_present(k, c) {
                c += 1;
                continue;
            }
            let from = c;
            if vertical_first {
                c += 1;
            } else {
                while c < nc && layout.h_present(k, c) {
                    c += 1;
                }
            }
            let (mut a, mut b) = (layout.vertical[from].at, layout.vertical[c].at);
            if vertical_first {
                if v_at(from, k) {
                    a += r_v(from).1;
                }
                if v_at(c, k) {
                    b -= r_v(c).0;
                }
            }
            out.push(Mullion {
                key: format!(
                    "h:{}:{}-{}",
                    layout.horizontal[k].key, layout.vertical[from].key, layout.vertical[c].key
                ),
                role: role(&layout.horizontal[k]),
                vertical: false,
                at,
                span: (a, b),
                spec,
                on_baseline: false,
            });
        }
    }
    for (e, (is_joined, owns)) in joined.into_iter().enumerate() {
        if is_joined && owns && ty.corner.profile != Profile::None {
            out.push(Mullion {
                key: format!("corner:{}", if e == 0 { "start" } else { "end" }),
                role: MullionRole::Corner,
                vertical: true,
                at: if e == 0 { 0.0 } else { wall.length() },
                span: (0.0, wall.height),
                spec: ty.corner,
                on_baseline: true,
            });
        }
    }
    out.retain(|m| m.span.1 - m.span.0 > TOL);
    out
}

fn panels(
    snapshot: &Snapshot,
    ty: &CurtainType,
    layout: &Layout,
    overrides: &[CellOverride],
    joined: [(bool, bool); 2],
) -> Vec<PanelInfo> {
    let nv = layout.vertical.len();
    let corner = |i: usize| {
        ty.corner.profile != Profile::None
            && ((i == 0 && joined[0].0) || (i + 1 == nv && joined[1].0))
    };
    layout
        .cells
        .iter()
        .map(|cell| {
            let found = overrides.iter().find(|o| o.cell == cell.key);
            let kind = found.map_or(ty.panel, |o| o.panel);
            // The clear rectangle: the cell less what its bounding mullions take.
            let lo_u = if corner(cell.columns.0) {
                ty.corner.extent() / 2.0
            } else {
                reach(&layout.vertical, cell.columns.0, &ty.border, &ty.interior).1
            };
            let hi_u = if corner(cell.columns.1) {
                ty.corner.extent() / 2.0
            } else {
                reach(&layout.vertical, cell.columns.1, &ty.border, &ty.interior).0
            };
            let lo_w = reach(&layout.horizontal, cell.rows.0, &ty.border, &ty.interior).1;
            let hi_w = reach(&layout.horizontal, cell.rows.1, &ty.border, &ty.interior).0;
            let u = (cell.u.0 + lo_u, cell.u.1 - hi_u);
            let w = (cell.w.0 + lo_w, cell.w.1 - hi_w);
            let type_ref = found
                .and_then(|o| o.type_id)
                .and_then(|id| spatial::get(snapshot, id))
                .map(|t| (t.id, crate::element_type::name(&t).to_owned()));
            let insert = if matches!(kind, PanelKind::Door | PanelKind::Window) {
                leaf(snapshot, kind, found.and_then(|o| o.type_id), u, w)
            } else {
                None
            };
            PanelInfo {
                key: cell.key.clone(),
                kind,
                type_ref,
                overridden: found.is_some(),
                cell: cell.clone(),
                u,
                w,
                insert,
            }
        })
        .collect()
}

/// The door or window that fills the clear rectangle `u` by `w`, with the frame and operation of
/// its type (or a plain one without a type).
fn leaf(
    snapshot: &Snapshot,
    kind: PanelKind,
    type_id: Option<ObjectId>,
    u: (f64, f64),
    w: (f64, f64),
) -> Option<Insert> {
    let (width, height) = (u.1 - u.0, w.1 - w.0);
    if width < crate::insert::MIN_CLEAR * 2.0 || height < crate::insert::MIN_CLEAR * 2.0 {
        return None;
    }
    let door = kind == PanelKind::Door;
    let item_kind = if door { Kind::Door } else { Kind::Window };
    let found = type_id
        .and_then(|id| spatial::get(snapshot, id))
        .and_then(|t| InsertType::from_values(&t.values));
    let plain = if door {
        Operation::SingleSwing
    } else {
        Operation::Fixed
    };
    let (frame_width, frame_depth, operation) = found.map_or((0.05, 0.1, plain), |t| {
        (t.frame_width, t.frame_depth, t.operation)
    });
    Some(Insert {
        kind: item_kind,
        distance: (u.0 + u.1) / 2.0,
        width,
        height,
        sill: w.0,
        // A bar cannot be wider than a third of the cell.
        frame_width: frame_width.min(width / 3.0).min(height / 3.0),
        frame_depth,
        operation: if operation.allowed(item_kind) {
            operation
        } else {
            plain
        },
        hand: Hand::default(),
        facing: Facing::default(),
        rise: 0.0,
    })
}

// ----- embedding and rules ----------------------------------------------------------------------

/// The opening a curtain wall inside the basic wall `host` cuts in it: the curtain wall must run
/// along the host with both ends within its length and its baseline within its thickness, and rise
/// no higher than the host. The opening is the curtain wall's width and height.
fn opening_of(host: &Wall, curtain: &Wall) -> Option<Insert> {
    let length = host.length();
    if length < types::MIN_WALL_LENGTH {
        return None;
    }
    let u = [
        (host.end[0] - host.start[0]) / length,
        (host.end[1] - host.start[1]) / length,
    ];
    let rel = |p: [f64; 2]| {
        let d = [p[0] - host.start[0], p[1] - host.start[1]];
        (d[0] * u[0] + d[1] * u[1], u[0] * d[1] - u[1] * d[0])
    };
    let ((a0, y0), (a1, y1)) = (rel(curtain.start), rel(curtain.end));
    let (low, high) = host.extent();
    let within = |y: f64| y >= low - TOL && y <= high + TOL;
    if !(within(y0) && within(y1)) || (y0 - y1).abs() > TOL * 2.0 {
        return None;
    }
    let (from, to) = (a0.min(a1), a0.max(a1));
    if from < -TOL || to > length + TOL {
        return None;
    }
    let (from, to) = (from.max(0.0), to.min(length));
    let sill = (curtain.base_offset - host.base_offset).max(0.0);
    let height = curtain.height.min(host.height - sill);
    if to - from < types::INSERT_WIDTH_RANGE.0 || height < types::INSERT_HEIGHT_RANGE.0 {
        return None;
    }
    Some(Insert {
        kind: Kind::Opening,
        distance: (from + to) / 2.0,
        width: to - from,
        height,
        sill,
        frame_width: 0.0,
        frame_depth: 0.0,
        operation: Operation::default(),
        hand: Hand::default(),
        facing: Facing::default(),
        rise: 0.0,
    })
}

/// The curtain walls of the storey of the basic wall `host`, with the opening each cuts in it.
#[must_use]
pub fn embedded(snapshot: &Snapshot, host: &Object) -> Vec<(ObjectId, Insert)> {
    let Some(storey) = host.header.owner else {
        return Vec::new();
    };
    // Most documents have no curtain type: nothing to search.
    if !spatial::objects(snapshot, types::WALL_TYPE)
        .iter()
        .any(|t| CurtainType::from_values(&t.values).is_some())
        || is_curtain(snapshot, host)
    {
        return Vec::new();
    }
    let Some(own) = Wall::of(snapshot, host) else {
        return Vec::new();
    };
    spatial::walls(snapshot, storey)
        .into_iter()
        .filter(|o| o.id != host.id && is_curtain(snapshot, o))
        .filter_map(|o| {
            let theirs = Wall::of(snapshot, &o)?;
            opening_of(&own, &theirs).map(|i| (o.id, i))
        })
        .collect()
}

/// Whether the curtain wall `curtain` is embedded in the basic wall `host`.
#[must_use]
pub fn is_embedded_in(snapshot: &Snapshot, curtain: &Object, host: &Object) -> bool {
    is_curtain(snapshot, curtain)
        && embedded(snapshot, host)
            .iter()
            .any(|(id, _)| *id == curtain.id)
}

/// Whether the wall record holds grid edits or panel overrides, which belong to its own grid.
#[must_use]
pub fn is_edited(object: &Object) -> bool {
    !GridEdit::list(&object.values).is_empty() || !CellOverride::list(&object.values).is_empty()
}

/// The rules of one wall record that is a curtain wall: its edits and overrides within bounds, no
/// door or window hosted by it (a door or window fills a cell instead), and no cell merge that
/// is not a rectangle.
#[must_use]
pub fn problems(snapshot: &Snapshot, object: &Object) -> Vec<Error> {
    let mut out = Vec::new();
    let name = types::text(&object.values, 0)
        .unwrap_or_default()
        .to_owned();
    let Some(ty) = type_of_wall(snapshot, object) else {
        return out;
    };
    let count = |at: usize| match object.values.get(at) {
        Some(Some(Value::List(items))) => items.len(),
        _ => 0,
    };
    if count(wall::GRID_EDITS) > MAX_EDITS || count(wall::CELL_OVERRIDES) > MAX_OVERRIDES {
        out.push(Error::new(codes::CURTAIN_LIMIT).with_arg("name", name.clone()));
    }
    if [types::DOOR, types::WINDOW, types::WALL_OPENING]
        .iter()
        .any(|k| !spatial::children(snapshot, k, object.id).is_empty())
    {
        out.push(Error::new(codes::CURTAIN_INSERT).with_arg("name", name.clone()));
    }
    if let Some(w) = Wall::from_values(&object.values) {
        let edits = GridEdit::list(&object.values);
        let layout = Layout::of(&ty, &edits, w.length(), w.height);
        if merge_is_split(&layout, &edits) {
            out.push(Error::new(codes::CURTAIN_CELL_SHAPE).with_arg("name", name));
        }
    }
    out
}

/// Whether a removed segment left a merge that is not a rectangle: some removed edge separates two
/// cells that did not merge.
fn merge_is_split(layout: &Layout, edits: &[GridEdit]) -> bool {
    if !edits
        .iter()
        .any(|e| e.kind == EditKind::Remove && e.span.is_some())
    {
        return false;
    }
    let (nc, nr) = (layout.vertical.len() - 1, layout.horizontal.len() - 1);
    let cell_of = |c: usize, r: usize| {
        layout
            .cells
            .iter()
            .position(|k| k.columns.0 <= c && c < k.columns.1 && k.rows.0 <= r && r < k.rows.1)
    };
    for r in 0..nr {
        for c in 1..nc {
            if !layout.v_present(c, r) && cell_of(c - 1, r) != cell_of(c, r) {
                return true;
            }
        }
    }
    for c in 0..nc {
        for r in 1..nr {
            if !layout.h_present(r, c) && cell_of(c, r - 1) != cell_of(c, r) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: RuleKind, value: f64) -> GridRule {
        GridRule {
            kind,
            value,
            justify: Justify::Start,
            offset: 0.0,
        }
    }

    fn at(lines: &[(String, f64)]) -> Vec<f64> {
        lines.iter().map(|l| (l.1 * 1e6).round() / 1e6).collect()
    }

    #[test]
    fn a_rule_lays_out_lines_by_distance_number_and_spacing() {
        assert_eq!(
            at(&rule(RuleKind::FixedDistance, 1.5).lines(5.0)),
            vec![1.5, 3.0, 4.5]
        );
        let end = GridRule {
            justify: Justify::End,
            ..rule(RuleKind::FixedDistance, 1.5)
        };
        assert_eq!(at(&end.lines(5.0)), vec![0.5, 2.0, 3.5]);
        let center = GridRule {
            justify: Justify::Center,
            offset: 0.25,
            ..rule(RuleKind::FixedDistance, 2.0)
        };
        assert_eq!(at(&center.lines(5.0)), vec![0.75, 2.75, 4.75]);
        assert_eq!(
            at(&rule(RuleKind::FixedNumber, 3.0).lines(4.0)),
            vec![1.0, 2.0, 3.0]
        );
        // At most 1.2 wide: 5 m needs five cells; at least 1.2 wide: four.
        assert_eq!(rule(RuleKind::MaxSpacing, 1.2).lines(5.0).len(), 4);
        assert_eq!(rule(RuleKind::MinSpacing, 1.2).lines(5.0).len(), 3);
        assert!(rule(RuleKind::FixedDistance, 0.0).lines(5.0).is_empty());
    }

    #[test]
    fn line_keys_do_not_depend_on_the_position() {
        let a = rule(RuleKind::FixedDistance, 1.5).lines(5.0);
        let b = rule(RuleKind::FixedDistance, 1.2).lines(5.0);
        assert_eq!(a[0].0, b[0].0);
        assert_eq!(a[0].0, "s1");
    }

    fn layout(edits: &[GridEdit]) -> Layout {
        let mut ty = CurtainType::standard();
        ty.vertical = rule(RuleKind::FixedDistance, 1.0);
        ty.horizontal = rule(RuleKind::FixedNumber, 1.0);
        Layout::of(&ty, edits, 3.0, 3.0)
    }

    fn edit(key: i64, kind: EditKind, direction: Direction, line: &str, at: f64) -> GridEdit {
        GridEdit {
            key,
            direction,
            kind,
            line: line.to_owned(),
            at,
            span: None,
        }
    }

    #[test]
    fn edits_add_move_and_remove_lines_and_keep_the_cells_they_leave() {
        let plain = layout(&[]);
        assert_eq!(plain.cells.len(), 6);
        let keys: Vec<_> = plain.cells.iter().map(|c| c.key.clone()).collect();
        // A moved line keeps its key, so the cells next to it keep theirs.
        let moved = layout(&[edit(1, EditKind::Move, Direction::Vertical, "s1", 1.2)]);
        assert_eq!(
            moved
                .cells
                .iter()
                .map(|c| c.key.clone())
                .collect::<Vec<_>>(),
            keys
        );
        assert!((moved.cells[0].u.1 - 1.2).abs() < 1e-9);
        // An added line splits a cell; a removed line merges two.
        let added = layout(&[edit(2, EditKind::Add, Direction::Vertical, "", 0.5)]);
        assert_eq!(added.cells.len(), 8);
        let removed = layout(&[edit(3, EditKind::Remove, Direction::Vertical, "s1", 0.0)]);
        assert_eq!(removed.cells.len(), 4);
        // An edit of a line that is gone is reported, not applied elsewhere.
        let orphan = layout(&[edit(4, EditKind::Move, Direction::Vertical, "s9", 1.0)]);
        assert_eq!(orphan.orphans.edits, vec![4]);
    }

    #[test]
    fn a_removed_segment_merges_the_two_cells_across_it() {
        let mut e = edit(1, EditKind::Remove, Direction::Vertical, "s1", 0.0);
        e.span = Some((0.0, 1.5));
        let l = layout(&[e]);
        // The segment in the lower row is gone: its two cells are one.
        assert_eq!(l.cells.len(), 5);
        assert!(l.cells.iter().any(|c| (c.u.1 - c.u.0 - 2.0).abs() < 1e-9));
    }

    #[test]
    fn mullions_run_through_in_the_continuous_direction() {
        let mut ty = CurtainType::standard();
        ty.vertical = rule(RuleKind::FixedNumber, 1.0);
        ty.horizontal = rule(RuleKind::FixedNumber, 1.0);
        let w = Wall {
            start: [0.0, 0.0],
            end: [4.0, 0.0],
            thickness: 0.15,
            core: [0.0, 0.0],
            height: 4.0,
            base_offset: 0.0,
            location: crate::wall::LocationLine::Center,
            start_join: crate::wall::JoinStyle::Auto,
            end_join: crate::wall::JoinStyle::Auto,
        };
        let layout = Layout::of(&ty, &[], 4.0, 4.0);
        let none = [(false, false); 2];
        let vertical = mullions(&ty, &layout, &w, none);
        // Three upright mullions, each the full height; the horizontal ones come in pieces.
        let upright: Vec<_> = vertical.iter().filter(|m| m.vertical).collect();
        assert_eq!(upright.len(), 3);
        assert!(upright.iter().all(|m| (m.length() - 4.0).abs() < 1e-9));
        assert_eq!(vertical.iter().filter(|m| !m.vertical).count(), 6);
        ty.priority = Priority::Horizontal;
        let horizontal = mullions(&ty, &layout, &w, none);
        let across: Vec<_> = horizontal.iter().filter(|m| !m.vertical).collect();
        assert_eq!(across.len(), 3);
        assert!(across.iter().all(|m| (m.length() - 4.0).abs() < 1e-9));
        // A corner mullion replaces the border mullion at a joined end.
        let corner = mullions(&ty, &layout, &w, [(true, true), (false, false)]);
        assert!(corner.iter().any(|m| m.role == MullionRole::Corner));
        assert!(!corner.iter().any(|m| m.key.starts_with("v:start:")));
    }

    #[test]
    fn child_identifiers_depend_on_the_parent_and_the_key_alone() {
        let parent = ObjectId::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef);
        let a = child_id(parent, PANEL_ROLE, "p:start-s1.base-top");
        assert_eq!(a, child_id(parent, PANEL_ROLE, "p:start-s1.base-top"));
        assert_ne!(a, child_id(parent, PANEL_ROLE, "p:s1-end.base-top"));
        assert_ne!(a, child_id(parent, MULLION_ROLE, "p:start-s1.base-top"));
        assert_ne!(a, parent.as_u128());
    }
}
