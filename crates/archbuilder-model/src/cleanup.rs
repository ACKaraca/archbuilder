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
use crate::layered::{self, Layout, Poly, Spec, area, finish_wall};
use crate::types::{FLOOR, WALL};
use crate::wall::Wall;
use crate::{insert, spatial};

type P = [f64; 2];

/// The most walls of a document looked at for the cleaning of one floor.
const MAX_WALLS: usize = 20_000;
const CHORD: f64 = 0.01;

/// The layers of the walls of one storey at one snapshot.
#[derive(Debug)]
pub struct StoreyLayers {
    /// The walls, in identifier order.
    pub ids: Vec<ObjectId>,
    /// Their data.
    pub walls: Vec<Wall>,
    /// Their layers.
    pub specs: Vec<Spec>,
    /// The cleaned layer polygons.
    pub layout: Layout,
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

/// The result kept in `store` for (`id`, `extra`) when it was made from `deps`, else `make()`,
/// which is then kept. The entry holds its objects, so an address is never reused under it.
pub(crate) fn cached<T>(
    store: &Store<T>,
    id: ObjectId,
    extra: u64,
    deps: Deps,
    make: impl FnOnce() -> T,
) -> Arc<T> {
    let hit = store.lock().ok().and_then(|c| {
        c.iter()
            .find(|e| e.id == id && e.extra == extra && same(&e.deps, &deps))
            .map(|e| Arc::clone(&e.value))
    });
    hit.unwrap_or_else(|| {
        let value = Arc::new(make());
        if let Ok(mut c) = store.lock() {
            c.retain(|e| !(e.id == id && e.extra == extra));
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
            for o in spatial::walls(snapshot, storey) {
                if let Some(w) = Wall::from_values(&o.values) {
                    specs.push(Spec::of(snapshot, &o, &w));
                    ids.push(o.id);
                    walls.push(w);
                }
            }
            let layout = layered::layout(&walls, &specs);
            StoreyLayers {
                ids,
                walls,
                specs,
                layout,
            }
        },
    )
}

impl StoreyLayers {
    /// The index of the wall `id`.
    fn index(&self, id: ObjectId) -> Option<usize> {
        self.ids.iter().position(|i| *i == id)
    }

    /// The polygons of the wall at `at` with openings active at the height `z` above its base.
    /// Kept while the polygons the layout gave the wall, its openings and its type are the same,
    /// so a move elsewhere in the storey does not make them again.
    fn finished(&self, snapshot: &Snapshot, at: usize, z: f64) -> Vec<Poly> {
        let (Some(wall), Some(spec), Some(id)) =
            (self.walls.get(at), self.specs.get(at), self.ids.get(at))
        else {
            return Vec::new();
        };
        let own: Vec<Poly> = self
            .layout
            .polys
            .iter()
            .filter(|p| p.wall == at)
            .cloned()
            .collect();
        let gaps: Vec<(f64, f64)> = insert::hosted(snapshot, *id)
            .into_iter()
            .filter(|(_, i)| i.sill <= z && z < i.top())
            .map(|(_, i)| i.extent())
            .collect();
        let free = self.layout.free.get(at).copied().unwrap_or([true; 2]);
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
            for at in 0..layers.walls.len() {
                all.extend(layers.finished(snapshot, at, cut));
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
    layers
        .index(object.id)
        .map(|at| layered::outline(&layers.finished(snapshot, at, f64::MAX)))
        .unwrap_or_default()
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
        let Some(at) = layers.index(wall_object.id) else {
            continue;
        };
        let Some(wall) = layers.walls.get(at) else {
            continue;
        };
        let polys = layers.finished(snapshot, at, (from + to) / 2.0 - base);
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
    /// The layer, by index in the wall type's layers.
    pub layer: usize,
    /// A key of the piece that stays the same while its wall's openings and layers stay: the
    /// band of heights, the layer and the part of the layer packed in one number.
    pub key: u32,
}

/// The material of each layer of the wall record `object`, by layer index; empty for a wall
/// whose type does not give its layers (a plain wall).
#[must_use]
pub fn layer_materials(snapshot: &Snapshot, object: &Object) -> Vec<String> {
    use crate::element_type::{Layers, TypeKind, type_of};
    let wall = Wall::of(snapshot, object);
    type_of(snapshot, TypeKind::Wall, &object.values)
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

fn pieces_in(
    snapshot: &Snapshot,
    object: &Object,
    openings: bool,
) -> Option<arc_base::Result<Vec<Piece>>> {
    let storey = object.header.owner?;
    let wall = Wall::of(snapshot, object)?;
    let (base, height) = wall_range(snapshot, object)?;
    let layers = storey_layers(snapshot, storey);
    let at = layers.index(object.id)?;
    let inserts = if openings {
        insert::hosted(snapshot, object.id)
    } else {
        Vec::new()
    };
    let mut marks: Vec<f64> = vec![0.0, height];
    for (_, i) in &inserts {
        marks.extend([i.sill.clamp(0.0, height), i.top().clamp(0.0, height)]);
    }
    marks.sort_by(f64::total_cmp);
    marks.dedup_by(|a, b| (*a - *b).abs() < 1.0e-9);
    let roof = insert::attached_roof(snapshot, object).map(|(_, s)| s);
    let cleaned = floors(snapshot);
    let level = base - wall.base_offset;
    let mut out = Vec::new();
    let tolerance = Tolerance::DEFAULT;
    for (band_at, band) in marks.windows(2).enumerate() {
        let (Some(&a), Some(&b)) = (band.first(), band.get(1)) else {
            continue;
        };
        let reaches_top = (b - height).abs() < 1.0e-9;
        for p in layers.finished(
            snapshot,
            at,
            if openings { (a + b) / 2.0 } else { f64::MAX },
        ) {
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
                match prism(&region, z0, z1 - z0, CHORD, &tolerance, &Limits::DEFAULT) {
                    Ok(solid) => out.push(Piece {
                        class,
                        solid,
                        layer: p.layer,
                        key: piece_key(band_at, p.layer, part_at),
                    }),
                    Err(e) => return Some(Err(e)),
                }
            }
        }
    }
    Some(Ok(out))
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
