//! The datums of a building: grids (AB-DATUM-009), their extents across levels (AB-DATUM-010),
//! reference planes (AB-DATUM-011) and work planes (AB-DATUM-012).
//!
//! A grid is a labeled line in plan, straight or an arc (a bulge as a floor edge has), that
//! reaches from a bottom to a top level: it shows on every storey whose level lies in that range
//! and as a vertical plane in 3D. A reference plane is a named vertical plane through a line,
//! bounded or running on, with a [`Strength`] that decides whether snapping finds it. A work plane
//! is the plane placement and sketching work on; it is authored as a source (a level, a grid, a
//! reference plane or three points) and resolved here ([`Plane`]), so moving the datum moves the
//! plane. Records hold the authored data only: the polylines, the extents and the planes are
//! derived on every read.
//!
//! Labels increment in numeric (`1`, `2`), alphabetic (`A`, `B`, ..., `Z`, `AA`) and sub-grid
//! (`A.1`, `A.2`) sequences ([`increment`], [`next_label`]); a label used twice in a building is
//! reported ([`duplicate_labels`]) and never refused, as drawing both comes first.

use arc_base::ObjectId;
use arc_doc::{Object, Snapshot, Value};

use crate::spatial;
use crate::types::{self, GRID, REFERENCE_PLANE, WORK_PLANE, grid, reference_plane, work_plane};

/// The shortest grid or reference plane, in meters.
pub const MIN_LENGTH: f64 = 0.001;
/// How far an unbounded reference plane is drawn and snapped on each side of its points.
pub const UNBOUNDED_REACH: f64 = 200.0;
/// The distance from the arc within which a grid's polyline follows it, in meters.
pub const FLAT_CHORD: f64 = 0.01;
/// The most datums of one kind that are looked at.
pub const MAX_PER_KIND: usize = 20_000;
/// The most vertices between the ends of a multi-segment grid.
pub const MAX_PATH: usize = 1_000;
/// The most labels one increment searches through for a free one.
const MAX_SEARCH: usize = 100_000;

type P = [f64; 2];

coded! {
    /// How strongly a reference plane attracts snapping and dimensions.
    Strength {
        /// Snaps first.
        Strong = 0, "strong";
        /// Snaps.
        #[default]
        Weak = 1, "weak";
        /// Does not snap: a plane that only hosts and constrains.
        Unsnapped = 2, "none";
    }
}

coded! {
    /// Where a work plane comes from.
    Source {
        /// The horizontal plane of a level.
        #[default]
        Level = 0, "level";
        /// The vertical plane of a grid.
        Grid = 1, "grid";
        /// The plane of a reference plane.
        ReferencePlane = 2, "reference_plane";
        /// The plane through three points.
        Points = 3, "points";
        /// A face of a wall, floor or ceiling.
        Face = 4, "face";
    }
}

coded! {
    /// Which face of an element a work plane lies on.
    FaceSide {
        /// The outer face of a wall: the side the baseline's left hand is on.
        #[default]
        Exterior = 0, "exterior";
        /// The inner face of a wall.
        Interior = 1, "interior";
        /// The top of a floor or ceiling.
        Top = 2, "top";
        /// The underside of a floor or ceiling.
        Bottom = 3, "bottom";
    }
}

impl FaceSide {
    /// The face of a work plane record's field values.
    #[must_use]
    pub fn of_values(values: &[Option<Value>]) -> Self {
        Self::of_field(values, work_plane::FACE)
    }

    /// Whether an element of type `key` has this face.
    #[must_use]
    pub fn on(self, key: &str) -> bool {
        match key {
            types::WALL => matches!(self, Self::Exterior | Self::Interior),
            types::FLOOR | types::CEILING => matches!(self, Self::Top | Self::Bottom),
            _ => false,
        }
    }

    /// The face a work plane takes from an element of type `key` when none is named.
    #[must_use]
    pub fn default_of(key: &str) -> Option<Self> {
        match key {
            types::WALL => Some(Self::Exterior),
            types::FLOOR => Some(Self::Top),
            types::CEILING => Some(Self::Bottom),
            _ => None,
        }
    }
}

impl Source {
    /// The source of a work plane record's field values.
    #[must_use]
    pub fn of_values(values: &[Option<Value>]) -> Self {
        Self::of_field(values, work_plane::SOURCE)
    }
}

fn bool_at(values: &[Option<Value>], at: usize, default: bool) -> bool {
    match values.get(at) {
        Some(Some(Value::Bool(b))) => *b,
        _ => default,
    }
}

fn reference_at(values: &[Option<Value>], at: usize) -> Option<ObjectId> {
    match values.get(at) {
        Some(Some(Value::Reference(id))) => Some(*id),
        _ => None,
    }
}

fn point_at(values: &[Option<Value>], x: usize, y: usize) -> Option<P> {
    Some([types::meters(values, x)?, types::meters(values, y)?])
}

// ----- grids ---------------------------------------------------------------------------------

/// The authored data of a grid line.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// The label.
    pub label: String,
    /// The start.
    pub start: P,
    /// The end.
    pub end: P,
    /// The bulge: 0 for a straight line.
    pub bulge: f64,
    /// Whether the start shows its head.
    pub head_start: bool,
    /// Whether the end shows its head.
    pub head_end: bool,
    /// The level the extent starts at; the building's lowest when absent.
    pub bottom: Option<ObjectId>,
    /// The level the extent ends at; the building's highest when absent.
    pub top: Option<ObjectId>,
    /// The vertices between start and end, each with the bulge of the segment that arrives at it
    /// (`bulge` is that of the last segment); empty for a grid of one segment.
    pub via: Vec<(P, f64)>,
}

/// The vertices of a grid's `path` field, in order; unreadable entries are skipped.
fn path_at(values: &[Option<Value>]) -> Vec<(P, f64)> {
    let Some(Some(Value::List(items))) = values.get(grid::PATH) else {
        return Vec::new();
    };
    items
        .iter()
        .take(MAX_PATH)
        .filter_map(|item| match item {
            Value::Record(r) => Some((
                [types::meters(r, 0)?, types::meters(r, 1)?],
                match r.get(2) {
                    Some(Some(Value::F64(b))) if b.is_finite() => *b,
                    _ => 0.0,
                },
            )),
            _ => None,
        })
        .collect()
}

/// The value of the `path` field for `via`; absent when there is none.
#[must_use]
pub fn path_value(via: &[(P, f64)]) -> Option<Value> {
    (!via.is_empty()).then(|| {
        Value::List(
            via.iter()
                .map(|(p, bulge)| {
                    Value::Record(vec![
                        Some(types::length(p[0])),
                        Some(types::length(p[1])),
                        Some(Value::F64(*bulge)),
                    ])
                })
                .collect(),
        )
    })
}

impl Grid {
    /// The grid of a record's field values; `None` when a coordinate is missing.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        Some(Self {
            label: types::text(values, grid::NAME)?.to_owned(),
            start: point_at(values, grid::START_X, grid::START_Y)?,
            end: point_at(values, grid::END_X, grid::END_Y)?,
            bulge: match values.get(grid::BULGE) {
                Some(Some(Value::F64(b))) if b.is_finite() => *b,
                _ => 0.0,
            },
            head_start: bool_at(values, grid::HEAD_START, true),
            head_end: bool_at(values, grid::HEAD_END, true),
            bottom: reference_at(values, grid::BOTTOM_LEVEL),
            top: reference_at(values, grid::TOP_LEVEL),
            via: path_at(values),
        })
    }

    /// The control points: start, the vertices between and end.
    #[must_use]
    pub fn points(&self) -> Vec<P> {
        let mut out = vec![self.start];
        out.extend(self.via.iter().map(|(p, _)| *p));
        out.push(self.end);
        out
    }

    /// The length along the control points (chords), so a grid that closes on itself has one.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.points()
            .windows(2)
            .map(|w| match (w.first(), w.get(1)) {
                (Some(a), Some(b)) => (b[0] - a[0]).hypot(b[1] - a[1]),
                _ => 0.0,
            })
            .sum()
    }

    /// The points of the line from start to end through every vertex, each arc flattened within
    /// [`FLAT_CHORD`].
    #[must_use]
    pub fn polyline(&self) -> Vec<P> {
        let points = self.points();
        let bulges = self.via.iter().map(|(_, b)| *b).chain([self.bulge]);
        let mut out = Vec::new();
        for (w, bulge) in points.windows(2).zip(bulges) {
            if let (Some(a), Some(b)) = (w.first(), w.get(1)) {
                out.extend(crate::floor::flatten_edge(*a, *b, bulge, FLAT_CHORD));
            }
        }
        out.push(self.end);
        out
    }

    /// The direction the line leaves its start in and the direction it arrives at its end in, as
    /// unit vectors (the same for a straight line).
    #[must_use]
    pub fn directions(&self) -> (P, P) {
        line_directions(&self.polyline())
    }

    /// The grid as the view with the override `over` draws it: the line pulled in or pushed out
    /// at each end and the heads that show. `None` when the view pulls the line in past its other
    /// end (it shows nothing).
    #[must_use]
    pub fn in_view(&self, over: Option<&crate::view::DatumView>) -> Option<ViewGrid> {
        let line = trimmed(
            &self.polyline(),
            over.and_then(|o| o.start_inset).unwrap_or(0.0),
            over.and_then(|o| o.end_inset).unwrap_or(0.0),
        )?;
        let (d0, d1) = line_directions(&line);
        Some(ViewGrid {
            start: *line.first()?,
            end: *line.last()?,
            line,
            toward_start: [-d0[0], -d0[1]],
            toward_end: d1,
            head_start: over.and_then(|o| o.head_start).unwrap_or(self.head_start),
            head_end: over.and_then(|o| o.head_end).unwrap_or(self.head_end),
        })
    }
}

/// A grid as one view draws it ([`Grid::in_view`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ViewGrid {
    /// The line as drawn.
    pub line: Vec<P>,
    /// Where it starts as drawn.
    pub start: P,
    /// Where it ends as drawn.
    pub end: P,
    /// The unit direction that leaves the line at its start.
    pub toward_start: P,
    /// The unit direction that leaves the line at its end.
    pub toward_end: P,
    /// Whether the start shows its head.
    pub head_start: bool,
    /// Whether the end shows its head.
    pub head_end: bool,
}

fn line_directions(line: &[P]) -> (P, P) {
    let unit = |a: P, b: P| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let d = dx.hypot(dy);
        if d > 0.0 {
            [dx / d, dy / d]
        } else {
            [1.0, 0.0]
        }
    };
    let n = line.len();
    match (
        line.first(),
        line.get(1),
        n.checked_sub(2).and_then(|k| line.get(k)),
        line.last(),
    ) {
        (Some(a), Some(b), Some(c), Some(d)) => (unit(*a, *b), unit(*c, *d)),
        _ => ([1.0, 0.0], [1.0, 0.0]),
    }
}

/// The length of a polyline.
fn path_length(line: &[P]) -> f64 {
    line.windows(2)
        .map(|w| match (w.first(), w.get(1)) {
            (Some(a), Some(b)) => (b[0] - a[0]).hypot(b[1] - a[1]),
            _ => 0.0,
        })
        .sum()
}

/// `line` without its first `d` meters.
fn cut_front(line: &[P], mut d: f64) -> Vec<P> {
    let mut rest = line;
    while let [a, b, ..] = rest {
        let s = (b[0] - a[0]).hypot(b[1] - a[1]);
        if d < s {
            let t = d / s;
            let mut out = vec![[a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]];
            out.extend_from_slice(rest.get(1..).unwrap_or_default());
            return out;
        }
        d -= s;
        rest = rest.get(1..).unwrap_or_default();
    }
    rest.to_vec()
}

/// `line` with its start moved by `inset` along the line: pulled in when positive, pushed out
/// along the first segment when negative.
fn move_start(line: &[P], inset: f64) -> Vec<P> {
    if inset > 0.0 {
        return cut_front(line, inset);
    }
    let mut out = line.to_vec();
    if let [a, b, ..] = line {
        let d = (b[0] - a[0]).hypot(b[1] - a[1]);
        if inset < 0.0 && d > 0.0 {
            let t = inset / d;
            out.insert(0, [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    out
}

/// `line` with its two ends moved by the insets; `None` when nothing is left.
fn trimmed(line: &[P], start_inset: f64, end_inset: f64) -> Option<Vec<P>> {
    let pulled = start_inset.max(0.0) + end_inset.max(0.0);
    if !(start_inset.is_finite() && end_inset.is_finite())
        || pulled >= path_length(line) - MIN_LENGTH
    {
        return None;
    }
    let mut v = move_start(line, start_inset);
    v.reverse();
    let mut v = move_start(&v, end_inset);
    v.reverse();
    Some(v)
}

/// The grids of a building, in identifier order.
#[must_use]
pub fn grids(snapshot: &Snapshot, building: ObjectId) -> Vec<std::sync::Arc<Object>> {
    spatial::children(snapshot, GRID, building)
}

/// The elevation of the level `id`, in meters.
fn elevation(snapshot: &Snapshot, id: ObjectId) -> Option<f64> {
    spatial::get(snapshot, id).and_then(|l| types::meters(&l.values, types::level::ELEVATION))
}

/// The vertical extent of a grid or reference plane that belongs to `building`: from the level
/// it starts at (else the lowest level) to the level it ends at (else the highest), in meters;
/// at least [`crate::defaults::STOREY_HEIGHT`] tall.
#[must_use]
pub fn extent(
    snapshot: &Snapshot,
    building: Option<ObjectId>,
    bottom: Option<ObjectId>,
    top: Option<ObjectId>,
) -> (f64, f64) {
    let levels: Vec<f64> = building
        .map(|b| spatial::levels(snapshot, b))
        .unwrap_or_default()
        .iter()
        .filter_map(|l| types::meters(&l.values, types::level::ELEVATION))
        .collect();
    let low = bottom
        .and_then(|b| elevation(snapshot, b))
        .or_else(|| levels.first().copied())
        .unwrap_or(0.0);
    let high = top
        .and_then(|t| elevation(snapshot, t))
        .or_else(|| levels.last().copied())
        .unwrap_or(low);
    if high - low < crate::defaults::STOREY_HEIGHT {
        (low, low + crate::defaults::STOREY_HEIGHT)
    } else {
        (low, high)
    }
}

/// The vertical extent of the grid record `object`.
#[must_use]
pub fn grid_extent(snapshot: &Snapshot, object: &Object) -> (f64, f64) {
    let (bottom, top) = (
        reference_at(&object.values, grid::BOTTOM_LEVEL),
        reference_at(&object.values, grid::TOP_LEVEL),
    );
    extent(snapshot, object.header.owner, bottom, top)
}

/// Whether the grid `object` shows on a storey whose level is at `level`: the level lies within
/// its extent (a millimeter beyond counts).
#[must_use]
pub fn shows_at(snapshot: &Snapshot, object: &Object, level: f64) -> bool {
    let (low, high) = grid_extent(snapshot, object);
    level >= low - 1.0e-3 && level <= high + 1.0e-3
}

// ----- labels --------------------------------------------------------------------------------

/// The label after `label` in its sequence: a number (`9` to `10`, `007` to `008`), letters
/// (`A` to `B`, `Z` to `AA`, case kept), a sub-grid (`A.1` to `A.2`: the part after the last dot
/// increments) or a label that ends in a number (`G7` to `G8`). `None` for a label that has no
/// sequence, such as `North`.
#[must_use]
pub fn increment(label: &str) -> Option<String> {
    let label = label.trim();
    if let Some((head, tail)) = label.rsplit_once('.') {
        return increment(tail).map(|next| format!("{head}.{next}"));
    }
    if !label.is_empty() && label.bytes().all(|b| b.is_ascii_digit()) {
        let width = label.len();
        let next = label.parse::<u64>().ok()?.checked_add(1)?;
        return Some(format!("{next:0width$}"));
    }
    if !label.is_empty()
        && (label.bytes().all(|b| b.is_ascii_uppercase())
            || label.bytes().all(|b| b.is_ascii_lowercase()))
    {
        let base = if label.starts_with(|c: char| c.is_ascii_uppercase()) {
            b'A'
        } else {
            b'a'
        };
        let mut digits: Vec<u8> = label.bytes().map(|b| b - base).collect();
        let mut at = digits.len();
        loop {
            let Some(k) = at.checked_sub(1) else {
                digits.insert(0, 0);
                break;
            };
            at = k;
            match digits.get_mut(k) {
                Some(d) if *d < 25 => {
                    *d += 1;
                    break;
                }
                Some(d) => *d = 0,
                None => break,
            }
        }
        return Some(digits.into_iter().map(|d| char::from(base + d)).collect());
    }
    // A prefix and a number at the end.
    let split = label.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (head, tail) = label.split_at(split);
    if !head.is_empty() && !tail.is_empty() {
        return increment(tail).map(|next| format!("{head}{next}"));
    }
    None
}

/// How far along its sequence a label is, for finding the last one: numbers before letters, each
/// by value.
fn rank(label: &str) -> (u8, u64, String) {
    let label = label.trim();
    if let Ok(n) = label.parse::<u64>() {
        return (1, n, String::new());
    }
    if !label.is_empty() && label.bytes().all(|b| b.is_ascii_alphabetic()) {
        let value = label.bytes().fold(0_u64, |acc, b| {
            acc.saturating_mul(26)
                .saturating_add(u64::from(b.to_ascii_uppercase() - b'A') + 1)
        });
        return (0, value, String::new());
    }
    (0, 0, label.to_owned())
}

/// The label a new grid gets: the one after `after` when it is given, else after the last of
/// `taken` (the highest number, else the furthest letters); `1` for the first. A label that is
/// taken (compared as names are) is skipped.
#[must_use]
pub fn next_label(taken: &[&str], after: Option<&str>) -> String {
    let free = |label: &str| !taken.iter().any(|t| crate::names::same(t, label));
    let last = after.map(str::to_owned).or_else(|| {
        taken
            .iter()
            .max_by_key(|t| rank(t))
            .map(|t| (*t).to_owned())
    });
    let mut candidate = last
        .as_deref()
        .and_then(increment)
        .unwrap_or_else(|| "1".to_owned());
    for _ in 0..MAX_SEARCH {
        if free(&candidate) {
            return candidate;
        }
        candidate = increment(&candidate).unwrap_or_else(|| format!("{candidate}1"));
    }
    candidate
}

/// The labels used by more than one grid of a building, each with the grids that use it.
#[must_use]
pub fn duplicate_labels(snapshot: &Snapshot) -> Vec<(String, Vec<ObjectId>)> {
    let mut by: std::collections::BTreeMap<(Option<ObjectId>, String), Vec<ObjectId>> =
        std::collections::BTreeMap::new();
    for object in spatial::objects(snapshot, GRID).iter().take(MAX_PER_KIND) {
        let label = types::text(&object.values, grid::NAME).unwrap_or_default();
        by.entry((object.header.owner, crate::names::fold(label)))
            .or_default()
            .push(object.id);
    }
    by.into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .filter_map(|(_, ids)| {
            let label = ids
                .first()
                .and_then(|id| spatial::get(snapshot, *id))
                .and_then(|o| types::text(&o.values, grid::NAME).map(str::to_owned))?;
            Some((label, ids))
        })
        .collect()
}

// ----- reference planes ----------------------------------------------------------------------

/// The authored data of a reference plane.
#[derive(Debug, Clone, PartialEq)]
pub struct RefPlane {
    /// The name.
    pub name: String,
    /// The first point of the line.
    pub start: P,
    /// The second point of the line.
    pub end: P,
    /// How strongly it snaps.
    pub strength: Strength,
    /// Whether it ends at the two points.
    pub bounded: bool,
}

impl RefPlane {
    /// The reference plane of a record's field values.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        Some(Self {
            name: types::text(values, reference_plane::NAME)?.to_owned(),
            start: point_at(values, reference_plane::START_X, reference_plane::START_Y)?,
            end: point_at(values, reference_plane::END_X, reference_plane::END_Y)?,
            strength: Strength::of_field(values, reference_plane::STRENGTH),
            bounded: bool_at(values, reference_plane::BOUNDED, true),
        })
    }

    /// The line as drawn and snapped: the two points, or for an unbounded plane the same line run
    /// [`UNBOUNDED_REACH`] beyond each.
    #[must_use]
    pub fn line(&self) -> [P; 2] {
        let (dx, dy) = (self.end[0] - self.start[0], self.end[1] - self.start[1]);
        let length = dx.hypot(dy);
        if self.bounded || length <= 0.0 {
            return [self.start, self.end];
        }
        let (ux, uy) = (dx / length, dy / length);
        [
            [
                self.start[0] - ux * UNBOUNDED_REACH,
                self.start[1] - uy * UNBOUNDED_REACH,
            ],
            [
                self.end[0] + ux * UNBOUNDED_REACH,
                self.end[1] + uy * UNBOUNDED_REACH,
            ],
        ]
    }
}

// ----- lines, intersections and picking ------------------------------------------------------

/// What lies on a line a snap or a pick finds.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The datum.
    pub id: ObjectId,
    /// The segments of the line; consecutive ones of an arc.
    pub points: Vec<P>,
    /// Whether it is a reference plane of strength [`Strength::Strong`].
    pub strong: bool,
}

/// The grids that show at `level` (every grid without one) and the reference planes that snap:
/// the lines of the datums of `snapshot`, for snapping and picking in plan.
#[must_use]
pub fn lines(snapshot: &Snapshot, level: Option<f64>) -> Vec<Line> {
    let mut out = Vec::new();
    for object in spatial::objects(snapshot, GRID).iter().take(MAX_PER_KIND) {
        let Some(g) = Grid::from_values(&object.values) else {
            continue;
        };
        if level.is_some_and(|z| !shows_at(snapshot, object, z)) {
            continue;
        }
        out.push(Line {
            id: object.id,
            points: g.polyline(),
            strong: false,
        });
    }
    for object in spatial::objects(snapshot, REFERENCE_PLANE)
        .iter()
        .take(MAX_PER_KIND)
    {
        let Some(r) = RefPlane::from_values(&object.values) else {
            continue;
        };
        if r.strength == Strength::Unsnapped {
            continue;
        }
        out.push(Line {
            id: object.id,
            points: r.line().to_vec(),
            strong: r.strength == Strength::Strong,
        });
    }
    out
}

/// The point where the segments `a` and `b` cross, strictly inside both.
#[must_use]
pub fn cross(a: [P; 2], b: [P; 2]) -> Option<P> {
    let (r, s) = (
        [a[1][0] - a[0][0], a[1][1] - a[0][1]],
        [b[1][0] - b[0][0], b[1][1] - b[0][1]],
    );
    let denominator = r[0] * s[1] - r[1] * s[0];
    if denominator.abs() < 1.0e-12 {
        return None;
    }
    let d = [b[0][0] - a[0][0], b[0][1] - a[0][1]];
    let t = (d[0] * s[1] - d[1] * s[0]) / denominator;
    let u = (d[0] * r[1] - d[1] * r[0]) / denominator;
    let inside = |v: f64| (-1.0e-9..=1.0 + 1.0e-9).contains(&v);
    (inside(t) && inside(u)).then(|| [a[0][0] + r[0] * t, a[0][1] + r[1] * t])
}

/// The points where the lines of two different datums cross, in the order of `lines`.
#[must_use]
pub fn intersections(lines: &[Line]) -> Vec<P> {
    let mut out: Vec<P> = Vec::new();
    for (i, a) in lines.iter().enumerate() {
        for b in lines.iter().skip(i + 1) {
            for sa in a.points.windows(2) {
                for sb in b.points.windows(2) {
                    if let (Some(p), Some(q), Some(r), Some(s)) =
                        (sa.first(), sa.get(1), sb.first(), sb.get(1))
                        && let Some(at) = cross([*p, *q], [*r, *s])
                        && !out
                            .iter()
                            .any(|o| (o[0] - at[0]).hypot(o[1] - at[1]) < 1.0e-6)
                    {
                        out.push(at);
                    }
                }
            }
        }
    }
    out
}

/// The open line of the grid or reference plane `object`, as the hover outlines it: its points
/// at the height its extent starts at.
#[must_use]
pub fn outline(snapshot: &Snapshot, object: &Object) -> Vec<Vec<[f64; 3]>> {
    let line = match spatial::type_of(snapshot, object.id).as_deref() {
        Some(GRID) => Grid::from_values(&object.values)
            .map(|g| (g.polyline(), grid_extent(snapshot, object).0)),
        Some(REFERENCE_PLANE) => RefPlane::from_values(&object.values).map(|r| {
            (
                r.line().to_vec(),
                extent(snapshot, object.header.owner, None, None).0,
            )
        }),
        _ => None,
    };
    line.map(|(points, z)| vec![points.iter().map(|p| [p[0], p[1], z]).collect()])
        .unwrap_or_default()
}

/// The distance from `p` to the segment `a`-`b`.
fn segment_distance(a: P, b: P, p: P) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - d[0] * t).hypot(p[1] - a[1] - d[1] * t)
}

/// The datums whose line passes within `aperture` of `at`, the nearest first; reference planes
/// that do not snap are picked too (they are drawn).
#[must_use]
pub fn pick(snapshot: &Snapshot, at: P, aperture: f64) -> Vec<ObjectId> {
    let mut found: Vec<(f64, ObjectId)> = Vec::new();
    let mut offer = |id: ObjectId, points: &[P]| {
        let miss = points
            .windows(2)
            .filter_map(|w| Some(segment_distance(*w.first()?, *w.get(1)?, at)))
            .fold(f64::MAX, f64::min);
        if miss <= aperture {
            found.push((miss, id));
        }
    };
    for object in spatial::objects(snapshot, GRID).iter().take(MAX_PER_KIND) {
        if let Some(g) = Grid::from_values(&object.values) {
            offer(object.id, &g.polyline());
        }
    }
    for object in spatial::objects(snapshot, REFERENCE_PLANE)
        .iter()
        .take(MAX_PER_KIND)
    {
        if let Some(r) = RefPlane::from_values(&object.values) {
            offer(object.id, &r.line());
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, id)| id).collect()
}

// ----- work planes ---------------------------------------------------------------------------

/// A resolved plane: a point on it, its unit normal and the unit direction of its first axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// A point on the plane.
    pub origin: [f64; 3],
    /// The unit normal.
    pub normal: [f64; 3],
    /// The unit direction of the plane's x axis.
    pub u: [f64; 3],
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: [f64; 3]) -> Option<[f64; 3]> {
    let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (n.is_finite() && n > 1.0e-9).then(|| [a[0] / n, a[1] / n, a[2] / n])
}

impl Plane {
    /// The plane through three points: its x axis runs from the first to the second, its normal
    /// follows the right-hand rule through the third. `None` for points on one line.
    #[must_use]
    pub fn through(p: [[f64; 3]; 3]) -> Option<Self> {
        let u = unit3(sub(p[1], p[0]))?;
        let normal = unit3(cross3(u, sub(p[2], p[0])))?;
        Some(Self {
            origin: p[0],
            normal,
            u,
        })
    }

    /// The vertical plane through the line `a`-`b` of plan, standing on `z`.
    #[must_use]
    pub fn vertical(a: P, b: P, z: f64) -> Option<Self> {
        let u = unit3([b[0] - a[0], b[1] - a[1], 0.0])?;
        Some(Self {
            origin: [a[0], a[1], z],
            normal: [-u[1], u[0], 0.0],
            u,
        })
    }

    /// The horizontal plane at `z`.
    #[must_use]
    pub fn level(z: f64) -> Self {
        Self {
            origin: [0.0, 0.0, z],
            normal: [0.0, 0.0, 1.0],
            u: [1.0, 0.0, 0.0],
        }
    }
}

fn from_slope(plane: &crate::terrain::Plane, lowered: f64, up: bool) -> Option<Plane> {
    let [gx, gy] = plane.grad;
    let normal = unit3([-gx, -gy, 1.0])?;
    let u = unit3([1.0, 0.0, gx])?;
    let sign = if up { 1.0 } else { -1.0 };
    Some(Plane {
        origin: [plane.at[0], plane.at[1], plane.z - lowered],
        normal: [normal[0] * sign, normal[1] * sign, normal[2] * sign],
        u,
    })
}

/// The plane of the face `side` of the wall, floor or ceiling `element`: a wall's vertical face
/// standing on its base, a slab's top or underside (sloped as the slab is). Its normal points
/// away from the body. `None` for another kind, a face the kind has not, or an element that cannot
/// be read.
#[must_use]
pub fn face_plane(snapshot: &Snapshot, element: &Object, side: FaceSide) -> Option<Plane> {
    let key = spatial::type_of(snapshot, element.id)?;
    if !side.on(&key) {
        return None;
    }
    let level = element
        .header
        .owner
        .and_then(|s| spatial::get(snapshot, s))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    match key.as_str() {
        types::WALL => {
            let wall = crate::wall::Wall::of(snapshot, element)?;
            let (low, high) = wall.extent();
            let u = unit3([
                wall.end[0] - wall.start[0],
                wall.end[1] - wall.start[1],
                0.0,
            ])?;
            let (left, offset) = match side {
                FaceSide::Interior => (-1.0, low),
                _ => (1.0, high),
            };
            Some(Plane {
                origin: [
                    wall.start[0] - u[1] * offset,
                    wall.start[1] + u[0] * offset,
                    level + wall.base_offset,
                ],
                normal: [-u[1] * left, u[0] * left, 0.0],
                u,
            })
        }
        types::FLOOR => {
            let d = crate::floor::Derived::of(snapshot, element)?;
            match side {
                FaceSide::Top => from_slope(&d.plane, 0.0, true),
                _ => from_slope(&d.plane, d.floor.thickness, false),
            }
        }
        types::CEILING => {
            let d = crate::ceiling::derived(snapshot, &spatial::get(snapshot, element.id)?)?;
            match side {
                FaceSide::Bottom => from_slope(&d.plane, 0.0, false),
                _ => from_slope(&d.plane, -d.ceiling.thickness(), true),
            }
        }
        _ => None,
    }
}

/// The plane the work plane record `object` stands for, now: a level's elevation, the line of a
/// grid or reference plane, the three points, or a face of an element. `None` when the source is gone or the points lie
/// on one line.
#[must_use]
pub fn resolve(snapshot: &Snapshot, object: &Object) -> Option<Plane> {
    let v = &object.values;
    match Source::of_field(v, work_plane::SOURCE) {
        Source::Level => {
            elevation(snapshot, reference_at(v, work_plane::TARGET)?).map(Plane::level)
        }
        Source::Grid => {
            let target = spatial::get(snapshot, reference_at(v, work_plane::TARGET)?)?;
            let g = Grid::from_values(&target.values)?;
            Plane::vertical(g.start, g.end, grid_extent(snapshot, &target).0)
        }
        Source::ReferencePlane => {
            let target = spatial::get(snapshot, reference_at(v, work_plane::TARGET)?)?;
            let r = RefPlane::from_values(&target.values)?;
            let (low, _) = extent(snapshot, target.header.owner, None, None);
            Plane::vertical(r.start, r.end, low)
        }
        Source::Face => {
            let element = spatial::get(snapshot, reference_at(v, work_plane::FACE_ELEMENT)?)?;
            face_plane(snapshot, &element, FaceSide::of_field(v, work_plane::FACE))
        }
        Source::Points => {
            let at = |k: usize| -> Option<[f64; 3]> {
                let base = work_plane::POINTS + 3 * k;
                Some([
                    types::meters(v, base)?,
                    types::meters(v, base + 1)?,
                    types::meters(v, base + 2)?,
                ])
            };
            Plane::through([at(0)?, at(1)?, at(2)?])
        }
    }
}

/// The active work plane of `building`: its record and the plane it stands for.
#[must_use]
pub fn active(
    snapshot: &Snapshot,
    building: ObjectId,
) -> Option<(std::sync::Arc<Object>, Option<Plane>)> {
    spatial::children(snapshot, WORK_PLANE, building)
        .into_iter()
        .find(|o| bool_at(&o.values, work_plane::ACTIVE, false))
        .map(|o| {
            let plane = resolve(snapshot, &o);
            (o, plane)
        })
}

/// Whether the datum record `object` of type `key` is malformed in a way a document must not
/// hold: a grid or reference plane shorter than [`MIN_LENGTH`], or a work plane that follows a
/// datum of the wrong kind or three points on a line.
#[must_use]
pub fn is_degenerate(snapshot: &Snapshot, key: &str, object: &Object) -> bool {
    match key {
        GRID => Grid::from_values(&object.values).is_none_or(|g| g.length() < MIN_LENGTH),
        REFERENCE_PLANE => RefPlane::from_values(&object.values)
            .is_none_or(|r| (r.end[0] - r.start[0]).hypot(r.end[1] - r.start[1]) < MIN_LENGTH),
        WORK_PLANE => {
            let source = Source::of_field(&object.values, work_plane::SOURCE);
            let expected = match source {
                Source::Level => Some(types::LEVEL),
                Source::Grid => Some(GRID),
                Source::ReferencePlane => Some(REFERENCE_PLANE),
                Source::Points | Source::Face => None,
            };
            let target = reference_at(&object.values, work_plane::TARGET);
            match (expected, target) {
                (Some(key), Some(id)) => spatial::type_of(snapshot, id).as_deref() != Some(key),
                (Some(_), None) => true,
                (None, _) => resolve(snapshot, object).is_none(),
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_increment_in_every_sequence() {
        assert_eq!(increment("1").as_deref(), Some("2"));
        assert_eq!(increment("9").as_deref(), Some("10"));
        assert_eq!(increment("007").as_deref(), Some("008"));
        assert_eq!(increment("A").as_deref(), Some("B"));
        assert_eq!(increment("Z").as_deref(), Some("AA"));
        assert_eq!(increment("AZ").as_deref(), Some("BA"));
        assert_eq!(increment("ZZ").as_deref(), Some("AAA"));
        assert_eq!(increment("c").as_deref(), Some("d"));
        assert_eq!(increment("A.1").as_deref(), Some("A.2"));
        assert_eq!(increment("A.9").as_deref(), Some("A.10"));
        assert_eq!(increment("G7").as_deref(), Some("G8"));
        assert_eq!(increment("North"), None);
        assert_eq!(increment(""), None);
    }

    #[test]
    fn the_next_label_skips_taken_ones_and_starts_at_one() {
        assert_eq!(next_label(&[], None), "1");
        assert_eq!(next_label(&["1", "2", "3"], None), "4");
        assert_eq!(next_label(&["A", "B"], None), "C");
        assert_eq!(next_label(&["A", "B", "C"], Some("A")), "D");
        assert_eq!(next_label(&["1", "A"], Some("A")), "B");
        // Compared as names are: case and surrounding space do not make a label free.
        assert_eq!(next_label(&["a", "B"], Some("A")), "C");
        // A label without a sequence starts the numbers.
        assert_eq!(next_label(&["North"], None), "1");
    }

    #[test]
    fn segments_cross_inside_both_only() {
        assert_eq!(
            cross([[0.0, 0.0], [4.0, 0.0]], [[2.0, -1.0], [2.0, 1.0]]),
            Some([2.0, 0.0])
        );
        assert_eq!(
            cross([[0.0, 0.0], [1.0, 0.0]], [[2.0, -1.0], [2.0, 1.0]]),
            None
        );
        assert_eq!(
            cross([[0.0, 0.0], [4.0, 0.0]], [[0.0, 1.0], [4.0, 1.0]]),
            None
        );
    }

    #[test]
    fn a_line_is_pulled_in_or_pushed_out_along_its_segments() {
        let line = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0]];
        assert_eq!(
            trimmed(&line, 1.0, 1.0),
            Some(vec![[1.0, 0.0], [4.0, 0.0], [4.0, 2.0]])
        );
        // Past a vertex, along the next segment.
        assert_eq!(trimmed(&line, 5.0, 0.0), Some(vec![[4.0, 1.0], [4.0, 3.0]]));
        // Pushed out along the end segments.
        assert_eq!(
            trimmed(&line, -2.0, -1.0),
            Some(vec![
                [-2.0, 0.0],
                [0.0, 0.0],
                [4.0, 0.0],
                [4.0, 3.0],
                [4.0, 4.0]
            ])
        );
        // Nothing is left of a line pulled in past itself, or by a value that is not a number.
        assert_eq!(trimmed(&line, 4.0, 3.0), None);
        assert_eq!(trimmed(&line, f64::NAN, 0.0), None);
    }

    #[test]
    fn a_plane_through_three_points_refuses_a_line() {
        let plane = Plane::through([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]);
        assert_eq!(plane.map(|p| p.normal), Some([0.0, 0.0, 1.0]));
        assert!(Plane::through([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]).is_none());
    }

    #[test]
    fn an_unbounded_reference_plane_runs_on_and_a_bounded_one_does_not() {
        let mut r = RefPlane {
            name: "R".into(),
            start: [0.0, 0.0],
            end: [10.0, 0.0],
            strength: Strength::Weak,
            bounded: true,
        };
        assert_eq!(r.line(), [[0.0, 0.0], [10.0, 0.0]]);
        r.bounded = false;
        assert_eq!(r.line()[0], [-UNBOUNDED_REACH, 0.0]);
        assert_eq!(r.line()[1], [10.0 + UNBOUNDED_REACH, 0.0]);
    }
}
