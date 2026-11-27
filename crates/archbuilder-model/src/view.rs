//! The plan view of a storey: a record of authored view settings (the view range, scale, detail
//! level, which categories are drawn, the elements hidden in this view and whether the storey
//! below shows faint underneath). The drawing is derived from it and never stored (see
//! [`crate::plan`]).
//!
//! A view belongs to its storey (its header owner), so deleting the storey deletes its views.
//! The range is stored relative to the storey's level: the cut height above the level (1.2 m by
//! default), the bottom (the level itself by default), the view depth below it (a centimeter
//! under the level by default), and the top, which is the next level, else 3 m above the level,
//! unless the view sets one. What lies between the cut and the top is the overhead range; what
//! lies below the cut down to the view depth is drawn beyond the cut.

use arc_base::{ObjectId, Result};
use arc_doc::{Object, Snapshot, Value};

use crate::error::codes;
use crate::spatial;
use crate::types::{PLAN_VIEW, STOREY, plan_view};

/// The cut height of a new view above its level, in meters.
pub const DEFAULT_CUT: f64 = 1.2;
/// The top of a view range above the level when no level lies above, in meters.
pub const DEFAULT_TOP: f64 = 3.0;
/// The scale denominator of a new view (1:100).
pub const DEFAULT_SCALE: f64 = 100.0;
/// The least distance between the cut and the top, in meters.
pub const MIN_BAND: f64 = 0.01;
/// How far under the level the view depth of a new view lies, in meters: a slab whose top is the
/// level ends within the depth, so it draws as beyond instead of being left out.
pub const BOTTOM_MARGIN: f64 = 0.01;
/// The most plan views of a document that are read.
pub const MAX_VIEWS: usize = 10_000;

coded! {
    /// How much detail a view draws: coarse merges the layers of a wall, medium and fine draw
    /// each layer; fine also draws the frames and leaves of doors and windows.
    Detail {
        /// One body per element.
        Coarse = 0, "coarse";
        /// Layers of layered elements.
        #[default]
        Medium = 1, "medium";
        /// Layers and the parts of doors and windows.
        Fine = 2, "fine";
    }
}

/// The kinds of element a view shows or hides as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    /// Walls.
    Wall,
    /// Floors.
    Floor,
    /// Roofs.
    Roof,
    /// Ceilings.
    Ceiling,
    /// Doors.
    Door,
    /// Windows.
    Window,
    /// Stairs.
    Stair,
    /// Rooms.
    Room,
    /// Shafts and openings.
    Opening,
}

impl Category {
    /// Every category, in the order of the flags of a view record.
    pub const ALL: [Self; 9] = [
        Self::Wall,
        Self::Floor,
        Self::Roof,
        Self::Ceiling,
        Self::Door,
        Self::Window,
        Self::Stair,
        Self::Room,
        Self::Opening,
    ];

    /// The name of the category in the drawing (the key of its graphic overrides).
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Wall => "wall",
            Self::Floor => "floor",
            Self::Roof => "roof",
            Self::Ceiling => "ceiling",
            Self::Door => "door",
            Self::Window => "window",
            Self::Stair => "stair",
            Self::Room => "room",
            Self::Opening => "opening",
        }
    }

    /// The name of the category's field in the view record.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Wall => "show_walls",
            Self::Floor => "show_floors",
            Self::Roof => "show_roofs",
            Self::Ceiling => "show_ceilings",
            Self::Door => "show_doors",
            Self::Window => "show_windows",
            Self::Stair => "show_stairs",
            Self::Room => "show_rooms",
            Self::Opening => "show_openings",
        }
    }

    /// Whether a new view draws the category: every one but ceilings, which belong to ceiling
    /// plans.
    #[must_use]
    pub const fn shown_by_default(self) -> bool {
        !matches!(self, Self::Ceiling)
    }

    /// The position of the category's flag in the view record.
    #[must_use]
    pub fn slot(self) -> usize {
        plan_view::CATEGORIES + Self::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }

    /// The category of the element type `key`; `None` for what a view does not draw.
    #[must_use]
    pub fn of_key(key: &str) -> Option<Self> {
        use crate::types as t;
        Some(match key {
            t::WALL => Self::Wall,
            t::FLOOR => Self::Floor,
            t::ROOF => Self::Roof,
            t::CEILING => Self::Ceiling,
            t::DOOR => Self::Door,
            t::WINDOW => Self::Window,
            t::STAIR | t::RAMP | t::RAILING => Self::Stair,
            t::ROOM => Self::Room,
            t::SHAFT | t::WALL_OPENING | t::VERTICAL_OPENING => Self::Opening,
            _ => return None,
        })
    }
}

/// The authored settings of a plan view, lengths in meters.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanView {
    /// The name.
    pub name: String,
    /// The cut plane's height above the level.
    pub cut: f64,
    /// The top of the range above the level, when the view sets one.
    pub top: Option<f64>,
    /// Whether the overhead range is drawn.
    pub overhead: bool,
    /// The scale denominator.
    pub scale: f64,
    /// The detail level.
    pub detail: Detail,
    /// Whether the storey below shows faint underneath.
    pub underlay: bool,
    /// Which categories are drawn, in the order of [`Category::ALL`].
    pub shown: [bool; 9],
    /// The elements hidden in this view.
    pub hidden: Vec<ObjectId>,
    /// The bottom of the range relative to the level (not above the cut).
    pub bottom: f64,
    /// The view depth relative to the level: how far below the bottom elements are still drawn
    /// (not above the bottom).
    pub depth: f64,
    /// The extent and head overrides of grids in this view.
    pub datum_views: Vec<DatumView>,
    /// Which north the view is oriented to.
    pub north: North,
}

coded! {
    /// Which north a plan view is oriented to.
    North {
        /// Project north: the internal y axis points up.
        #[default]
        Project = 0, "project_north";
        /// True north points up.
        True = 1, "true_north";
    }
}

/// The range of a view-specific grid extent adjustment, in meters.
pub const INSET_RANGE: (f64, f64) = (-1000.0, 1000.0);

/// What one view overrides of a grid: how far each end is pulled in (positive) or pushed out
/// (negative) from the model extent, and whether each end shows its head. `None` keeps what the
/// grid itself says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DatumView {
    /// The grid.
    pub datum: ObjectId,
    /// How far the start is pulled in along the line, in meters.
    pub start_inset: Option<f64>,
    /// How far the end is pulled in along the line, in meters.
    pub end_inset: Option<f64>,
    /// Whether the start shows its head.
    pub head_start: Option<bool>,
    /// Whether the end shows its head.
    pub head_end: Option<bool>,
}

impl DatumView {
    /// The override of `datum` that overrides nothing.
    #[must_use]
    pub fn new(datum: ObjectId) -> Self {
        Self {
            datum,
            start_inset: None,
            end_inset: None,
            head_start: None,
            head_end: None,
        }
    }

    /// Whether the entry overrides nothing and may be dropped.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start_inset.is_none()
            && self.end_inset.is_none()
            && self.head_start.is_none()
            && self.head_end.is_none()
    }
}

/// The value of the `datum_views` field; absent when there is no override.
#[must_use]
pub fn datum_views_value(list: &[DatumView]) -> Option<Value> {
    (!list.is_empty()).then(|| {
        Value::List(
            list.iter()
                .take(crate::types::MAX_PER_VIEW_DATUMS)
                .map(|d| {
                    Value::Record(vec![
                        Some(Value::Reference(d.datum)),
                        d.start_inset.map(crate::types::length),
                        d.end_inset.map(crate::types::length),
                        d.head_start.map(Value::Bool),
                        d.head_end.map(Value::Bool),
                    ])
                })
                .collect(),
        )
    })
}

fn datum_views_at(values: &[Option<Value>]) -> Vec<DatumView> {
    let Some(Some(Value::List(items))) = values.get(plan_view::DATUM_VIEWS) else {
        return Vec::new();
    };
    let flag = |r: &[Option<Value>], at: usize| match r.get(at) {
        Some(Some(Value::Bool(b))) => Some(*b),
        _ => None,
    };
    items
        .iter()
        .take(crate::types::MAX_PER_VIEW_DATUMS)
        .filter_map(|item| match item {
            Value::Record(r) => match r.first() {
                Some(Some(Value::Reference(datum))) => Some(DatumView {
                    datum: *datum,
                    start_inset: crate::types::meters(r, 1),
                    end_inset: crate::types::meters(r, 2),
                    head_start: flag(r, 3),
                    head_end: flag(r, 4),
                }),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

impl PlanView {
    /// The settings of a new view named `name`.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            cut: DEFAULT_CUT,
            top: None,
            overhead: true,
            scale: DEFAULT_SCALE,
            detail: Detail::Medium,
            underlay: false,
            shown: Category::ALL.map(Category::shown_by_default),
            hidden: Vec::new(),
            bottom: 0.0,
            depth: -BOTTOM_MARGIN,
            datum_views: Vec::new(),
            north: North::Project,
        }
    }

    /// The settings of a view record; `None` when it is not readable.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let flag = |at: usize, default: bool| match values.get(at) {
            Some(Some(Value::Bool(b))) => *b,
            _ => default,
        };
        let shown = Category::ALL.map(|c| flag(c.slot(), c.shown_by_default()));
        let hidden = match values.get(plan_view::HIDDEN) {
            Some(Some(Value::List(items))) => items
                .iter()
                .take(crate::types::MAX_HIDDEN)
                .filter_map(|item| match item {
                    Value::Record(fields) => match fields.get(plan_view::HIDDEN_OBJECT) {
                        Some(Some(Value::Reference(id))) => Some(*id),
                        _ => None,
                    },
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let scale = match values.get(plan_view::SCALE) {
            Some(Some(Value::F64(s))) if s.is_finite() && *s > 0.0 => *s,
            _ => DEFAULT_SCALE,
        };
        Some(Self {
            name: crate::types::text(values, plan_view::NAME)?.to_owned(),
            cut: crate::types::meters(values, plan_view::CUT).unwrap_or(DEFAULT_CUT),
            top: crate::types::meters(values, plan_view::TOP),
            overhead: flag(plan_view::OVERHEAD, true),
            scale,
            detail: Detail::of_field(values, plan_view::DETAIL),
            underlay: flag(plan_view::UNDERLAY, false),
            shown,
            hidden,
            bottom: crate::types::meters(values, plan_view::BOTTOM).unwrap_or(0.0),
            depth: crate::types::meters(values, plan_view::DEPTH).unwrap_or(-BOTTOM_MARGIN),
            datum_views: datum_views_at(values),
            north: North::of_field(values, plan_view::NORTH),
        })
    }

    /// The record's values, in field order.
    #[must_use]
    pub fn values(&self) -> Vec<Option<Value>> {
        let mut out = vec![
            Some(Value::String(self.name.clone())),
            Some(crate::types::length(self.cut)),
            self.top.map(crate::types::length),
            Some(Value::Bool(self.overhead)),
            Some(Value::F64(self.scale)),
            Some(Value::Enum(self.detail.code())),
            Some(Value::Bool(self.underlay)),
        ];
        out.extend(self.shown.iter().map(|b| Some(Value::Bool(*b))));
        out.push(Some(hidden_value(&self.hidden)));
        out.push(Some(crate::types::length(self.bottom)));
        out.push(Some(crate::types::length(self.depth)));
        out.push(datum_views_value(&self.datum_views));
        out.push((self.north != North::Project).then(|| Value::Enum(self.north.code())));
        out
    }

    /// Whether the category is drawn.
    #[must_use]
    pub fn shows(&self, category: Category) -> bool {
        Category::ALL
            .iter()
            .position(|c| *c == category)
            .and_then(|k| self.shown.get(k))
            .copied()
            .unwrap_or(true)
    }

    /// The range of the view of a storey whose level is at `level`, with the next level at
    /// `above`: absolute elevations `(lowest, cut, top)`, the lowest being the view depth (or the
    /// bottom, when it lies lower: an invalid record still draws); the top is `None` without
    /// overhead.
    #[must_use]
    pub fn range(&self, level: f64, above: Option<f64>) -> (f64, f64, Option<f64>) {
        let cut = level + self.cut;
        let lowest = (level + self.depth.min(self.bottom)).min(cut - MIN_BAND);
        let top = match self.top {
            Some(t) => level + t,
            None => above
                .filter(|a| *a > cut + MIN_BAND)
                .unwrap_or(level + DEFAULT_TOP),
        };
        let top = (top > cut + MIN_BAND)
            .then_some(top)
            .or(Some(cut + DEFAULT_TOP));
        (lowest, cut, top.filter(|_| self.overhead))
    }
}

/// The value of the `hidden` field for `ids`.
#[must_use]
pub fn hidden_value(ids: &[ObjectId]) -> Value {
    Value::List(
        ids.iter()
            .map(|id| Value::Record(vec![Some(Value::Reference(*id))]))
            .collect(),
    )
}

/// The rule of the range: the view depth is not above the bottom, the bottom is below the cut and
/// the cut is below the top.
pub(crate) fn problem(object: &Object) -> Option<arc_base::Error> {
    let view = PlanView::from_values(&object.values)?;
    let bad = view.top.is_some_and(|top| top < view.cut + MIN_BAND)
        || view.bottom > view.cut - MIN_BAND
        || view.depth > view.bottom;
    bad.then(|| arc_base::Error::new(codes::VIEW_RANGE).with_arg("name", view.name))
}

/// The views of `storey`, by name.
#[must_use]
pub fn of_storey(snapshot: &Snapshot, storey: ObjectId) -> Vec<std::sync::Arc<Object>> {
    let mut found = spatial::children(snapshot, PLAN_VIEW, storey);
    found.truncate(MAX_VIEWS);
    found.sort_by(|a, b| {
        crate::types::text(&a.values, 0)
            .cmp(&crate::types::text(&b.values, 0))
            .then(a.id.cmp(&b.id))
    });
    found
}

/// Every plan view of the document, by storey elevation, then name.
#[must_use]
pub fn all(snapshot: &Snapshot) -> Vec<std::sync::Arc<Object>> {
    let mut found: Vec<(f64, std::sync::Arc<Object>)> = spatial::objects(snapshot, PLAN_VIEW)
        .into_iter()
        .take(MAX_VIEWS)
        .map(|v| {
            let at = v
                .header
                .owner
                .and_then(|s| spatial::get(snapshot, s))
                .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
            (at, v)
        })
        .collect();
    found.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then(crate::types::text(&a.1.values, 0).cmp(&crate::types::text(&b.1.values, 0)))
            .then(a.1.id.cmp(&b.1.id))
    });
    found.into_iter().map(|(_, v)| v).collect()
}

/// Whether `id` is something a plan can be shown of: a plan view record, or a storey, which stands
/// for its default view when it has no view of its own ([`settings_of`]).
#[must_use]
pub fn is_shown_view(snapshot: &Snapshot, id: ObjectId) -> bool {
    matches!(
        spatial::type_of(snapshot, id).as_deref(),
        Some(PLAN_VIEW | STOREY)
    )
}

/// The settings a plan of `object` is drawn with: a plan view record's own, or, for a storey (a
/// document saved before plan views has none of its own), those of a new view named after it. That
/// default view exists only while it is shown: opening it writes nothing to the document, so the
/// file stays as it was.
#[must_use]
pub fn settings_of(snapshot: &Snapshot, object: &Object) -> Option<PlanView> {
    match spatial::type_of(snapshot, object.id).as_deref() {
        Some(PLAN_VIEW) => PlanView::from_values(&object.values),
        Some(STOREY) => Some(PlanView::new(
            crate::types::text(&object.values, crate::types::storey::NAME).unwrap_or_default(),
        )),
        _ => None,
    }
}

/// The level of the storey a view belongs to (or of the storey itself, for its default view) and
/// the level above it, as elevations in meters.
#[must_use]
pub fn levels_of(snapshot: &Snapshot, view: &Object) -> Option<(ObjectId, f64, Option<f64>)> {
    let storey = if spatial::type_of(snapshot, view.id).as_deref() == Some(STOREY) {
        spatial::get(snapshot, view.id)?
    } else {
        spatial::get(snapshot, view.header.owner?)?
    };
    if spatial::type_of(snapshot, storey.id).as_deref() != Some(STOREY) {
        return None;
    }
    let level = spatial::storey_elevation(snapshot, &storey);
    let above = spatial::level_above(snapshot, &storey)
        .and_then(|l| crate::types::meters(&l.values, crate::types::level::ELEVATION));
    Some((storey.id, level, above))
}

/// The range of the view record `view` as absolute elevations `(bottom, cut, top)`.
///
/// # Errors
/// `ab.model.view_range` when the record is unreadable or its storey is missing.
pub fn range_of(snapshot: &Snapshot, view: &Object) -> Result<(f64, f64, Option<f64>)> {
    let invalid = || arc_base::Error::new(codes::VIEW_RANGE).with_arg("name", String::new());
    let settings = PlanView::from_values(&view.values).ok_or_else(invalid)?;
    let (_, level, above) = levels_of(snapshot, view).ok_or_else(invalid)?;
    Ok(settings.range(level, above))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_round_trips_through_its_values() {
        let mut view = PlanView::new("Ground");
        view.top = Some(2.8);
        view.detail = Detail::Fine;
        view.shown[3] = true;
        view.bottom = -0.2;
        view.depth = -0.5;
        let again = PlanView::from_values(&view.values());
        assert_eq!(again.as_ref(), Some(&view));
    }

    #[test]
    fn the_range_takes_the_next_level_else_three_meters() {
        let view = PlanView::new("v");
        assert!(!view.shows(Category::Ceiling) && view.shows(Category::Wall));
        assert_eq!(view.range(0.0, Some(3.2)), (-BOTTOM_MARGIN, 1.2, Some(3.2)));
        assert_eq!(view.range(0.0, None), (-BOTTOM_MARGIN, 1.2, Some(3.0)));
        let mut high = view;
        high.cut = 3.5;
        assert_eq!(high.range(0.0, Some(3.2)).2, Some(6.5));
        high.overhead = false;
        assert_eq!(high.range(0.0, Some(3.2)).2, None);
    }
}
