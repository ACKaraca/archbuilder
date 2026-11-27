//! Selection helpers that need no window: select all, select all of a kind, keep one kind of the
//! selection, the walls joined to a wall, the previous selection, and the plan extent of a set of
//! objects (the clipboard's base point, zooming to a selection). Hidden objects ([`crate::visibility`]) are skipped.

use std::collections::BTreeSet;

use arc_base::ObjectId;
use arc_doc::Snapshot;
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, FLOOR, ROOM, WALL, WINDOW};
use archbuilder_model::wall::Wall;

use crate::visibility::{KINDS, Visibility};

/// The most objects a selection command returns.
pub const MAX_SELECTED: usize = 100_000;

/// How far apart two wall ends may be and still count as joined, in meters.
const JOINED: f64 = 1.0e-4;

/// Every shown object of `kinds`, in identifier order within each kind.
#[must_use]
pub fn all_of(snapshot: &Snapshot, kinds: &[&str], vis: &Visibility) -> Vec<ObjectId> {
    kinds
        .iter()
        .flat_map(|k| spatial::objects(snapshot, k))
        .map(|o| o.id)
        .filter(|id| vis.shows(snapshot, *id))
        .take(MAX_SELECTED)
        .collect()
}

/// Every shown wall, door, window, floor and room.
#[must_use]
pub fn all(snapshot: &Snapshot, vis: &Visibility) -> Vec<ObjectId> {
    all_of(snapshot, &KINDS, vis)
}

/// Every shown object of the kinds that `selection` holds.
#[must_use]
pub fn all_of_kinds_in(
    snapshot: &Snapshot,
    selection: &[ObjectId],
    vis: &Visibility,
) -> Vec<ObjectId> {
    all_of(
        snapshot,
        &crate::visibility::kinds_of(snapshot, selection),
        vis,
    )
}

/// Every shown wall, floor, door and window whose type is the type of one of the objects of
/// `selection`.
#[must_use]
pub fn all_of_types_in(
    snapshot: &Snapshot,
    selection: &[ObjectId],
    vis: &Visibility,
) -> Vec<ObjectId> {
    use archbuilder_model::element_type::{TypeKind, instances, type_of_object};
    let mut wanted: Vec<(TypeKind, ObjectId)> = selection
        .iter()
        .filter_map(|id| spatial::get(snapshot, *id))
        .filter_map(|o| type_of_object(snapshot, &o).map(|(k, t)| (k, t.id)))
        .collect();
    wanted.sort();
    wanted.dedup();
    wanted
        .into_iter()
        .flat_map(|(kind, id)| instances(snapshot, kind, id))
        .map(|o| o.id)
        .filter(|id| vis.shows(snapshot, *id))
        .take(MAX_SELECTED)
        .collect()
}

/// The objects of `selection` that are of `kind`.
#[must_use]
pub fn only(snapshot: &Snapshot, selection: &[ObjectId], kind: &str) -> Vec<ObjectId> {
    selection
        .iter()
        .copied()
        .filter(|id| spatial::type_of(snapshot, *id).as_deref() == Some(kind))
        .collect()
}

/// How many objects of each hideable kind `selection` holds, in the order of [`KINDS`].
#[must_use]
pub fn kind_counts(snapshot: &Snapshot, selection: &[ObjectId]) -> Vec<(&'static str, usize)> {
    KINDS
        .into_iter()
        .map(|k| (k, only(snapshot, selection, k).len()))
        .filter(|(_, n)| *n > 0)
        .collect()
}

/// The walls joined to `wall` end to end, itself included, in a storey: every wall of the same
/// storey with an end at an end of a wall already in the chain.
#[must_use]
pub fn wall_chain(snapshot: &Snapshot, wall: ObjectId, vis: &Visibility) -> Vec<ObjectId> {
    let Some(first) = spatial::get(snapshot, wall) else {
        return Vec::new();
    };
    let storey = first.header.owner;
    let walls: Vec<(ObjectId, Wall)> = spatial::objects(snapshot, WALL)
        .iter()
        .filter(|o| o.header.owner == storey && vis.shows(snapshot, o.id))
        .filter_map(|o| Wall::from_values(&o.values).map(|w| (o.id, w)))
        .collect();
    let near = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) <= JOINED;
    let mut chain: BTreeSet<ObjectId> = BTreeSet::from([wall]);
    let mut ends: Vec<[f64; 2]> = walls
        .iter()
        .find(|(id, _)| *id == wall)
        .map(|(_, w)| vec![w.start, w.end])
        .unwrap_or_default();
    // Each pass adds the walls that touch the chain; the chain grows at most to the walls.
    for _ in 0..walls.len() {
        let before = chain.len();
        for (id, w) in &walls {
            if !chain.contains(id) && ends.iter().any(|e| near(*e, w.start) || near(*e, w.end)) {
                chain.insert(*id);
                ends.extend([w.start, w.end]);
            }
        }
        if chain.len() == before {
            break;
        }
    }
    chain.into_iter().collect()
}

/// The lower left corner of the plan extent of `ids` ([`plan_points`]).
#[must_use]
pub fn base_point(snapshot: &Snapshot, ids: &[ObjectId]) -> Option<[f64; 2]> {
    ids.iter()
        .take(MAX_SELECTED)
        .flat_map(|id| plan_points(snapshot, *id))
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .reduce(|l, p| [l[0].min(p[0]), l[1].min(p[1])])
}

/// The points in plan that bound the element `id`, for every kind of element: walls by their
/// footprint, floors, ceilings, shafts and vertical openings by their outline, rooms by their
/// point, stairs, ramps and roofs by their outline, railings by their plan line, floor edges by
/// their chains, grids and reference planes by their ends, doors, windows and wall openings by
/// their place in their wall. Empty for anything else or an element that has no place.
#[must_use]
pub fn plan_points(snapshot: &Snapshot, id: ObjectId) -> Vec<[f64; 2]> {
    use archbuilder_model::types;
    let Some(object) = spatial::get(snapshot, id) else {
        return Vec::new();
    };
    match spatial::type_of(snapshot, id).as_deref() {
        Some(WALL) => Wall::from_values(&object.values)
            .map(|w| w.plain_footprint().to_vec())
            .unwrap_or_default(),
        Some(FLOOR) => archbuilder_model::floor::Floor::from_values(&object.values)
            .and_then(|f| f.boundary_in(snapshot))
            .map(|b| b.loops.into_iter().flatten().collect())
            .unwrap_or_default(),
        Some(ROOM) => archbuilder_model::room::Room::from_values(&object.values)
            .map(|r| vec![r.point])
            .unwrap_or_default(),
        Some(types::STAIR) => archbuilder_model::stair::Placed::of(snapshot, &object)
            .map(|p| p.outline())
            .unwrap_or_default(),
        Some(types::RAMP) => archbuilder_model::ramp::Placed::of(snapshot, &object)
            .map(|p| p.outline())
            .unwrap_or_default(),
        Some(types::RAILING) => archbuilder_model::railing::Derived::of(snapshot, &object)
            .map(|d| d.plan_line())
            .unwrap_or_default(),
        Some(types::ROOF) => archbuilder_model::roof::Derived::of(snapshot, &object)
            .map(|d| d.surface.outline.clone())
            .unwrap_or_default(),
        Some(types::ROOF_EXTRUSION) => {
            archbuilder_model::roof_extrusion::Derived::of(snapshot, &object)
                .map(|d| {
                    d.pieces()
                        .iter()
                        .flat_map(|p| p.points.iter().copied())
                        .collect()
                })
                .unwrap_or_default()
        }
        Some(types::CEILING) => archbuilder_model::ceiling::Ceiling::from_values(&object.values)
            .and_then(|c| c.shape.boundary_in(snapshot))
            .map(|b| b.loops.into_iter().flatten().collect())
            .unwrap_or_default(),
        Some(types::SHAFT) => archbuilder_model::opening::shaft_span(snapshot, &object)
            .map(|s| s.outline)
            .unwrap_or_default(),
        Some(types::VERTICAL_OPENING) => archbuilder_model::opening::outline_of(
            snapshot,
            &object,
            archbuilder_model::floor::VERTICAL_SLOTS,
        )
        .unwrap_or_default(),
        Some(types::FLOOR_EDGE) => archbuilder_model::floor_edge::Derived::of(snapshot, &object)
            .map(|d| {
                d.chains
                    .iter()
                    .flat_map(|c| c.points.iter().map(|p| [p[0], p[1]]))
                    .collect()
            })
            .unwrap_or_default(),
        Some(types::GRID) => archbuilder_model::datum::Grid::from_values(&object.values)
            .map(|g| vec![g.start, g.end])
            .unwrap_or_default(),
        Some(types::REFERENCE_PLANE) => {
            archbuilder_model::datum::RefPlane::from_values(&object.values)
                .map(|r| vec![r.start, r.end])
                .unwrap_or_default()
        }
        Some(DOOR | WINDOW | types::WALL_OPENING) => {
            let host = object
                .header
                .owner
                .and_then(|w| spatial::get(snapshot, w))
                .and_then(|w| Wall::from_values(&w.values));
            let item = archbuilder_model::insert::Insert::of(snapshot, &object);
            let (Some(w), Some(item)) = (host, item) else {
                return Vec::new();
            };
            let length = w.length().max(1e-9);
            let along = |d: f64| {
                [
                    w.start[0] + (w.end[0] - w.start[0]) * d / length,
                    w.start[1] + (w.end[1] - w.start[1]) * d / length,
                ]
            };
            let (low, high) = item.extent();
            vec![along(low), along(high)]
        }
        _ => Vec::new(),
    }
}

/// What the previous selection was: the selection the tab had before its last change, so that
/// it can be restored.
#[derive(Debug, Default)]
pub struct Memory {
    current: Vec<ObjectId>,
    previous: Vec<ObjectId>,
}

impl Memory {
    /// Takes the tab's selection of this frame. An empty selection never becomes the previous one.
    pub fn observe(&mut self, now: &[ObjectId]) {
        if self.current != now {
            if !self.current.is_empty() {
                self.previous = std::mem::take(&mut self.current);
            }
            self.current = now.to_vec();
        }
    }

    /// The previous selection, without the objects that no longer exist.
    #[must_use]
    pub fn previous(&self, snapshot: &Snapshot) -> Vec<ObjectId> {
        self.previous
            .iter()
            .copied()
            .filter(|id| spatial::get(snapshot, *id).is_some())
            .collect()
    }
}
