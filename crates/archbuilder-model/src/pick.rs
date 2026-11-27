//! What is under a point of a plan view, as one ordered list: doors and windows first, then
//! walls, roofs near their outline or a ridge, hip or valley line, stairs, rooms, floors, roofs
//! anywhere over their outline, and levels. The application hovers the first one and a click selects it
//! (a repeated click steps down the list), so the highlighted object is the one picked.
//!
//! Within a kind the object of the higher storey comes first, as it is the one a view from above
//! sees; walls and levels further from the point come later.

use arc_base::ObjectId;
use arc_doc::Snapshot;

use crate::floor::Derived;
use crate::room::{self, Entry};
use crate::spatial;
use crate::types::{FLOOR, LEVEL, WALL, level};
use crate::wall::Wall;

/// Half the width and depth of the level outline, in meters (the viewport draws it).
pub const LEVEL_HALF: [f64; 2] = [12.0, 8.0];
/// The most objects of one kind that are looked at.
const MAX_PER_KIND: usize = 20_000;

/// The rooms of every storey with their derived data, for [`stack`] and [`outline`].
#[must_use]
pub fn all_rooms(snapshot: &Snapshot) -> Vec<Entry> {
    spatial::objects(snapshot, crate::types::STOREY)
        .iter()
        .flat_map(|s| room::of_storey(snapshot, s.id))
        .collect()
}

fn storey_elevation(snapshot: &Snapshot, owner: Option<ObjectId>) -> f64 {
    owner
        .and_then(|s| spatial::get(snapshot, s))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s))
}

/// The objects under `at` (project coordinates, meters) in picking order. `aperture` is how far
/// from a wall or level outline a point still counts, in meters; `ray` is the view ray, which
/// finds the door or window whose opening it enters first (a vertical ray through `at` without
/// one).
#[must_use]
#[allow(clippy::too_many_lines, reason = "one list of the pickable kinds")]
pub fn stack(
    snapshot: &Snapshot,
    rooms: &[Entry],
    at: [f64; 2],
    aperture: f64,
    ray: Option<([f64; 3], [f64; 3])>,
) -> Vec<ObjectId> {
    let mut out: Vec<ObjectId> = Vec::new();
    let ray = ray.unwrap_or(([at[0], at[1], 1.0e4], [0.0, 0.0, -1.0]));
    out.extend(crate::insert::pick(snapshot, ray));
    // A group instance is picked at its reference point; its elements are picked as walls.
    for instance in crate::group::instances(snapshot).iter().take(MAX_PER_KIND) {
        let d = (instance.origin[0] - at[0]).hypot(instance.origin[1] - at[1]);
        if d <= aperture {
            out.push(instance.id);
        }
    }
    let mut walls: Vec<(f64, f64, ObjectId)> = spatial::objects(snapshot, WALL)
        .iter()
        .take(MAX_PER_KIND)
        .filter_map(|o| {
            let w = Wall::from_values(&o.values)?;
            let miss = distance_to_body(&w, at);
            (miss <= aperture).then(|| (-storey_elevation(snapshot, o.header.owner), miss, o.id))
        })
        .collect();
    walls.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out.extend(walls.into_iter().map(|(_, _, id)| id));
    // A roof's lines come next; anywhere else over it, it comes after the floors.
    let mut on_lines: Vec<(f64, ObjectId)> = Vec::new();
    let mut over: Vec<(f64, ObjectId)> = Vec::new();
    for o in spatial::objects(snapshot, crate::types::ROOF)
        .iter()
        .take(MAX_PER_KIND)
    {
        let Ok(d) = crate::roof::Derived::of(snapshot, o) else {
            continue;
        };
        let n = d.surface.outline.len();
        let near = |a: [f64; 2], b: [f64; 2]| segment_distance(a, b, at) <= aperture;
        let edges =
            (0..n).any(
                |k| match (d.surface.outline.get(k), d.surface.outline.get((k + 1) % n)) {
                    (Some(a), Some(b)) => near(*a, *b),
                    _ => false,
                },
            );
        let lines = d.surface.lines().iter().any(|l| near(l.from, l.to));
        let z = -d.surface.top();
        if edges || lines {
            on_lines.push((z, o.id));
        } else if (crate::floor::Boundary {
            loops: vec![d.surface.outline.clone()],
            ..crate::floor::Boundary::default()
        })
        .contains(at)
        {
            over.push((z, o.id));
        }
    }
    // A roof by extrusion like a roof: on the lines of its pieces, else over it.
    for o in spatial::objects(snapshot, crate::types::ROOF_EXTRUSION)
        .iter()
        .take(MAX_PER_KIND)
    {
        let Ok(d) = crate::roof_extrusion::Derived::of(snapshot, o) else {
            continue;
        };
        let pieces = d.pieces();
        let z = -pieces
            .iter()
            .map(|p| p.plane.z)
            .fold(f64::MIN, f64::max)
            .max(d.level);
        let edges = pieces.iter().any(|p| {
            p.points
                .iter()
                .zip(p.points.iter().cycle().skip(1))
                .any(|(a, b)| segment_distance(*a, *b, at) <= aperture)
        });
        if edges {
            on_lines.push((z, o.id));
        } else if pieces
            .iter()
            .any(|p| crate::floor::Boundary::polygons(vec![p.points.clone()]).contains(at))
        {
            over.push((z, o.id));
        }
    }
    // A floor edge lies along its floor's edges: picked on its line, before the floor.
    for o in spatial::objects(snapshot, crate::types::FLOOR_EDGE)
        .iter()
        .take(MAX_PER_KIND)
    {
        let Some(d) = crate::floor_edge::Derived::of(snapshot, o) else {
            continue;
        };
        let near = d.chains.iter().any(|c| {
            c.points
                .windows(2)
                .any(|w| segment_distance([w[0][0], w[0][1]], [w[1][0], w[1][1]], at) <= aperture)
        });
        let z = d
            .chains
            .iter()
            .flat_map(|c| c.points.iter().map(|p| p[2]))
            .fold(f64::MIN, f64::max);
        if near {
            on_lines.push((-z, o.id));
        }
    }
    // A roof edge element lies along its roof's eave: picked on its line, like a floor edge.
    for o in spatial::objects(snapshot, crate::types::ROOF_EDGE)
        .iter()
        .take(MAX_PER_KIND)
    {
        let Some(d) = crate::roof_edge::Derived::of(snapshot, o) else {
            continue;
        };
        let near = d.chains.iter().any(|c| {
            c.points
                .windows(2)
                .any(|w| segment_distance([w[0][0], w[0][1]], [w[1][0], w[1][1]], at) <= aperture)
        });
        let z = d
            .chains
            .iter()
            .flat_map(|c| c.points.iter().map(|p| p[2]))
            .fold(f64::MIN, f64::max);
        if near {
            on_lines.push((-z, o.id));
        }
    }
    // A dormer is picked over its opening in the roof.
    for o in spatial::objects(snapshot, crate::types::DORMER)
        .iter()
        .take(MAX_PER_KIND)
    {
        if let Some(dormer) = crate::dormer::Dormer::from_values(&o.values)
            && crate::floor::Boundary::polygons(vec![dormer.opening()]).contains(at)
        {
            on_lines.push((f64::MIN, o.id));
        }
    }
    // A component is picked over its footprint.
    for o in spatial::objects(snapshot, crate::types::COMPONENT)
        .iter()
        .take(MAX_PER_KIND)
    {
        let Some(c) = crate::family::loaded::Component::of(o) else {
            continue;
        };
        let outline = crate::family::loaded::outline(snapshot, &c);
        if outline.len() == 4 && crate::floor::Boundary::polygons(vec![outline]).contains(at) {
            on_lines.push((f64::MIN, o.id));
        }
    }
    on_lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    over.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(on_lines.into_iter().map(|(_, id)| id));
    // A ceiling's edge is picked before what lies under it: the plan draws it dashed over the room.
    let mut edges: Vec<(f64, ObjectId)> = Vec::new();
    for o in spatial::objects(snapshot, crate::types::CEILING)
        .iter()
        .take(MAX_PER_KIND)
    {
        let Some(d) = crate::ceiling::derived(snapshot, o) else {
            continue;
        };
        let near = d
            .boundary
            .flattened(crate::floor::FLAT_CHORD)
            .iter()
            .any(|l| {
                l.iter()
                    .zip(l.iter().cycle().skip(1))
                    .any(|(a, b)| segment_distance(*a, *b, at) <= aperture)
            });
        if near {
            edges.push((-d.plane.z, o.id));
        }
    }
    edges.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(edges.into_iter().map(|(_, id)| id));
    // Stairs come after walls: a stair stands between walls, in front of the room around it.
    let mut stairs: Vec<(f64, ObjectId)> = spatial::objects(snapshot, crate::types::STAIR)
        .iter()
        .take(MAX_PER_KIND)
        .filter_map(|o| {
            let p = crate::stair::Placed::of(snapshot, o)?;
            let outline = crate::floor::Boundary {
                loops: vec![p.outline()],
                ..crate::floor::Boundary::default()
            };
            outline.contains(at).then_some((-p.base, o.id))
        })
        .collect();
    stairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(stairs.into_iter().map(|(_, id)| id));
    // Ramps like stairs; railings by their line.
    let mut circulation: Vec<(f64, ObjectId)> = spatial::objects(snapshot, crate::types::RAMP)
        .iter()
        .take(MAX_PER_KIND)
        .filter_map(|o| {
            let p = crate::ramp::Placed::of(snapshot, o)?;
            let outline = crate::floor::Boundary {
                loops: vec![p.outline()],
                ..crate::floor::Boundary::default()
            };
            outline.contains(at).then_some((-p.base, o.id))
        })
        .collect();
    circulation.extend(
        spatial::objects(snapshot, crate::types::RAILING)
            .iter()
            .take(MAX_PER_KIND)
            .filter_map(|o| {
                let d = crate::railing::Derived::of(snapshot, o).ok()?;
                let line = d.plan_line();
                line.windows(2)
                    .any(|w| segment_distance(w[0], w[1], at) <= aperture)
                    .then_some((-d.base, o.id))
            }),
    );
    circulation.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(circulation.into_iter().map(|(_, id)| id));
    // Shafts and vertical openings: the cuts come before the room around them.
    let mut cuts: Vec<(f64, ObjectId)> = Vec::new();
    for s in crate::opening::shafts(snapshot) {
        if crate::floor::Boundary::polygons(vec![s.outline.clone()]).contains(at) {
            cuts.push((-s.z.1, s.id));
        }
    }
    for o in spatial::objects(snapshot, crate::types::VERTICAL_OPENING)
        .iter()
        .take(MAX_PER_KIND)
    {
        if let Some(outline) = crate::opening::outline_of(snapshot, o, crate::floor::VERTICAL_SLOTS)
            && crate::floor::Boundary::polygons(vec![outline]).contains(at)
        {
            cuts.push((0.0, o.id));
        }
    }
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(cuts.into_iter().map(|(_, id)| id));
    let mut hit_rooms: Vec<(f64, ObjectId)> = rooms
        .iter()
        .filter(|e| {
            let p = e.room.point;
            match e.state() {
                room::State::Unenclosed => (p[0] - at[0]).hypot(p[1] - at[1]) <= aperture * 2.0,
                _ => e.footprint.boundary().contains(at),
            }
        })
        .map(|e| (-e.base, e.id))
        .collect();
    hit_rooms.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(hit_rooms.into_iter().map(|(_, id)| id));
    let mut floors: Vec<(f64, ObjectId)> = spatial::objects(snapshot, FLOOR)
        .iter()
        .take(MAX_PER_KIND)
        .filter_map(|o| {
            let d = Derived::of(snapshot, o)?;
            d.boundary.contains(at).then_some((-d.top, o.id))
        })
        .collect();
    floors.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(floors.into_iter().map(|(_, id)| id));
    out.extend(over.into_iter().map(|(_, id)| id));
    // A ceiling hangs above the plan's cut: it never comes before what the plan cuts or fills.
    let mut ceilings: Vec<(f64, ObjectId)> = spatial::objects(snapshot, crate::types::CEILING)
        .iter()
        .take(MAX_PER_KIND)
        .filter_map(|o| {
            let d = crate::ceiling::derived(snapshot, o)?;
            d.boundary.contains(at).then_some((-d.plane.z, o.id))
        })
        .collect();
    ceilings.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(ceilings.into_iter().map(|(_, id)| id));
    let mut levels: Vec<(f64, ObjectId)> = spatial::objects(snapshot, LEVEL)
        .iter()
        .filter(|_| near_level_outline(at, aperture))
        .map(|o| {
            (
                -crate::types::meters(&o.values, level::ELEVATION).unwrap_or(0.0),
                o.id,
            )
        })
        .collect();
    levels.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.extend(levels.into_iter().map(|(_, id)| id));
    // Datum lines are thin: they are picked where nothing else is.
    out.extend(crate::datum::pick(snapshot, at, aperture));
    out.dedup();
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|id| seen.insert(*id));
    out
}

fn near_level_outline(at: [f64; 2], aperture: f64) -> bool {
    let (hx, hy) = (LEVEL_HALF[0], LEVEL_HALF[1]);
    let inside_outer = at[0].abs() <= hx + aperture && at[1].abs() <= hy + aperture;
    let inside_inner = at[0].abs() < hx - aperture && at[1].abs() < hy - aperture;
    inside_outer && !inside_inner
}

/// The distance from `p` to the segment `ab`, in meters.
fn segment_distance(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let length2 = d[0] * d[0] + d[1] * d[1];
    let t = if length2 > 0.0 {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / length2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - d[0] * t).hypot(p[1] - a[1] - d[1] * t)
}

/// The distance from `p` to the body of `wall` (0 inside), in meters.
fn segment_miss(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l2 = d[0] * d[0] + d[1] * d[1];
    let t = if l2 > 0.0 {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - d[0] * t).hypot(p[1] - a[1] - d[1] * t)
}

fn distance_to_body(wall: &Wall, p: [f64; 2]) -> f64 {
    let length = wall.length();
    if length <= 0.0 {
        return f64::MAX;
    }
    if wall.is_arc() {
        let outline = wall.plain_outline();
        if crate::wall::inside_outline(&outline, p) {
            return 0.0;
        }
        return (0..outline.len())
            .map(|k| segment_miss(p, outline[k], outline[(k + 1) % outline.len()]))
            .fold(f64::MAX, f64::min);
    }
    let (ux, uy) = (
        (wall.end[0] - wall.start[0]) / length,
        (wall.end[1] - wall.start[1]) / length,
    );
    let rel = [p[0] - wall.start[0], p[1] - wall.start[1]];
    let along = rel[0] * ux + rel[1] * uy;
    let across = ux * rel[1] - uy * rel[0];
    let (low, high) = wall.extent();
    let gap = (low - across).max(across - high).max(0.0);
    let beyond = (-along).max(along - length).max(0.0);
    gap.hypot(beyond)
}

/// The elevation of the top of the floor, roof or ceiling a vertical opening is cut in, or of
/// the host `opening` names.
#[must_use]
pub fn host_top_of(snapshot: &Snapshot, opening: &std::sync::Arc<arc_doc::Object>) -> f64 {
    opening
        .header
        .owner
        .and_then(|h| spatial::get(snapshot, h))
        .map_or(0.0, |h| host_top(snapshot, &h))
}

/// The elevation of the top of a floor, roof or ceiling.
fn host_top(snapshot: &Snapshot, host: &std::sync::Arc<arc_doc::Object>) -> f64 {
    match spatial::type_of(snapshot, host.id).as_deref() {
        Some(FLOOR) => Derived::of(snapshot, host).map_or(0.0, |d| d.top),
        Some(crate::types::ROOF) => {
            crate::roof::Derived::of(snapshot, host).map_or(0.0, |d| d.surface.top())
        }
        _ => crate::ceiling::derived(snapshot, host).map_or(0.0, |d| d.z_range().1),
    }
}

/// The closed outlines that mark `id` for the hover: the loops of its plan shape as 3D points.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one arm for each kind of element")]
pub fn outline(snapshot: &Snapshot, rooms: &[Entry], id: ObjectId) -> Vec<Vec<[f64; 3]>> {
    let Some(object) = spatial::get(snapshot, id) else {
        return Vec::new();
    };
    let close = |points: &[[f64; 2]], z: f64| -> Vec<[f64; 3]> {
        points
            .iter()
            .chain(points.first())
            .map(|p| [p[0], p[1], z])
            .collect()
    };
    match spatial::type_of(snapshot, id).as_deref() {
        Some(WALL) => {
            let Some(w) = Wall::of(snapshot, &object) else {
                return Vec::new();
            };
            let z = storey_elevation(snapshot, object.header.owner) + w.base_offset + w.height;
            // The outline of the layers, which meet the walls around as the body does.
            let loops = crate::cleanup::wall_outline(snapshot, &object);
            if loops.is_empty() {
                vec![close(&w.plain_outline(), z)]
            } else {
                loops.iter().map(|l| close(l, z)).collect()
            }
        }
        Some(FLOOR) => Derived::of(snapshot, &object)
            .map(|d| d.boundary.loops.iter().map(|l| close(l, d.top)).collect())
            .unwrap_or_default(),
        Some(crate::types::CEILING) => crate::ceiling::derived(snapshot, &object)
            .map(|d| {
                d.boundary
                    .loops
                    .iter()
                    .map(|l| close(l, d.plane.z))
                    .collect()
            })
            .unwrap_or_default(),
        Some(crate::types::SHAFT) => crate::opening::shaft_span(snapshot, &object)
            .map(|s| vec![close(&s.outline, s.z.0), close(&s.outline, s.z.1)])
            .unwrap_or_default(),
        Some(crate::types::ELEVATOR) => crate::elevator::span(snapshot, &object)
            .map(|s| vec![close(&s.outline, s.z.0), close(&s.outline, s.z.1)])
            .unwrap_or_default(),
        Some(crate::types::DORMER) => crate::dormer::Derived::of(snapshot, &object)
            .map(|d| {
                d.parts
                    .iter()
                    .flat_map(|(_, s)| s.vertices().iter().map(|v| [v.x, v.y, v.z]))
                    .collect::<Vec<_>>()
            })
            .map(|v| vec![v])
            .unwrap_or_default(),
        Some(crate::types::ROOF_EDGE) => crate::roof_edge::Derived::of(snapshot, &object)
            .map(|d| d.chains.iter().map(|c| c.points.clone()).collect())
            .unwrap_or_default(),
        Some(crate::types::VERTICAL_OPENING) => {
            crate::opening::outline_of(snapshot, &object, crate::floor::VERTICAL_SLOTS)
                .map(|o| {
                    let z = object
                        .header
                        .owner
                        .and_then(|h| spatial::get(snapshot, h))
                        .map_or(0.0, |h| host_top(snapshot, &h));
                    vec![close(&o, z)]
                })
                .unwrap_or_default()
        }
        Some(crate::types::ROOF) => crate::roof::Derived::of(snapshot, &object)
            .map(|d| vec![close(&d.surface.outline, d.surface.base)])
            .unwrap_or_default(),
        Some(crate::types::STAIR) => crate::stair::Placed::of(snapshot, &object)
            .map(|p| vec![close(&p.outline(), p.base)])
            .unwrap_or_default(),
        Some(crate::types::RAMP) => crate::ramp::Placed::of(snapshot, &object)
            .map(|p| vec![close(&p.outline(), p.base)])
            .unwrap_or_default(),
        Some(crate::types::RAILING) => crate::railing::Derived::of(snapshot, &object)
            .map(|d| vec![close(&d.plan_line(), d.base)])
            .unwrap_or_default(),
        Some(crate::types::GRID | crate::types::REFERENCE_PLANE) => {
            crate::datum::outline(snapshot, &object)
        }
        Some(crate::types::ROOF_EXTRUSION) => crate::roof_extrusion::Derived::of(snapshot, &object)
            .map(|d| {
                d.pieces()
                    .iter()
                    .map(|p| close(&p.points, p.plane.z))
                    .collect()
            })
            .unwrap_or_default(),
        Some(crate::types::FLOOR_EDGE) => crate::floor_edge::Derived::of(snapshot, &object)
            .map(|d| d.chains.iter().map(|c| c.points.clone()).collect())
            .unwrap_or_default(),
        Some(LEVEL) => {
            let z = crate::types::meters(&object.values, level::ELEVATION).unwrap_or(0.0);
            let (hx, hy) = (LEVEL_HALF[0], LEVEL_HALF[1]);
            vec![close(&[[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy]], z)]
        }
        Some(crate::types::ROOM) => rooms
            .iter()
            .find(|e| e.id == id)
            .map(|e| {
                if e.footprint.polygon.is_empty() {
                    let (p, r) = (e.room.point, 0.3);
                    vec![close(
                        &[
                            [p[0] - r, p[1] - r],
                            [p[0] + r, p[1] - r],
                            [p[0] + r, p[1] + r],
                            [p[0] - r, p[1] + r],
                        ],
                        e.base,
                    )]
                } else {
                    std::iter::once(&e.footprint.polygon)
                        .chain(&e.footprint.holes)
                        .map(|l| close(l, e.base))
                        .collect()
                }
            })
            .unwrap_or_default(),
        Some(crate::types::DOOR | crate::types::WINDOW | crate::types::WALL_OPENING) => {
            let item = crate::insert::Insert::of(snapshot, &object);
            let host = object.header.owner.and_then(|w| spatial::get(snapshot, w));
            match (item, host.as_ref().and_then(|h| Wall::of(snapshot, h))) {
                (Some(item), Some(wall)) => crate::insert::outline(
                    &wall,
                    storey_elevation(snapshot, host.and_then(|h| h.header.owner)),
                    &item,
                )
                .unwrap_or_default(),
                _ => Vec::new(),
            }
        }
        _ => Vec::new(),
    }
}
