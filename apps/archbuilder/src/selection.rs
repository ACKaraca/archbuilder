//! Selection helpers that need no window: select all, select all of a kind, keep one kind of the
//! selection, the walls joined to a wall, the previous selection, and the plan extent of a set of
//! objects (the clipboard's base point). Hidden objects ([`crate::visibility`]) are skipped.

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

/// The lower left corner of the plan extent of `ids`: walls by their footprints, floors by their
/// boundaries, rooms and doors and windows by their points.
#[must_use]
pub fn base_point(snapshot: &Snapshot, ids: &[ObjectId]) -> Option<[f64; 2]> {
    let mut low: Option<[f64; 2]> = None;
    let mut add = |p: [f64; 2]| {
        if p[0].is_finite() && p[1].is_finite() {
            let l = low.get_or_insert(p);
            l[0] = l[0].min(p[0]);
            l[1] = l[1].min(p[1]);
        }
    };
    for id in ids.iter().take(MAX_SELECTED) {
        let Some(object) = spatial::get(snapshot, *id) else {
            continue;
        };
        match spatial::type_of(snapshot, *id).as_deref() {
            Some(WALL) => {
                if let Some(w) = Wall::from_values(&object.values) {
                    w.plain_footprint().into_iter().for_each(&mut add);
                }
            }
            Some(FLOOR) => {
                if let Some(d) = archbuilder_model::floor::Derived::of(snapshot, &object) {
                    d.boundary
                        .loops
                        .iter()
                        .flatten()
                        .copied()
                        .for_each(&mut add);
                }
            }
            Some(ROOM) => {
                if let Some(r) = archbuilder_model::room::Room::from_values(&object.values) {
                    add(r.point);
                }
            }
            Some(archbuilder_model::types::STAIR) => {
                if let Some(p) = archbuilder_model::stair::Placed::of(snapshot, &object) {
                    p.outline().into_iter().for_each(&mut add);
                }
            }
            Some(archbuilder_model::types::ROOF) => {
                if let Ok(d) = archbuilder_model::roof::Derived::of(snapshot, &object) {
                    d.surface.outline.iter().copied().for_each(&mut add);
                }
            }
            Some(DOOR | WINDOW) => {
                if let Some(host) = object
                    .header
                    .owner
                    .and_then(|h| spatial::get(snapshot, h))
                    .and_then(|h| Wall::from_values(&h.values))
                {
                    add(host.start);
                    add(host.end);
                }
            }
            _ => {}
        }
    }
    low
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
