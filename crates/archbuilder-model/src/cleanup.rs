//! The cleaned bodies of walls and floors of a document: the layer pieces of a wall solid and the
//! floor slab cut back to the cores it meets, both derived from the layer polygons of
//! [`crate::layered`] each time and never stored.
//!
//! # Floor and wall joins
//!
//! Where a floor and a wall overlap in plan and in height, the wall's core runs through: the
//! floor loses the core's footprint, and the footprint of the wall's layers on the side the floor
//! does not lie on (the floor is cut back to the core face it meets). The layers on the floor's
//! side that the slab still covers stop at the slab: they end at its underside when the wall
//! reaches up through it, and start at its top when the wall stands on it. Floor areas, room areas
//! and quantities use the authored boundary and are not changed by this. A wall whose layers do
//! not meet a floor in height or plan is untouched.

use std::sync::{Arc, Mutex};

use arc_base::{Limits, ObjectId, Tolerance};
use arc_doc::{Object, Snapshot};
use arc_geom::{BooleanOp, CurveFlatten, Polyhedron, Region, prism};

use crate::floor::{Boundary, Derived, FLAT_CHORD};
use crate::insert::Insert;
use crate::layered::{self, Layout, Poly, Spec, area, finish_wall};
use crate::types::{FLOOR, WALL};
use crate::wall::{Band, BandFace, Profile, Wall};
use crate::wall_shape;
use crate::{insert, spatial};

type P = [f64; 2];

/// The most walls of a document looked at for the cleaning of one floor.
const MAX_WALLS: usize = 20_000;
const CHORD: f64 = 0.01;

/// The layers of the walls of one storey at one snapshot.
#[derive(Debug)]
pub struct StoreyLayers {
    /// The walls, in identifier order; an arc wall appears once for each of its facets.
    pub ids: Vec<ObjectId>,
    /// Their data (the straight facet of an arc wall).
    pub walls: Vec<Wall>,
    /// Where each entry lies along its record's baseline: the distance of its start and its
    /// length, both along the arc for an arc wall.
    pub spans: Vec<(f64, f64)>,
    /// The whole wall each entry belongs to (the entry itself for a straight wall).
    pub wholes: Vec<Wall>,
    /// Their layers (the first segment's for a stacked wall).
    pub specs: Vec<Spec>,
    /// The cleaned layer polygons.
    pub layout: Layout,
    /// For each further segment of the tallest stack (the first is `layout`), the layout made
    /// with each stacked wall's layers of that segment: a stack shows each segment's layers.
    pub stacked: Vec<(Vec<Spec>, Layout)>,
    /// The layers of every segment of each entry's stack, from the base; empty for a wall that is
    /// not stacked.
    pub stack_specs: Vec<Vec<Spec>>,
}

/// The objects a derived result reads. A result is kept while every one of them is the very
/// object (the same `Arc`) it was made from, so a commit that touches none of them (a room
/// renamed, a wall of another storey moved) finds it again, whatever snapshot asks.
pub(crate) type Deps = Vec<Arc<Object>>;

pub(crate) struct Entry<T> {
    id: ObjectId,
    extra: u64,
    deps: Deps,
    value: Arc<T>,
}

pub(crate) type Store<T> = Mutex<Vec<Entry<T>>>;
const CACHE_ENTRIES: usize = 256;

fn same(a: &[Arc<Object>], b: &[Arc<Object>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| Arc::ptr_eq(x, y))
}

/// The most results kept under one (`id`, `extra`): one for each document that has an object with
/// that identifier (two documents opened from one file, or documents made from the same seed).
const SAME_KEY: usize = 8;

/// Whether `old` shares an object with `new`: results of one document follow each other this way
/// (a commit replaces some of the objects), results of two documents never do.
fn shares(old: &[Arc<Object>], new: &std::collections::HashSet<*const Object>) -> bool {
    old.iter().any(|o| new.contains(&Arc::as_ptr(o)))
}

/// The result kept in `store` for (`id`, `extra`) when it was made from `deps`, else `make()`,
/// which is then kept. The entry holds its objects, so an address is never reused under it. A new
/// result replaces the older ones of the same document only, so documents whose objects have the
/// same identifiers do not throw each other's results away; the least recently used entry goes
/// first when the store is full.
pub(crate) fn cached<T>(
    store: &Store<T>,
    id: ObjectId,
    extra: u64,
    deps: Deps,
    make: impl FnOnce() -> T,
) -> Arc<T> {
    let hit = store.lock().ok().and_then(|mut c| {
        let at = c
            .iter()
            .position(|e| e.id == id && e.extra == extra && same(&e.deps, &deps))?;
        let entry = c.remove(at);
        let value = Arc::clone(&entry.value);
        c.push(entry);
        Some(value)
    });
    hit.unwrap_or_else(|| {
        let value = Arc::new(make());
        if let Ok(mut c) = store.lock() {
            let new: std::collections::HashSet<*const Object> =
                deps.iter().map(Arc::as_ptr).collect();
            c.retain(|e| !(e.id == id && e.extra == extra && shares(&e.deps, &new)));
            if let Some(oldest) = c
                .iter()
                .position(|e| e.id == id && e.extra == extra)
                .filter(|_| c.iter().filter(|e| e.id == id && e.extra == extra).count() >= SAME_KEY)
            {
                c.remove(oldest);
            }
            if c.len() >= CACHE_ENTRIES {
                c.remove(0);
            }
            c.push(Entry {
                id,
                extra,
                deps,
                value: Arc::clone(&value),
            });
        }
        value
    })
}

/// The result kept in `store` for (`id`, `extra`) when it was made from `deps`; nothing is made.
pub(crate) fn lookup<T>(store: &Store<T>, id: ObjectId, extra: u64, deps: &Deps) -> Option<Arc<T>> {
    store.lock().ok().and_then(|c| {
        c.iter()
            .find(|e| e.id == id && e.extra == extra && same(&e.deps, deps))
            .map(|e| Arc::clone(&e.value))
    })
}

/// Drops the results kept for `id` under another key than `extra`.
pub(crate) fn keep_only<T>(store: &Store<T>, id: ObjectId, extra: u64) {
    if let Ok(mut c) = store.lock() {
        c.retain(|e| e.id != id || e.extra == extra);
    }
}

/// Drops the result kept for (`id`, `extra`).
pub(crate) fn forget<T>(store: &Store<T>, id: ObjectId, extra: u64) {
    if let Ok(mut c) = store.lock() {
        c.retain(|e| !(e.id == id && e.extra == extra));
    }
}

/// The walls of `storey` with their types, and with their doors and windows when `inserts`: what
/// the layers of the storey are made from.
pub(crate) fn storey_deps(snapshot: &Snapshot, storey: ObjectId, inserts: bool) -> Deps {
    use crate::element_type::{TypeKind, type_of};
    let mut deps = Vec::new();
    for w in spatial::walls(snapshot, storey) {
        deps.extend(type_of(snapshot, TypeKind::Wall, &w.values));
        if inserts {
            for key in [crate::types::DOOR, crate::types::WINDOW] {
                deps.extend(spatial::children(snapshot, key, w.id));
            }
        }
        deps.push(w);
    }
    deps
}

static LAYERS: Store<StoreyLayers> = Mutex::new(Vec::new());

/// The layers of the walls of `storey`, computed again only when a wall of the storey or one of
/// their types changed.
#[must_use]
pub fn storey_layers(snapshot: &Snapshot, storey: ObjectId) -> Arc<StoreyLayers> {
    cached(
        &LAYERS,
        storey,
        0,
        storey_deps(snapshot, storey, false),
        || {
            let mut ids = Vec::new();
            let mut walls = Vec::new();
            let mut specs = Vec::new();
            let mut spans = Vec::new();
            let mut wholes = Vec::new();
            for o in spatial::walls(snapshot, storey) {
                if let Some(w) = Wall::from_values(&o.values) {
                    let spec = Spec::of(snapshot, &o, &w);
                    let marks = w.facet_marks();
                    for (k, facet) in w.facets().into_iter().enumerate() {
                        ids.push(o.id);
                        specs.push(spec.clone());
                        spans.push((marks[k], marks[k + 1] - marks[k]));
                        wholes.push(w);
                        walls.push(facet);
                    }
                }
            }
            // A stacked wall's layers come from its segments.
            let mut stack_specs: Vec<Vec<Spec>> = vec![Vec::new(); walls.len()];
            let mut at = 0;
            for o in spatial::walls(snapshot, storey) {
                if let Some(w) = Wall::from_values(&o.values) {
                    let n = w.facets().len();
                    if let Some(found) = stack_specs_of(snapshot, &o, &w) {
                        for k in at..at + n {
                            if let (Some(first), Some(slot), Some(spec)) =
                                (found.first(), specs.get_mut(k), stack_specs.get_mut(k))
                            {
                                slot.clone_from(first);
                                spec.clone_from(&found);
                            }
                        }
                    }
                    at += n;
                }
            }
            let layout = layered::layout(&walls, &specs);
            let most = stack_specs.iter().map(Vec::len).max().unwrap_or(0);
            let stacked = (1..most)
                .map(|k| {
                    let pass: Vec<Spec> = specs
                        .iter()
                        .zip(&stack_specs)
                        .map(|(own, segments)| {
                            segments
                                .get(k)
                                .or_else(|| segments.last())
                                .unwrap_or(own)
                                .clone()
                        })
                        .collect();
                    let layout = layered::layout(&walls, &pass);
                    (pass, layout)
                })
                .collect();
            StoreyLayers {
                ids,
                walls,
                spans,
                wholes,
                specs,
                layout,
                stacked,
                stack_specs,
            }
        },
    )
}

/// The layers of each segment of the stacked wall `object`, from the base; `None` for a wall that
/// is not stacked (or whose stack cannot be read).
fn stack_specs_of(snapshot: &Snapshot, object: &Object, wall: &Wall) -> Option<Vec<Spec>> {
    use crate::element_type::{Layers, StackEntry, TypeKind, WallType, type_of};
    let ty = type_of(snapshot, TypeKind::Wall, &object.values)?;
    let stack = StackEntry::of(&ty.values);
    crate::element_type::stack_ranges(&stack, wall.height)?;
    stack
        .iter()
        .map(|e| {
            let t = spatial::get(snapshot, e.type_id)?;
            let wt = WallType::from_values(&t.values)?;
            Some(Spec::of_type(
                &Layers::from_values(&t.values)?,
                wt.wrap_ends,
                wt.wrap_inserts,
                wall.thickness,
            ))
        })
        .collect()
}

/// The height ranges of the segments of the stacked wall `object`, above its base; empty for a
/// wall that is not stacked.
#[must_use]
pub fn stack_ranges_of(snapshot: &Snapshot, object: &Object) -> Vec<(f64, f64)> {
    use crate::element_type::{StackEntry, TypeKind, type_of};
    let (Some(wall), Some(ty)) = (
        Wall::of(snapshot, object),
        type_of(snapshot, TypeKind::Wall, &object.values),
    ) else {
        return Vec::new();
    };
    crate::element_type::stack_ranges(&StackEntry::of(&ty.values), wall.height).unwrap_or_default()
}

impl StoreyLayers {
    /// The indexes of the entries of the wall `id`: its facets, in order from its start.
    fn indexes(&self, id: ObjectId) -> Vec<usize> {
        self.ids
            .iter()
            .enumerate()
            .filter(|(_, i)| **i == id)
            .map(|(k, _)| k)
            .collect()
    }

    /// The polygons of the whole wall `id` with openings active at the height `z`. The facets of
    /// an arc wall are united layer by layer where they touch, and each opening is cut (and its
    /// reveal wrapped) on its own tangent frame, as the solid of the wall and the IFC opening
    /// have it.
    fn finished_wall(&self, snapshot: &Snapshot, id: ObjectId, z: f64) -> Vec<Poly> {
        let pass = if z.is_finite() && z < f64::MAX / 2.0 {
            spatial::get(snapshot, id)
                .map(|o| stack_ranges_of(snapshot, &o))
                .and_then(|r| r.iter().position(|(a, b)| z >= *a && z < *b))
                .unwrap_or(0)
        } else {
            0
        };
        self.finished_wall_in(snapshot, id, z, pass)
    }

    /// [`Self::finished_wall`] with the layers of the stack segment `pass`.
    fn finished_wall_in(
        &self,
        snapshot: &Snapshot,
        id: ObjectId,
        z: f64,
        pass: usize,
    ) -> Vec<Poly> {
        let at = self.indexes(id);
        if at.len() < 2 {
            return at
                .iter()
                .flat_map(|k| self.finished(snapshot, *k, z, true, pass))
                .collect();
        }
        let all: Vec<Poly> = at
            .iter()
            .flat_map(|k| self.finished(snapshot, *k, z, false, pass))
            .collect();
        let mut out = layered::merge_touching(all, &|w| self.ids.get(w).copied());
        let specs = match pass.checked_sub(1).and_then(|k| self.stacked.get(k)) {
            Some((specs, _)) => specs,
            None => &self.specs,
        };
        let (Some(whole), Some(spec)) = (
            at.first().and_then(|k| self.wholes.get(*k)),
            at.first().and_then(|k| specs.get(*k)),
        ) else {
            return out;
        };
        for item in insert::openings_of(snapshot, id)
            .into_iter()
            .filter(|i| i.sill <= z && z < i.top())
        {
            let (tangent, local) = insert::on_arc(whole, &item);
            out = finish_wall(&tangent, spec, &out, [false; 2], &[local.extent()]);
        }
        out
    }

    /// The polygons of the wall at `at` with openings active at the height `z` above its base.
    /// Kept while the polygons the layout gave the wall, its openings and its type are the same,
    /// so a move elsewhere in the storey does not make them again.
    fn finished(
        &self,
        snapshot: &Snapshot,
        at: usize,
        z: f64,
        with_gaps: bool,
        pass: usize,
    ) -> Vec<Poly> {
        // The layers and the layout of the segment of a stack (the first when it has none).
        let (specs, layout) = match pass.checked_sub(1).and_then(|k| self.stacked.get(k)) {
            Some((specs, layout)) => (specs, layout),
            None => (&self.specs, &self.layout),
        };
        let (Some(wall), Some(spec), Some(id)) =
            (self.walls.get(at), specs.get(at), self.ids.get(at))
        else {
            return Vec::new();
        };
        let own: Vec<Poly> = layout
            .polys
            .iter()
            .filter(|p| p.wall == at)
            .cloned()
            .collect();
        let (from, length) = self.spans.get(at).copied().unwrap_or((0.0, wall.length()));
        // The openings along the record's baseline (the arc), moved into this facet's own
        // distances along its chord.
        let scale = if length > 0.0 {
            wall.length() / length
        } else {
            1.0
        };
        let gaps: Vec<(f64, f64)> = insert::openings_of(snapshot, *id)
            .into_iter()
            .filter(|_| with_gaps)
            .filter(|i| i.sill <= z && z < i.top())
            .map(|i| i.extent())
            .filter(|(a, b)| *b > from && *a < from + length)
            .map(|(a, b)| ((a - from) * scale, (b - from) * scale))
            .collect();
        let free = layout.free.get(at).copied().unwrap_or([true; 2]);
        let key = FinishedKey {
            own,
            free,
            gaps,
            wall: *wall,
            spec: spec.clone(),
        };
        let slot = (*id, z.to_bits());
        let hit = FINISHED.lock().ok().and_then(|c| {
            c.iter()
                .find(|(i, k, input, _)| (*i, *k) == slot && *input == key)
                .map(|(_, _, _, v)| Arc::clone(v))
        });
        let out = hit.unwrap_or_else(|| {
            let v = Arc::new(finish_wall(wall, spec, &key.own, free, &key.gaps));
            if let Ok(mut c) = FINISHED.lock() {
                c.retain(|(i, k, _, _)| (*i, *k) != slot);
                if c.len() >= 4 * CACHE_ENTRIES {
                    c.remove(0);
                }
                c.push((slot.0, slot.1, key, Arc::clone(&v)));
            }
            v
        });
        out.as_ref().clone()
    }
}

/// What the finished polygons of a wall are made from.
#[derive(PartialEq)]
struct FinishedKey {
    own: Vec<Poly>,
    free: [bool; 2],
    gaps: Vec<(f64, f64)>,
    wall: Wall,
    spec: Spec,
}

type FinishedEntry = (ObjectId, u64, FinishedKey, Arc<Vec<Poly>>);
static FINISHED: Mutex<Vec<FinishedEntry>> = Mutex::new(Vec::new());

/// One line between layers: the wall, its ends and whether it bounds a core.
pub type PlanLine = (ObjectId, [P; 2], bool);

static LINES: Store<Vec<PlanLine>> = Mutex::new(Vec::new());

/// The lines between the layers of the walls of `storey` at the height `cut` above their bases:
/// the wall, the line and whether it bounds a core. Joins are cleaned and wrapping and openings
/// are shown. Computed again only when a wall of the storey, its type or one of its doors and
/// windows changed.
#[must_use]
pub fn plan_lines(snapshot: &Snapshot, storey: ObjectId, cut: f64) -> Arc<Vec<PlanLine>> {
    cached(
        &LINES,
        storey,
        cut.to_bits(),
        storey_deps(snapshot, storey, true),
        || {
            let layers = storey_layers(snapshot, storey);
            let mut all: Vec<Poly> = Vec::new();
            for (at, id) in layers.ids.iter().enumerate() {
                // An arc wall is finished once, as a whole, at its first facet.
                if at == 0 || layers.ids.get(at - 1) != Some(id) {
                    all.extend(layers.finished_wall(snapshot, *id, cut));
                }
            }
            layered::lines(&all)
                .into_iter()
                .filter_map(|(w, seg, core)| layers.ids.get(w).map(|id| (*id, seg, core)))
                .collect()
        },
    )
}

/// The outline of the wall record `object` in plan: the outer loops of the union of its cleaned
/// layers (joins cleaned, wrapped ends shown, openings left in). Empty when the wall has none.
#[must_use]
pub fn wall_outline(snapshot: &Snapshot, object: &Object) -> Vec<Vec<P>> {
    let Some(storey) = object.header.owner else {
        return Vec::new();
    };
    let layers = storey_layers(snapshot, storey);
    if layers.indexes(object.id).is_empty() {
        return Vec::new();
    }
    layered::outline(&layers.finished_wall(snapshot, object.id, f64::MAX))
}

/// The elevation of the base of the wall record `object` and the height of its top above it.
fn wall_range(snapshot: &Snapshot, object: &Object) -> Option<(f64, f64)> {
    let wall = Wall::of(snapshot, object)?;
    let level = object
        .header
        .owner
        .and_then(|s| spatial::get(snapshot, s))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    Some((level + wall.base_offset, wall.height))
}

/// A floor with its slab cut back to the cores it meets.
#[derive(Debug)]
pub struct CleanFloor {
    /// The floor.
    pub id: ObjectId,
    /// The cut slab in plan, with exact arcs.
    pub region: Region,
    /// The same, faceted, as polygons (the outer loops of its parts).
    pub flat: Vec<Vec<P>>,
    /// The same, faceted, as a region with its holes.
    pub faceted: Region,
    /// The storey of the floor.
    pub storey: Option<ObjectId>,
    /// The bounding box of the slab in plan: `[min x, min y, max x, max y]`.
    pub bbox: [f64; 4],
    /// The underside and the top of the slab; for a sloped slab the lowest and the highest.
    pub z: (f64, f64),
    /// The plane of the top when the slab slopes.
    pub slope: Option<crate::terrain::Plane>,
    /// The thickness of the slab.
    pub thickness: f64,
}

/// The points of a loop, arcs flattened within [`FLAT_CHORD`].
fn flatten_loop(l: &arc_geom::Loop2) -> Option<Vec<P>> {
    let mut points: Vec<P> = Vec::new();
    for c in l.curves() {
        let flat = c.flatten(FLAT_CHORD, 0.2, 4096).ok()?;
        // Each curve gives its end points; the next curve repeats the joint.
        points.extend(
            flat.iter()
                .take(flat.len().saturating_sub(1))
                .map(|p| [p.x, p.y]),
        );
    }
    (points.len() >= 3).then_some(points)
}

/// The parts of `region` as polygons: the outer loop and the holes of each, arcs flattened.
#[must_use]
pub fn flatten_parts(region: &Region) -> Vec<(Vec<P>, Vec<Vec<P>>)> {
    region
        .parts()
        .iter()
        .filter_map(|part| {
            Some((
                flatten_loop(part.outer())?,
                part.holes().iter().filter_map(flatten_loop).collect(),
            ))
        })
        .collect()
}

/// The outer polygons of the parts of `region`.
fn flatten_region(region: &Region) -> Vec<Vec<P>> {
    flatten_parts(region).into_iter().map(|p| p.0).collect()
}

/// The loop of a polygon.
fn loop_of(points: &[P]) -> Option<arc_geom::Loop2> {
    let part = poly_region(points)?.parts().first()?.clone();
    Some(part.outer().clone())
}

/// A region of the faceted parts (outer loop and holes) of a floor.
fn faceted_region(parts: &[(Vec<P>, Vec<Vec<P>>)]) -> Option<Region> {
    let mut out = Vec::with_capacity(parts.len());
    for (outer, holes) in parts {
        out.push(arc_geom::RegionPart::new(
            loop_of(outer)?,
            holes.iter().filter_map(|h| loop_of(h)).collect(),
        ));
    }
    Region::new(out).ok()
}

fn poly_region(points: &[P]) -> Option<Region> {
    Boundary::polygons(vec![points.to_vec()]).region().ok()
}

static SLABS: Store<Option<Arc<CleanFloor>>> = Mutex::new(Vec::new());

/// The floor `object` cut back to the cores of the walls it meets; `None` when the floor cannot
/// be read. Computed again only when the floor, the walls of its storey (with their types and
/// openings), the stairs, the levels, the roofs or the other floors changed.
#[must_use]
pub fn clean_floor(snapshot: &Snapshot, object: &Object) -> Option<Arc<CleanFloor>> {
    let mut deps: Deps = Vec::new();
    if let Some(storey) = object.header.owner {
        deps.extend(spatial::get(snapshot, storey));
        deps.extend(storey_deps(snapshot, storey, true));
    }
    for key in [
        crate::types::STAIR,
        crate::types::RAMP,
        crate::types::LEVEL,
        crate::types::STOREY,
        FLOOR,
        crate::types::ROOF,
        crate::types::SHAFT,
        crate::types::ELEVATOR,
    ] {
        deps.extend(spatial::objects(snapshot, key));
    }
    deps.extend(spatial::children(
        snapshot,
        crate::types::VERTICAL_OPENING,
        object.id,
    ));
    let stored = cached(&SLABS, object.id, 0, deps, || {
        compute_clean_floor(snapshot, object).map(Arc::new)
    });
    stored.as_ref().clone()
}

/// [`clean_floor`] made fresh.
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the walls that meet the slab"
)]
fn compute_clean_floor(snapshot: &Snapshot, object: &Object) -> Option<CleanFloor> {
    let derived = Derived::of(snapshot, object)?;
    let mut region = derived.boundary.region().ok()?;
    let z = derived.z_range();
    let outer = derived.boundary.faceted(FLAT_CHORD);
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in outer.loops.iter().flatten() {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    // Only the walls of the floor's own storey cut it back: a wall of the storey below stops
    // under the slab (see [`wall_pieces`]) and never splits it.
    let own_storey = object.header.owner;
    for wall_object in spatial::objects(snapshot, WALL).iter().take(MAX_WALLS) {
        let Some(storey) = wall_object.header.owner.filter(|s| Some(*s) == own_storey) else {
            continue;
        };
        // The cheap tests first: the bounding box of the wall against that of the slab.
        let Some(raw) = Wall::from_values(&wall_object.values) else {
            continue;
        };
        let pad = raw.thickness;
        let (wl, wh) = (
            [
                raw.start[0].min(raw.end[0]) - pad,
                raw.start[1].min(raw.end[1]) - pad,
            ],
            [
                raw.start[0].max(raw.end[0]) + pad,
                raw.start[1].max(raw.end[1]) + pad,
            ],
        );
        if wh[0] < lo[0] || wl[0] > hi[0] || wh[1] < lo[1] || wl[1] > hi[1] {
            continue;
        }
        let Some((base, height)) = wall_range(snapshot, wall_object) else {
            continue;
        };
        let (from, to) = (base.max(z.0), (base + height).min(z.1));
        if to - from <= 1.0e-6 {
            continue;
        }
        let layers = storey_layers(snapshot, storey);
        let facets = layers.indexes(wall_object.id);
        // The side is probed at the middle of the wall: its middle facet.
        let Some(at) = facets.get(facets.len() / 2).copied() else {
            continue;
        };
        let Some(wall) = layers.walls.get(at) else {
            continue;
        };
        let polys = layers.finished_wall(snapshot, wall_object.id, (from + to) / 2.0 - base);
        let overlap: f64 = polys
            .iter()
            .filter_map(|p| {
                let r = poly_region(&p.points)?;
                let i = arc_geom::boolean(
                    &region,
                    &r,
                    BooleanOp::Intersection,
                    &Tolerance::DEFAULT,
                    &Limits::DEFAULT,
                )
                .ok()??;
                Some(flatten_region(&i).iter().map(|l| area(l)).sum::<f64>())
            })
            .sum();
        if overlap <= 1.0e-8 {
            continue;
        }
        // The side the floor lies on: probe across the wall at its middle.
        let (low, high) = wall.extent();
        let length = wall.length().max(f64::MIN_POSITIVE);
        let u = [
            (wall.end[0] - wall.start[0]) / length,
            (wall.end[1] - wall.start[1]) / length,
        ];
        let mid = [
            (wall.start[0] + wall.end[0]) / 2.0,
            (wall.start[1] + wall.end[1]) / 2.0,
        ];
        let at_v = |v: f64| [mid[0] - u[1] * v, mid[1] + u[0] * v];
        let ext_in = derived.boundary.contains(at_v(high + 0.1));
        let int_in = derived.boundary.contains(at_v(low - 0.1));
        let Some(spec) = layers.specs.get(at) else {
            continue;
        };
        let first_core = spec.layers.iter().position(|l| l.core).unwrap_or(0);
        let last_core = spec.layers.iter().rposition(|l| l.core).unwrap_or(0);
        for p in &polys {
            let far = (ext_in != int_in)
                && ((int_in && p.layer < first_core) || (ext_in && p.layer > last_core));
            if !(p.core || far) {
                continue;
            }
            let Some(cut) = poly_region(&p.points) else {
                continue;
            };
            match arc_geom::boolean(
                &region,
                &cut,
                BooleanOp::Difference,
                &Tolerance::DEFAULT,
                &Limits::DEFAULT,
            ) {
                Ok(Some(rest)) => region = rest,
                Ok(None) => return None,
                Err(_) => {}
            }
        }
    }
    let parts = flatten_parts(&region);
    let flat: Vec<Vec<P>> = parts.iter().map(|p| p.0.clone()).collect();
    let faceted = faceted_region(&parts)?;
    let bbox = flat
        .iter()
        .flatten()
        .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        });
    Some(CleanFloor {
        id: object.id,
        storey: own_storey,
        bbox,
        region,
        flat,
        faceted,
        z,
        slope: derived.sloped().then_some(derived.plane),
        thickness: derived.floor.thickness,
    })
}

/// The solid of the floor `object`: its slab cut back to the cores it meets; `None` when the
/// floor cannot be read.
#[must_use]
pub fn floor_solid(snapshot: &Snapshot, object: &Object) -> Option<arc_base::Result<Polyhedron>> {
    let clean = clean_floor(snapshot, object)?;
    if let Some(plane) = clean.slope {
        let flat = prism(
            &clean.region,
            -clean.thickness,
            clean.thickness,
            CHORD,
            &Tolerance::DEFAULT,
            &Limits::DEFAULT,
        );
        return Some(flat.and_then(|f| crate::floor::shear(&f, plane)));
    }
    Some(prism(
        &clean.region,
        clean.z.0,
        clean.z.1 - clean.z.0,
        CHORD,
        &Tolerance::DEFAULT,
        &Limits::DEFAULT,
    ))
}

/// The cleaned floors of the document, each kept while what it is made from is unchanged.
fn floors(snapshot: &Snapshot) -> Vec<Arc<CleanFloor>> {
    spatial::objects(snapshot, FLOOR)
        .iter()
        .take(MAX_WALLS)
        .filter_map(|f| clean_floor(snapshot, f))
        .collect()
}

/// What a piece of a wall is made of, for the color it is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceClass {
    /// A layer of the core.
    Core,
    /// A finish layer outside the core.
    Finish,
    /// Any other layer outside the core.
    Other,
}

/// One solid of a wall: a layer (or part of one) over a range of heights.
#[derive(Debug, Clone)]
pub struct Piece {
    /// What the piece is made of.
    pub class: PieceClass,
    /// The solid.
    pub solid: Polyhedron,
    /// The layer, by index in the layers of its segment's type.
    pub layer: usize,
    /// The segment of a stacked wall the piece belongs to, from the base; 0 for a wall that is
    /// not stacked.
    pub segment: usize,
    /// A key of the piece that stays the same while its wall's openings and layers stay: the
    /// band of heights, the layer and the part of the layer packed in one number.
    pub key: u32,
}

/// The material of each layer of the wall record `object`, by layer index; empty for a wall
/// whose type does not give its layers (a plain wall).
#[must_use]
pub fn layer_materials(snapshot: &Snapshot, object: &Object) -> Vec<String> {
    layer_materials_in(snapshot, object, 0)
}

/// [`layer_materials`] for the segment `segment` of a stacked wall (the wall's own type when it is
/// not stacked).
#[must_use]
pub fn layer_materials_in(snapshot: &Snapshot, object: &Object, segment: usize) -> Vec<String> {
    use crate::element_type::{Layers, StackEntry, TypeKind, type_of};
    let wall = Wall::of(snapshot, object);
    let ty = type_of(snapshot, TypeKind::Wall, &object.values);
    let of_segment = ty
        .as_ref()
        .and_then(|t| StackEntry::of(&t.values).get(segment).copied())
        .and_then(|e| spatial::get(snapshot, e.type_id));
    of_segment
        .or(ty)
        .and_then(|t| Layers::from_values(&t.values))
        .filter(|l| {
            wall.is_some_and(|w| (l.total() - w.thickness).abs() <= 1.0e-6) && !l.layers.is_empty()
        })
        .map(|l| l.layers.into_iter().map(|l| l.material).collect())
        .unwrap_or_default()
}

/// The solids of the wall record `object`, layer by layer: the cleaned layer polygons of its
/// storey, cut at openings and wrapped, raised in bands where the openings change, clipped under
/// the roof it is attached to, and cut by the slabs it meets (see the module notes). `None` when
/// the record is not a readable wall.
#[must_use]
pub fn wall_pieces(snapshot: &Snapshot, object: &Object) -> Option<arc_base::Result<Vec<Piece>>> {
    pieces_in(snapshot, object, true)
}

/// [`wall_pieces`] without the openings of the wall's doors and windows, for a body that the
/// openings void separately (the IFC body).
#[must_use]
pub fn wall_pieces_whole(
    snapshot: &Snapshot,
    object: &Object,
) -> Option<arc_base::Result<Vec<Piece>>> {
    pieces_in(snapshot, object, false)
}

/// The net volume of the wall record `object`: the volume of its cleaned layers with the openings
/// of its doors and windows cut, under the slab above it and the roof it follows. This is the one
/// volume of a wall: the property grid, `inspect` and the IFC quantities all use it. `None` when the
/// record is not a readable wall or its layers cannot be made.
#[must_use]
pub fn wall_net_volume(snapshot: &Snapshot, object: &Object) -> Option<f64> {
    let pieces = wall_pieces(snapshot, object)?.ok()?;
    Some(pieces.iter().map(|p| p.solid.volume()).sum())
}

/// [`wall_net_volume`] when the layers of the wall's storey are already derived for `snapshot`;
/// `None` while they are not, so an interface thread never derives them itself (the extraction
/// workers do, and the caller asks again on a later frame).
#[must_use]
pub fn wall_net_volume_ready(snapshot: &Snapshot, object: &Object) -> Option<Option<f64>> {
    let storey = object.header.owner?;
    lookup(&LAYERS, storey, 0, &storey_deps(snapshot, storey, false))?;
    Some(wall_net_volume(snapshot, object))
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass over the bands of heights: layers, stacks, shapes, bands"
)]
fn pieces_in(
    snapshot: &Snapshot,
    object: &Object,
    openings: bool,
) -> Option<arc_base::Result<Vec<Piece>>> {
    let storey = object.header.owner?;
    let wall = Wall::of(snapshot, object)?;
    let (base, height) = wall_range(snapshot, object)?;
    let layers = storey_layers(snapshot, storey);
    if layers.indexes(object.id).is_empty() {
        return None;
    }
    let inserts = if openings {
        insert::openings_of(snapshot, object.id)
    } else {
        Vec::new()
    };
    let level = base - wall.base_offset;
    // A wall with a profile is its layers extruded from the profile, ends square.
    if let Some(profile) = Profile::of(&object.values)
        && profile.fits(wall.length(), wall.height)
        && let Some(spec) = layers.specs.get(layers.indexes(object.id)[0])
    {
        return Some(
            wall_shape::profile_solids(&wall, &profile, level, spec, &inserts, openings)
                .map(|shaped| shaped.into_iter().map(piece_of).collect()),
        );
    }
    // Bands apply to a plain vertical straight wall.
    let bands: Vec<Band> = if wall.is_arc() || wall.leans() {
        Vec::new()
    } else {
        Band::of(&object.values)
            .into_iter()
            .filter(|b| {
                b.fits(if b.vertical {
                    wall.length()
                } else {
                    wall.height
                })
            })
            .collect()
    };
    let mut marks: Vec<f64> = vec![0.0, height];
    for i in &inserts {
        marks.extend([i.sill.clamp(0.0, height), i.top().clamp(0.0, height)]);
    }
    for b in bands.iter().filter(|b| !b.vertical) {
        let (a, z) = b.range(wall.height);
        marks.extend([a.clamp(0.0, height), z.clamp(0.0, height)]);
    }
    // A stacked wall changes its layers at the borders of its segments.
    let ranges = stack_ranges_of(snapshot, object);
    for (from, to) in &ranges {
        marks.extend([from.clamp(0.0, height), to.clamp(0.0, height)]);
    }
    marks.sort_by(f64::total_cmp);
    marks.dedup_by(|a, b| (*a - *b).abs() < 1.0e-9);
    let roof = insert::attached_roof(snapshot, object).map(|(_, s)| s);
    let cleaned = floors(snapshot);
    let mut out = Vec::new();
    let tolerance = Tolerance::DEFAULT;
    for (band_at, band) in marks.windows(2).enumerate() {
        let (Some(&a), Some(&b)) = (band.first(), band.get(1)) else {
            continue;
        };
        let reaches_top = (b - height).abs() < 1.0e-9;
        let cuts = wall_shape::recess_cuts(&wall, &bands, a, b);
        let segment = ranges
            .iter()
            .position(|(from, to)| (a + b) / 2.0 >= *from && (a + b) / 2.0 < *to)
            .unwrap_or(0);
        let mut polys = layers.finished_wall_in(
            snapshot,
            object.id,
            if openings { (a + b) / 2.0 } else { f64::MAX },
            segment,
        );
        for cut in &cuts {
            polys = polys
                .into_iter()
                .flat_map(|p| layered::subtract(p, cut))
                .collect();
        }
        // A projecting band is united with the outermost (or innermost) layer it stands on, and
        // stops at the openings active at this height.
        let gaps: Vec<(f64, f64)> = inserts
            .iter()
            .filter(|i| i.sill <= (a + b) / 2.0 && (a + b) / 2.0 < i.top())
            .map(Insert::extent)
            .collect();
        for (face, rect) in wall_shape::band_rects(&wall, &bands, a, b, &gaps) {
            let spec = layers.specs.get(layers.indexes(object.id)[0]);
            let layer = spec.and_then(|s| match face {
                BandFace::Exterior => s.layers.iter().position(|l| l.thickness > 0.0),
                BandFace::Interior => s.layers.iter().rposition(|l| l.thickness > 0.0),
            });
            let Some(layer) = layer else { continue };
            polys = layered::unite_into(polys, layer, rect);
        }
        for p in polys {
            let Some(region) = poly_region(&p.points) else {
                continue;
            };
            let class = if p.core {
                PieceClass::Core
            } else if p.function == crate::element_type::Function::Finish {
                PieceClass::Finish
            } else {
                PieceClass::Other
            };
            // The top band follows the roof the wall is attached to.
            if reaches_top
                && let Some(surface) = &roof
                && let Some(solid) = clipped(&wall, &region, level, a, surface)
            {
                out.push(Piece {
                    class,
                    solid,
                    layer: p.layer,
                    segment,
                    key: piece_key(band_at, p.layer, 0),
                });
                continue;
            }
            // The ranges of heights this layer keeps, as (region, from, to).
            let mut parts: Vec<(Region, f64, f64)> = vec![(region, base + a, base + b)];
            for f in cleaned.iter() {
                let own = f.storey == Some(storey);
                parts = parts
                    .into_iter()
                    .flat_map(|(r, z0, z1)| split_by_floor(r, z0, z1, f, own, p.core))
                    .collect();
            }
            for (part_at, (region, z0, z1)) in parts.into_iter().enumerate() {
                if z1 - z0 < 1.0e-6 {
                    continue;
                }
                let made = if wall.leans() {
                    // A leaning layer is its polygon moved sideways by height.
                    flatten_region(&region).first().map_or_else(
                        || {
                            Err(arc_base::Error::new(
                                crate::error::codes::WALL_SHAPE_INVALID,
                            ))
                        },
                        |pts| wall_shape::leaning_prism(&wall, pts, z0, z1, base),
                    )
                } else {
                    prism(&region, z0, z1 - z0, CHORD, &tolerance, &Limits::DEFAULT)
                };
                match made {
                    Ok(solid) => out.push(Piece {
                        class,
                        solid,
                        layer: p.layer,
                        segment,
                        key: piece_key(band_at, p.layer, part_at),
                    }),
                    Err(e) => return Some(Err(e)),
                }
            }
        }
    }
    Some(Ok(out))
}

/// The piece of a shaped solid.
fn piece_of(s: wall_shape::Shaped) -> Piece {
    Piece {
        class: if s.core {
            PieceClass::Core
        } else if s.finish {
            PieceClass::Finish
        } else {
            PieceClass::Other
        },
        solid: s.solid,
        layer: s.layer,
        segment: 0,
        key: piece_key(1000, s.layer, s.part),
    }
}

/// The key of a piece: band, layer and part, each clamped to its share of the number.
fn piece_key(band: usize, layer: usize, part: usize) -> u32 {
    let pack = |v: usize, max: u32| u32::try_from(v).unwrap_or(max).min(max);
    (pack(band, 1023) << 14) | (pack(layer, 255) << 6) | pack(part, 63)
}

/// The solid of the layer `region` from `a` above the wall's base up under the roof `surface`.
fn clipped(
    wall: &Wall,
    region: &Region,
    level: f64,
    a: f64,
    surface: &crate::roof::Surface,
) -> Option<Polyhedron> {
    let mesh = arc_geom::triangulate_with(region, &Tolerance::DEFAULT, &Limits::DEFAULT).ok()?;
    let points = mesh.vertices();
    let triangles: Vec<Vec<P>> = mesh
        .triangles()
        .iter()
        .filter_map(|t| {
            let at = |k: usize| {
                points
                    .get(usize::try_from(*t.get(k)?).ok()?)
                    .map(|p| [p.x, p.y])
            };
            Some(vec![at(0)?, at(1)?, at(2)?])
        })
        .collect();
    let mut cut = *wall;
    cut.base_offset += a;
    cut.height -= a;
    crate::wall::under_roof_in(&cut, &triangles, level, surface, &|_, _| {
        arc_geom::FaceTag::Untagged
    })
    .ok()
}

/// `region` over the heights `z0` to `z1`, cut by the slab of `floor`. The part outside the slab's
/// plan keeps the heights. In the slab's own storey the part inside, if it is not core, keeps what
/// lies below the slab and above it (its layers stop at the slab); a wall of another storey ends
/// under the slab, core and layers alike.
fn split_by_floor(
    region: Region,
    z0: f64,
    z1: f64,
    floor: &CleanFloor,
    own: bool,
    core: bool,
) -> Vec<(Region, f64, f64)> {
    let (zb, zt) = floor.z;
    if z1 <= zb + 1.0e-9 || z0 >= zt - 1.0e-9 || (own && core) {
        return vec![(region, z0, z1)];
    }
    // A layer far from the slab in plan is not cut by it.
    let b = region
        .parts()
        .iter()
        .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
            let o = p.outer().bounding_box();
            [
                b[0].min(o.min().x),
                b[1].min(o.min().y),
                b[2].max(o.max().x),
                b[3].max(o.max().y),
            ]
        });
    let f = floor.bbox;
    if b[2] < f[0] || b[0] > f[2] || b[3] < f[1] || b[1] > f[3] {
        return vec![(region, z0, z1)];
    }
    let go = |op| {
        arc_geom::boolean(
            &region,
            &floor.faceted,
            op,
            &Tolerance::DEFAULT,
            &Limits::DEFAULT,
        )
        .ok()
        .flatten()
        .filter(|r| {
            r.measure(&Tolerance::DEFAULT)
                .is_ok_and(|m| m.area > 1.0e-9)
        })
    };
    let Some(inside) = go(BooleanOp::Intersection) else {
        return vec![(region, z0, z1)];
    };
    let mut out: Vec<(Region, f64, f64)> = Vec::new();
    if let Some(outside) = go(BooleanOp::Difference) {
        out.push((outside, z0, z1));
    }
    out.push((inside.clone(), z0, z1.min(zb)));
    if own {
        out.push((inside, z0.max(zt), z1));
    }
    out.into_iter().filter(|(_, a, b)| b - a > 1.0e-6).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_base::id::Identifier;

    fn object(id: u128) -> Arc<Object> {
        Arc::new(Object::new(
            ObjectId::from_u128(id),
            arc_doc::SchemaVersion { major: 1, minor: 0 },
            arc_doc::Header::default(),
            Vec::new(),
        ))
    }

    #[test]
    fn results_of_two_documents_under_one_identifier_are_both_kept() {
        let store: Store<u32> = Mutex::new(Vec::new());
        let key = ObjectId::from_u128(7);
        // Two documents with the same identifiers: their objects are not the same Arcs.
        let (a, b) = (vec![object(1), object(2)], vec![object(1), object(2)]);
        let first = cached(&store, key, 0, a.clone(), || 1);
        let other = cached(&store, key, 0, b.clone(), || 2);
        assert_eq!((*first, *other), (1, 2));
        assert!(Arc::ptr_eq(
            &first,
            &cached(&store, key, 0, a.clone(), || 3)
        ));
        assert!(Arc::ptr_eq(&other, &cached(&store, key, 0, b, || 4)));
        // A commit of the first document (one object replaced) replaces its result only.
        let next = vec![Arc::clone(&a[0]), object(2)];
        assert_eq!(*cached(&store, key, 0, next, || 5), 5);
        assert_eq!(store.lock().map(|c| c.len()).unwrap_or(0), 2);
    }
}
