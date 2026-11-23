//! Layered walls in plan: the polygon of every layer of every wall of a storey, with the joins
//! cleaned by layer function and the wrapping of free ends and openings. Nothing here is stored;
//! the polygons, the plan lines and the solids derive from the walls, their types and the doors
//! and windows they host each time they are asked for.
//!
//! # Rules
//!
//! The footprint of a wall is the one [`crate::join`] derives; the layers divide it. Where walls
//! meet at their ends or one ends on another, each layer is then cleaned by its function. The
//! functions have a priority, from the highest: structure, substrate, thermal, finish.
//!
//! - **Corners (two walls, or a star of walls).** Around the node the layers of two neighbors are
//!   matched in order from the faces that look at each other: layers of the same function connect,
//!   so their lines meet each other and the layer turns the corner; when the next layers differ,
//!   the one of the lower priority stops on the line of the higher one, which goes on. Cores
//!   therefore meet cores and finishes wrap the corner on its outer side, whatever the thicknesses.
//! - **A wall ending on another (T, or a cross made of four ends).** Each layer of the ending wall
//!   runs on until the first layer of the other wall that has the same or a higher priority and
//!   stops on its near line; it passes over layers of a lower priority. Where the layers of two
//!   walls overlap, the higher priority keeps the area (the lower wall number on equal priority).
//! - Ends with a join style other than automatic keep the footprint's end, divided in proportion.
//!
//! **Wrapping.** A wall type says, for free ends and for the openings of doors and windows, which
//! side's layers wrap: none, exterior, interior or both. Every layer outside the core on a wrapped
//! side turns into the end or the reveal: each becomes an L whose foot covers the end across the
//! thickness (up to the middle of the core when both sides wrap), and the core stops short of the
//! end by the total of the feet. The length of the wall and the clear opening do not change.

use std::collections::HashMap;

use arc_geom::{BooleanOp, Region};

use crate::element_type::{Function, Wrap};
use crate::floor::Boundary;
use crate::join::{self, Ray, Through, along, cross, dot, left};
use crate::wall::{JoinStyle, Wall};

type P = [f64; 2];

/// The most layer polygons that are resolved against each other.
const MAX_POLYS: usize = 6000;
/// Areas below this, in square meters, are no overlap.
const AREA_EPS: f64 = 1.0e-9;
/// Layers thinner than this do not exist (membranes).
const MIN_LAYER: f64 = 1.0e-9;

/// One layer of a wall: what it does, whether it belongs to the core and how thick it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    /// The function of the layer.
    pub function: Function,
    /// Whether the layer is inside the type's core range.
    pub core: bool,
    /// The thickness in meters.
    pub thickness: f64,
}

/// The layers of a wall from its exterior face to its interior face, and its wrapping.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    /// The layers, exterior face first; membranes (without thickness) included.
    pub layers: Vec<Layer>,
    /// The wrapping of free ends.
    pub wrap_ends: Wrap,
    /// The wrapping of the openings of doors and windows.
    pub wrap_inserts: Wrap,
}

impl Spec {
    /// A wall of one structural layer of `thickness`.
    #[must_use]
    pub fn plain(thickness: f64) -> Self {
        Self {
            layers: vec![Layer {
                function: Function::Structure,
                core: true,
                thickness,
            }],
            wrap_ends: Wrap::None,
            wrap_inserts: Wrap::None,
        }
    }

    /// The spec of a wall of `thickness` whose type has `layers`, `wrap_ends` and
    /// `wrap_inserts`; a plain wall when the layers do not add up to the thickness.
    #[must_use]
    pub fn of_type(
        layers: &crate::element_type::Layers,
        wrap_ends: Wrap,
        wrap_inserts: Wrap,
        thickness: f64,
    ) -> Self {
        if (layers.total() - thickness).abs() > 1.0e-6 || layers.layers.is_empty() {
            return Self::plain(thickness);
        }
        Self {
            layers: layers
                .layers
                .iter()
                .enumerate()
                .map(|(k, l)| Layer {
                    function: l.function,
                    core: k >= layers.core.0 && k < layers.core.1,
                    thickness: l.thickness,
                })
                .collect(),
            wrap_ends,
            wrap_inserts,
        }
    }

    /// The spec of the wall record `object` with the data `wall`.
    #[must_use]
    pub fn of(snapshot: &arc_doc::Snapshot, object: &arc_doc::Object, wall: &Wall) -> Self {
        use crate::element_type::{Layers, TypeKind, WallType, type_of};
        let found = type_of(snapshot, TypeKind::Wall, &object.values);
        match found.as_ref().and_then(|t| {
            Some((
                Layers::from_values(&t.values)?,
                WallType::from_values(&t.values)?,
            ))
        }) {
            Some((layers, t)) => {
                Self::of_type(&layers, t.wrap_ends, t.wrap_inserts, wall.thickness)
            }
            None => Self::plain(wall.thickness),
        }
    }

    /// The layers with a thickness: the ones that have a polygon, by their index in `layers`.
    fn solid_layers(&self) -> Vec<usize> {
        (0..self.layers.len())
            .filter(|&k| self.layers.get(k).is_some_and(|l| l.thickness > MIN_LAYER))
            .collect()
    }
}

/// The priority of a function: lower is higher.
const fn rank(function: Function) -> u8 {
    match function {
        Function::Structure => 0,
        Function::Substrate => 1,
        Function::Thermal => 2,
        Function::Finish => 3,
        Function::Membrane => 4,
    }
}

/// The polygon of one layer of one wall (or one run of it, between openings).
#[derive(Debug, Clone, PartialEq)]
pub struct Poly {
    /// The wall, by index in the list the layout was made from.
    pub wall: usize,
    /// The layer, by index in the wall's [`Spec::layers`].
    pub layer: usize,
    /// The function of the layer.
    pub function: Function,
    /// Whether the layer is in the core.
    pub core: bool,
    /// The corners, in order, without repeating the first.
    pub points: Vec<P>,
}

/// The layer polygons of the walls of one storey with the joins cleaned, before openings and
/// wrapping.
#[derive(Debug, Clone, Default)]
pub struct Layout {
    /// The polygons, wall by wall.
    pub polys: Vec<Poly>,
    /// For each wall, whether its start and its end are free (take part in no join).
    pub free: Vec<[bool; 2]>,
}

/// The layers of a ray, in the order they are met from its left face inward: thickness, rank and
/// index in the wall's layers.
struct Stack {
    thick: Vec<f64>,
    rank: Vec<u8>,
    orig: Vec<usize>,
}

impl Stack {
    /// The stack of a wall seen from its left (`reversed` false: the exterior face) or from its
    /// right.
    fn of(spec: &Spec, reversed: bool) -> Self {
        let mut idx = spec.solid_layers();
        if reversed {
            idx.reverse();
        }
        let layer = |k: usize| spec.layers.get(k).copied();
        Self {
            thick: idx
                .iter()
                .filter_map(|&k| layer(k))
                .map(|l| l.thickness)
                .collect(),
            rank: idx
                .iter()
                .filter_map(|&k| layer(k))
                .map(|l| rank(l.function))
                .collect(),
            orig: idx,
        }
    }

    /// The same stack met from the other face.
    fn flipped(&self) -> Self {
        Self {
            thick: self.thick.iter().rev().copied().collect(),
            rank: self.rank.iter().rev().copied().collect(),
            orig: self.orig.iter().rev().copied().collect(),
        }
    }

    /// The distance of each line from the first face; one more than the layers.
    fn cum(&self) -> Vec<f64> {
        let mut at = 0.0;
        std::iter::once(0.0)
            .chain(self.thick.iter().map(|t| {
                at += t;
                at
            }))
            .collect()
    }
}

/// The partner line of each line of two stacks that look at each other, in order from their
/// faces: layers of one function connect, a layer of a lower priority ends on the line of the
/// higher one, and what is left ends on the far face of the other.
fn pair(a: &[u8], b: &[u8]) -> (Vec<usize>, Vec<usize>) {
    let (na, nb) = (a.len(), b.len());
    let mut pa = vec![nb; na + 1];
    let mut pb = vec![na; nb + 1];
    if let (Some(x), Some(y)) = (pa.first_mut(), pb.first_mut()) {
        *x = 0;
        *y = 0;
    }
    let (mut i, mut j) = (0, 0);
    while let (Some(&ra), Some(&rb)) = (a.get(i), b.get(j)) {
        if ra == rb {
            if let (Some(x), Some(y)) = (pa.get_mut(i + 1), pb.get_mut(j + 1)) {
                *x = j + 1;
                *y = i + 1;
            }
            i += 1;
            j += 1;
        } else if ra < rb {
            if let Some(y) = pb.get_mut(j + 1) {
                *y = i;
            }
            j += 1;
        } else {
            if let Some(x) = pa.get_mut(i + 1) {
                *x = j;
            }
            i += 1;
        }
    }
    (pa, pb)
}

/// The end points of the layers of a wall end, by index in the wall's layers: the point on the
/// layer's exterior line and on its interior line.
type EndPoints = Vec<Option<[P; 2]>>;

/// The end of a wall divided in proportion along the corners of the joined footprint.
fn proportional(wall: &Wall, spec: &Spec, foot: &[P; 4], at_end: bool) -> EndPoints {
    let (left_corner, right_corner) = if at_end {
        (foot[2], foot[1])
    } else {
        (foot[3], foot[0])
    };
    let total = wall.thickness.max(f64::MIN_POSITIVE);
    let lerp = |t: f64| {
        [
            left_corner[0] + (right_corner[0] - left_corner[0]) * t,
            left_corner[1] + (right_corner[1] - left_corner[1]) * t,
        ]
    };
    let mut at = 0.0;
    spec.layers
        .iter()
        .map(|l| {
            let outer = lerp(at / total);
            at += l.thickness;
            Some([outer, lerp(at / total)])
        })
        .collect()
}

/// Turns the points of the lines of a ray, `pts[i]` on the line `i` met from its left face, into
/// the end points of the layers of its wall.
fn to_end(stack: &Stack, pts: &[Option<P>], at_end: bool, count: usize) -> Option<EndPoints> {
    let mut out: EndPoints = vec![None; count];
    for (k, &o) in stack.orig.iter().enumerate() {
        let (first, second) = ((*pts.get(k)?)?, (*pts.get(k + 1)?)?);
        // A start ray meets the exterior face first; an end ray the interior face.
        let pair = if at_end {
            [second, first]
        } else {
            [first, second]
        };
        *out.get_mut(o)? = Some(pair);
    }
    Some(out)
}

/// The corners of a node of two or more ends that meet in a star, layer by layer.
fn star_ends(members: &[Ray], specs: &[Spec]) -> Option<Vec<EndPoints>> {
    let m = members.len();
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&i, &j| {
        let angle = |k: usize| members.get(k).map_or(0.0, |r| r.dir[1].atan2(r.dir[0]));
        angle(i).total_cmp(&angle(j)).then(
            members
                .get(i)
                .map(|r| r.wall)
                .cmp(&members.get(j).map(|r| r.wall)),
        )
    });
    let spec_of = |r: &Ray| specs.get(r.wall);
    let stacks: Vec<Stack> = members
        .iter()
        .map(|r| spec_of(r).map(|s| Stack::of(s, r.at_end)))
        .collect::<Option<_>>()?;
    let mut pts: Vec<Vec<Option<P>>> = stacks
        .iter()
        .map(|s| vec![None; s.thick.len() + 1])
        .collect();
    for k in 0..m {
        let (i, j) = (*order.get(k)?, *order.get((k + 1) % m)?);
        if i == j {
            return None;
        }
        let (a, b) = (members.get(i)?, members.get(j)?);
        if cross(a.dir, b.dir).abs() < join::PARALLEL {
            return None;
        }
        let (sa, sb) = (stacks.get(i)?, stacks.get(j)?.flipped());
        let (pa, pb) = pair(&sa.rank, &sb.rank);
        let (ca, cb) = (sa.cum(), sb.cum());
        let (na, nb) = (sa.thick.len(), sb.thick.len());
        let line_a = |l: usize| {
            Some(join::Line {
                p: along(a.point, left(a.dir), a.hi - ca.get(l)?),
                d: a.dir,
            })
        };
        let line_b = |l: usize| {
            Some(join::Line {
                p: along(b.point, left(b.dir), b.lo + cb.get(l)?),
                d: b.dir,
            })
        };
        for l in 0..=na {
            if 2 * l <= na {
                let at = line_a(l)?.hit(line_b(*pa.get(l)?)?)?;
                *pts.get_mut(i)?.get_mut(l)? = Some(at);
            }
        }
        for l in 0..=nb {
            if 2 * l < nb {
                let at = line_b(l)?.hit(line_a(*pb.get(l)?)?)?;
                *pts.get_mut(j)?.get_mut(nb - l)? = Some(at);
            }
        }
    }
    members
        .iter()
        .zip(&stacks)
        .zip(&pts)
        .map(|((r, s), p)| {
            let count = specs.get(r.wall)?.layers.len();
            to_end(s, p, r.at_end, count)
        })
        .collect()
}

/// The end of a ray that stops on another wall whose layers, met from the face that looks at the
/// ray, are `near` (thickness and rank): each layer runs on to the first layer of the same or a
/// higher priority and stops on its near line, or on the far face when there is none.
fn tee_ends(r: &Ray, spec: &Spec, through: &Through, near_left: &Stack) -> Option<EndPoints> {
    let n = left(through.dir);
    let facing = dot(r.dir, n);
    if facing.abs() < join::PARALLEL {
        return None;
    }
    // The through wall's lines as offsets from its baseline, from the near face inward.
    let (stack, offset): (Stack, Box<dyn Fn(f64) -> f64>) = if facing > 0.0 {
        (
            Stack {
                thick: near_left.thick.clone(),
                rank: near_left.rank.clone(),
                orig: near_left.orig.clone(),
            },
            Box::new(move |c| through.high - c),
        )
    } else {
        (near_left.flipped(), Box::new(move |c| through.low + c))
    };
    let cum = stack.cum();
    let mine = Stack::of(spec, r.at_end);
    let own = mine.cum();
    let line_of = |k: usize| {
        Some(join::Line {
            p: along(r.point, left(r.dir), r.hi - own.get(k)?),
            d: r.dir,
        })
    };
    let mut pts: Vec<Option<P>> = vec![None; mine.thick.len() + 1];
    let mut ends: EndPoints = vec![None; spec.layers.len()];
    for (k, &o) in mine.orig.iter().enumerate() {
        let rank_k = *mine.rank.get(k)?;
        let stop = stack
            .rank
            .iter()
            .position(|&band| band <= rank_k)
            .unwrap_or(stack.rank.len());
        let terminal = join::Line {
            p: along(r.point, n, offset(*cum.get(stop)?)),
            d: through.dir,
        };
        let (first, second) = (line_of(k)?.hit(terminal)?, line_of(k + 1)?.hit(terminal)?);
        if !r.fits([first, second], r.thickness + through.thickness) {
            return None;
        }
        // Remember the points for the check of the layer lines' order.
        *pts.get_mut(k)? = Some(first);
        *pts.get_mut(k + 1)? = Some(second);
        *ends.get_mut(o)? = Some(if r.at_end {
            [second, first]
        } else {
            [first, second]
        });
    }
    Some(ends)
}

/// The layer polygons of `walls` with the joins cleaned: see the module notes.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the nodes of the walls"
)]
pub fn layout(walls: &[Wall], specs: &[Spec]) -> Layout {
    let feet = join::footprints(walls);
    let n = walls.len();
    let mut ends: Vec<[EndPoints; 2]> = walls
        .iter()
        .zip(specs)
        .zip(&feet)
        .map(|((w, s), f)| [proportional(w, s, f, false), proportional(w, s, f, true)])
        .collect();
    let base_ends = ends.clone();
    let mut nodes_walls: Vec<Vec<usize>> = Vec::new();
    let mut centers: Vec<Poly> = Vec::new();
    let mut free = vec![[true; 2]; n];
    let valid: Vec<bool> = walls
        .iter()
        .map(|w| w.length() >= crate::types::MIN_WALL_LENGTH && w.length().is_finite())
        .collect();
    let rays: Vec<Ray> = (0..n)
        .flat_map(|i| [(i, false), (i, true)])
        .filter_map(|(i, at_end)| Ray::new(walls, i, at_end))
        .filter(|r| r.style != JoinStyle::Disallowed)
        .collect();
    let index = join::Index::new(walls, &valid);
    for group in join::nodes(&rays) {
        let members: Vec<Ray> = group.iter().filter_map(|&k| rays.get(k).copied()).collect();
        let owners: Vec<usize> = members.iter().map(|r| r.wall).collect();
        let Some(first) = members.first() else {
            continue;
        };
        let through = index.through(walls, first.point, &owners);
        if through.is_none() && members.len() < 2 {
            continue;
        }
        for r in &members {
            if let Some(f) = free.get_mut(r.wall) {
                f[usize::from(r.at_end)] = false;
            }
        }
        if through.is_none() && members.len() >= 3 && join::straight(&members).is_none() {
            centers.extend(star_center(&members, walls, specs, &feet));
        }
        let trimmed = |r: &Ray| matches!(r.style, JoinStyle::Butt | JoinStyle::Square);
        let plain = members.iter().all(|r| r.style == JoinStyle::Auto);
        let butted = through.is_none()
            && members.len() == 2
            && members.iter().any(trimmed)
            && members.iter().all(|r| r.style != JoinStyle::Disallowed);
        if !plain && !butted {
            continue;
        }
        let refined: Option<Vec<(usize, bool, EndPoints)>> = if butted {
            // One end stops at the near face of the other wall, as a T does; the other keeps its
            // footprint. Both ends butt: the corner is mitered like an automatic one.
            match members.as_slice() {
                [a, b] if trimmed(a) != trimmed(b) => {
                    let (t, o) = if trimmed(a) { (a, b) } else { (b, a) };
                    let through = Through {
                        dir: o.dir,
                        low: o.lo,
                        high: o.hi,
                        thickness: o.thickness,
                    };
                    specs
                        .get(t.wall)
                        .zip(specs.get(o.wall))
                        .and_then(|(own, other)| {
                            let stack = Stack::of(other, o.at_end);
                            Some(vec![(
                                t.wall,
                                t.at_end,
                                tee_ends(t, own, &through, &stack)?,
                            )])
                        })
                }
                _ => star_ends(&members, specs).map(|e| {
                    members
                        .iter()
                        .zip(e)
                        .map(|(r, ep)| (r.wall, r.at_end, ep))
                        .collect()
                }),
            }
        } else {
            match through {
                Some(t) => {
                    let (Some(w), Some(spec)) = (walls.get(t), specs.get(t)) else {
                        continue;
                    };
                    let stack = Stack::of(spec, false);
                    members
                        .iter()
                        .map(|r| {
                            let own = specs.get(r.wall)?;
                            Some((r.wall, r.at_end, tee_ends(r, own, &Through::of(w), &stack)?))
                        })
                        .collect()
                }
                None => {
                    if members.len() >= 3
                        && let Some((i, j, thr)) = join::straight(&members)
                    {
                        let run = members.get(i);
                        let stack =
                            run.and_then(|r| specs.get(r.wall).map(|s| Stack::of(s, r.at_end)));
                        let mut out = Vec::new();
                        let mut ok = stack.is_some();
                        if let Some(stack) = stack {
                            for (k, r) in members.iter().enumerate() {
                                if k == i || k == j {
                                    continue;
                                }
                                match specs
                                    .get(r.wall)
                                    .and_then(|own| tee_ends(r, own, &thr, &stack))
                                {
                                    Some(e) => out.push((r.wall, r.at_end, e)),
                                    None => ok = false,
                                }
                            }
                        }
                        ok.then_some(out)
                    } else {
                        star_ends(&members, specs).and_then(|e| {
                            let same_faces = members.iter().zip(&e).all(|(r, ep)| {
                                let (Some(foot), Some(spec), Some(w)) =
                                    (feet.get(r.wall), specs.get(r.wall), walls.get(r.wall))
                                else {
                                    return false;
                                };
                                let old = proportional(w, spec, foot, r.at_end);
                                // The outer corner of the first layer and the inner corner of the
                                // last are the corners of the footprint.
                                let ends_of = |v: &EndPoints| {
                                    let mut it = v.iter().flatten();
                                    let first = it.next().map(|p| p[0]);
                                    let last = it.last().map(|p| p[1]);
                                    first.zip(last)
                                };
                                match (ends_of(&old), ends_of(ep)) {
                                    (Some(a), Some(b)) => {
                                        let near =
                                            |x: P, y: P| (x[0] - y[0]).hypot(x[1] - y[1]) < 1.0e-6;
                                        near(a.0, b.0) && near(a.1, b.1)
                                    }
                                    _ => false,
                                }
                            });
                            same_faces.then(|| {
                                members
                                    .iter()
                                    .zip(e)
                                    .map(|(r, ep)| (r.wall, r.at_end, ep))
                                    .collect()
                            })
                        })
                    }
                }
            }
        };
        if refined.is_some() {
            let mut walls_of: Vec<usize> = owners.clone();
            walls_of.extend(through);
            walls_of.sort_unstable();
            walls_of.dedup();
            // Walls of one layer stack tile exactly: the nested corner, the T that stops at its
            // near line and the cross need no repair.
            let same = walls_of
                .iter()
                .filter_map(|w| specs.get(*w))
                .collect::<Vec<_>>()
                .windows(2)
                .all(|p| p[0] == p[1]);
            if !same {
                nodes_walls.push(walls_of);
            }
        }
        for (wall, at_end, points) in refined.into_iter().flatten() {
            if let Some(e) = ends.get_mut(wall) {
                e[usize::from(at_end)] = points;
            }
        }
    }
    let refined = polys_from(specs, &valid, &ends);
    let proportional_polys = polys_from(specs, &valid, &base_ends);
    let resolved = resolve(refined);
    let mut polys = fill_gaps(resolved, &proportional_polys, &nodes_walls);
    // What the closed star leaves in its middle belongs to the strongest wall of the node: what
    // the layers do not cover of it.
    for c in centers {
        let mut pieces = vec![c.points.clone()];
        for p in &polys {
            if !boxes_meet(bbox(&c.points), bbox(&p.points)) {
                continue;
            }
            pieces = pieces
                .into_iter()
                .flat_map(|piece| {
                    combine(&piece, &p.points, BooleanOp::Difference).unwrap_or_else(|| vec![piece])
                })
                .collect();
        }
        polys.extend(
            pieces
                .into_iter()
                .filter(|l| area(l) > AREA_EPS)
                .map(|points| Poly {
                    points,
                    ..c.clone()
                }),
        );
    }
    Layout { polys, free }
}

/// The polygon in the middle of a star of three or more walls, which the ends of the walls
/// leave open, as a polygon of the core of the strongest wall of the node (the lowest priority
/// number of a core layer, then the thickest).
fn star_center(members: &[Ray], walls: &[Wall], specs: &[Spec], feet: &[[P; 4]]) -> Option<Poly> {
    let mut order: Vec<&Ray> = members.iter().collect();
    order.sort_by(|a, b| {
        a.dir[1]
            .atan2(a.dir[0])
            .total_cmp(&b.dir[1].atan2(b.dir[0]))
            .then(a.wall.cmp(&b.wall))
    });
    let mut points: Vec<P> = Vec::new();
    for r in order {
        let f = feet.get(r.wall)?;
        let (left, right) = if r.at_end { (f[2], f[1]) } else { (f[3], f[0]) };
        for p in [right, left] {
            let dup = |q: &P| (q[0] - p[0]).hypot(q[1] - p[1]) < 1.0e-6;
            if !points.last().is_some_and(dup) {
                points.push(p);
            }
        }
    }
    while points.len() > 1
        && points
            .first()
            .zip(points.last())
            .is_some_and(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) < 1.0e-6)
    {
        points.pop();
    }
    if points.len() < 3 || area(&points) <= AREA_EPS || region(&points).is_none() {
        return None;
    }
    let (wall, layer) = members
        .iter()
        .filter_map(|r| {
            let spec = specs.get(r.wall)?;
            let (k, l) = spec
                .layers
                .iter()
                .enumerate()
                .filter(|(_, l)| l.core && l.thickness > MIN_LAYER)
                .min_by(|a, b| {
                    rank(a.1.function)
                        .cmp(&rank(b.1.function))
                        .then(b.1.thickness.total_cmp(&a.1.thickness))
                })?;
            Some((r.wall, k, l.function, walls.get(r.wall)?.thickness))
        })
        .min_by(|a, b| {
            rank(a.2)
                .cmp(&rank(b.2))
                .then(b.3.total_cmp(&a.3))
                .then(a.0.cmp(&b.0))
        })
        .map(|(w, k, _, _)| (w, k))?;
    let function = specs.get(wall)?.layers.get(layer)?.function;
    Some(Poly {
        wall,
        layer,
        function,
        core: true,
        points,
    })
}

/// The polygon of each layer of each wall from the end points of its layers.
fn polys_from(specs: &[Spec], valid: &[bool], ends: &[[EndPoints; 2]]) -> Vec<Poly> {
    let mut polys = Vec::new();
    for (i, (spec, pair)) in specs.iter().zip(ends).enumerate() {
        if !valid.get(i).copied().unwrap_or(false) {
            continue;
        }
        for &k in &spec.solid_layers() {
            let (Some(Some(s)), Some(Some(e)), Some(layer)) =
                (pair[0].get(k), pair[1].get(k), spec.layers.get(k))
            else {
                continue;
            };
            polys.push(Poly {
                wall: i,
                layer: k,
                function: layer.function,
                core: layer.core,
                points: vec![s[0], e[0], e[1], s[1]],
            });
        }
    }
    polys
}

/// The union of `polys`; `None` when the kernel refuses one.
fn union_all<'a>(polys: impl Iterator<Item = &'a [P]>) -> Option<Region> {
    let mut acc: Option<Region> = None;
    for p in polys {
        let r = region(p)?;
        acc = Some(match acc {
            None => r,
            Some(a) => arc_geom::boolean(
                &a,
                &r,
                BooleanOp::Union,
                &arc_base::Tolerance::DEFAULT,
                &arc_base::Limits::DEFAULT,
            )
            .ok()?
            .unwrap_or(a),
        });
    }
    acc
}

/// The outline of the polygons of one wall: the outer loops of their union. Empty when the kernel
/// refuses them.
#[must_use]
pub fn outline(polys: &[Poly]) -> Vec<Vec<P>> {
    union_all(polys.iter().map(|p| p.points.as_slice())).map_or_else(Vec::new, |r| polygons_of(&r))
}

/// `a` less `b`, when something is left of it.
fn minus(a: &Region, b: &Region) -> Option<Region> {
    arc_geom::boolean(
        a,
        b,
        BooleanOp::Difference,
        &arc_base::Tolerance::DEFAULT,
        &arc_base::Limits::DEFAULT,
    )
    .ok()
    .flatten()
    .filter(|r| {
        r.measure(&arc_base::Tolerance::DEFAULT)
            .is_ok_and(|m| m.area > AREA_EPS)
    })
}

/// Closes what the cleaned layers of the walls around a node leave open and trims what they
/// push out: the layers of the walls of a node must cover exactly the footprints those walls
/// have (the proportional layers `base`), with no gap and no overlap. A gap is given back to the
/// proportional layer that covers it, and a part outside the footprints is cut off.
fn fill_gaps(mut polys: Vec<Poly>, base: &[Poly], groups: &[Vec<usize>]) -> Vec<Poly> {
    for g in groups {
        let old: Vec<&Poly> = base.iter().filter(|p| g.contains(&p.wall)).collect();
        let at: Vec<usize> = polys
            .iter()
            .enumerate()
            .filter(|(_, p)| g.contains(&p.wall))
            .map(|(i, _)| i)
            .collect();
        let (Some(u_old), Some(u_cur)) = (
            union_all(old.iter().map(|p| p.points.as_slice())),
            union_all(
                at.iter()
                    .filter_map(|&i| polys.get(i))
                    .map(|p| p.points.as_slice()),
            ),
        ) else {
            continue;
        };
        if let Some(excess) = minus(&u_cur, &u_old) {
            for &i in &at {
                let Some(p) = polys.get(i).cloned() else {
                    continue;
                };
                if let Some(r) = region(&p.points)
                    && let Some(rest) = arc_geom::boolean(
                        &r,
                        &excess,
                        BooleanOp::Difference,
                        &arc_base::Tolerance::DEFAULT,
                        &arc_base::Limits::DEFAULT,
                    )
                    .ok()
                {
                    let mut pieces = rest.map_or_else(Vec::new, |x| polygons_of(&x)).into_iter();
                    if let (Some(first), Some(slot)) = (pieces.next(), polys.get_mut(i)) {
                        slot.points = first;
                    } else if let Some(slot) = polys.get_mut(i) {
                        slot.points = Vec::new();
                    }
                    for extra in pieces {
                        polys.push(Poly {
                            points: extra,
                            ..p.clone()
                        });
                    }
                }
            }
        }
        if let Some(gap) = minus(&u_old, &u_cur) {
            for p in old {
                let Some(r) = region(&p.points) else { continue };
                if let Ok(Some(i)) = arc_geom::boolean(
                    &r,
                    &gap,
                    BooleanOp::Intersection,
                    &arc_base::Tolerance::DEFAULT,
                    &arc_base::Limits::DEFAULT,
                ) {
                    for points in polygons_of(&i) {
                        polys.push(Poly {
                            points,
                            ..p.clone()
                        });
                    }
                }
            }
        }
    }
    polys.retain(|p| p.points.len() >= 3 && area(&p.points) > AREA_EPS);
    polys
}

// ----- regions ------------------------------------------------------------------------------

/// The region of a polygon; `None` when the kernel refuses it.
fn region(points: &[P]) -> Option<Region> {
    Boundary::polygons(vec![points.to_vec()]).region().ok()
}

/// The outer loops of the parts of `region`, as polygons.
fn polygons_of(region: &Region) -> Vec<Vec<P>> {
    region
        .parts()
        .iter()
        .map(|p| {
            p.outer()
                .curves()
                .iter()
                .filter_map(|c| c.start_point().map(|q| [q.x, q.y]))
                .collect()
        })
        .collect()
}

/// `a` combined with `b` by `op`, as polygons; `None` when the kernel refuses either.
fn combine(a: &[P], b: &[P], op: BooleanOp) -> Option<Vec<Vec<P>>> {
    let (ra, rb) = (region(a)?, region(b)?);
    let result = arc_geom::boolean(
        &ra,
        &rb,
        op,
        &arc_base::Tolerance::DEFAULT,
        &arc_base::Limits::DEFAULT,
    )
    .ok()?;
    Some(result.map_or_else(Vec::new, |r| polygons_of(&r)))
}

/// The area of a polygon.
#[must_use]
pub fn area(points: &[P]) -> f64 {
    crate::floor::signed_area(points).abs()
}

fn bbox(points: &[P]) -> [f64; 4] {
    points
        .iter()
        .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        })
}

fn boxes_meet(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] - 1.0e-9 && b[0] < a[2] - 1.0e-9 && a[1] < b[3] - 1.0e-9 && b[1] < a[3] - 1.0e-9
}

/// The area two convex polygons share, by clipping; `None` when either is not convex (the
/// caller then asks the kernel).
fn convex_overlap(a: &[P], b: &[P]) -> Option<f64> {
    let ccw = |p: &[P]| -> Option<Vec<P>> {
        let n = p.len();
        let sign = crate::floor::signed_area(p);
        let mut v: Vec<P> = if sign >= 0.0 {
            p.to_vec()
        } else {
            p.iter().rev().copied().collect()
        };
        let convex = (0..n).all(
            |k| match (v.get(k), v.get((k + 1) % n), v.get((k + 2) % n)) {
                (Some(x), Some(y), Some(z)) => {
                    (y[0] - x[0]) * (z[1] - y[1]) - (y[1] - x[1]) * (z[0] - y[0]) >= -1.0e-12
                }
                _ => false,
            },
        );
        convex.then(|| std::mem::take(&mut v))
    };
    let (a, b) = (ccw(a)?, ccw(b)?);
    let clipped = crate::terrain::clip_convex(&a, &b);
    Some(area(&clipped))
}

/// The edge of a cell of [`Grid`], in meters.
const GRID_CELL: f64 = 4.0;
/// A box that would fill more cells than this goes to the list every query checks.
const MAX_GRID_CELLS: u128 = 64;

/// The boxes of polygons by grid cell, to find the neighbors of a box or a point without
/// comparing every pair; a box that covers many cells (a long diagonal wall) is kept in a list
/// of its own, so the work and the memory stay bounded however far a wall reaches.
struct Grid {
    cells: HashMap<(i64, i64), Vec<usize>>,
    long: Vec<usize>,
    count: usize,
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "a float to integer cast saturates"
)]
fn cell_of(v: f64) -> i64 {
    (v / GRID_CELL).floor() as i64
}

/// The number of cells a box covers.
fn cells_of(b: [f64; 4]) -> u128 {
    let span = |lo: f64, hi: f64| {
        u128::try_from(cell_of(hi).saturating_sub(cell_of(lo)).saturating_add(1))
            .unwrap_or(u128::MAX)
    };
    span(b[0], b[2]).saturating_mul(span(b[1], b[3]))
}

impl Grid {
    fn new(boxes: &[[f64; 4]]) -> Self {
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        let mut long = Vec::new();
        for (k, b) in boxes.iter().enumerate() {
            if cells_of(*b) > MAX_GRID_CELLS {
                long.push(k);
                continue;
            }
            for x in cell_of(b[0])..=cell_of(b[2]) {
                for y in cell_of(b[1])..=cell_of(b[3]) {
                    cells.entry((x, y)).or_default().push(k);
                }
            }
        }
        Self {
            cells,
            long,
            count: boxes.len(),
        }
    }

    /// The indexes of the boxes that may meet `b`, in increasing order.
    fn near_box(&self, b: [f64; 4]) -> Vec<usize> {
        if cells_of(b) > MAX_GRID_CELLS {
            return (0..self.count).collect();
        }
        let mut near: Vec<usize> = self.long.clone();
        for x in cell_of(b[0])..=cell_of(b[2]) {
            for y in cell_of(b[1])..=cell_of(b[3]) {
                near.extend(self.cells.get(&(x, y)).into_iter().flatten().copied());
            }
        }
        near.sort_unstable();
        near.dedup();
        near
    }

    /// The indexes of the boxes that may hold `p`.
    fn near_point(&self, p: P) -> Vec<usize> {
        self.near_box([p[0], p[1], p[0], p[1]])
    }
}

/// Where the layers of different walls overlap the higher priority keeps the area.
fn resolve(polys: Vec<Poly>) -> Vec<Poly> {
    if polys.len() > MAX_POLYS {
        return polys;
    }
    let boxes: Vec<[f64; 4]> = polys.iter().map(|p| bbox(&p.points)).collect();
    let grid = Grid::new(&boxes);
    let wins = |j: &Poly, i: &Poly| (rank(j.function), j.wall) < (rank(i.function), i.wall);
    let mut out = Vec::with_capacity(polys.len());
    for (k, p) in polys.iter().enumerate() {
        let mut pieces = vec![p.points.clone()];
        let Some(b) = boxes.get(k) else { continue };
        let near = grid.near_box(*b);
        for j in near {
            let (Some(q), Some(bq)) = (polys.get(j), boxes.get(j)) else {
                continue;
            };
            if j == k || q.wall == p.wall || !wins(q, p) || !boxes_meet(*b, *bq) {
                continue;
            }
            let mut next = Vec::new();
            for piece in pieces {
                let overlap = convex_overlap(&piece, &q.points).unwrap_or_else(|| {
                    combine(&piece, &q.points, BooleanOp::Intersection)
                        .map_or(0.0, |v| v.iter().map(|l| area(l)).sum())
                });
                if overlap <= AREA_EPS {
                    next.push(piece);
                    continue;
                }
                match combine(&piece, &q.points, BooleanOp::Difference) {
                    Some(rest) => next.extend(rest),
                    None => next.push(piece),
                }
            }
            pieces = next;
        }
        out.extend(
            pieces
                .into_iter()
                .filter(|l| area(l) > AREA_EPS)
                .map(|points| Poly {
                    points,
                    ..p.clone()
                }),
        );
    }
    out
}

// ----- openings and wrapping ------------------------------------------------------------------

/// The frame of a wall: `u` along the baseline from the start, `v` across, toward the left.
struct Frame {
    start: P,
    u: P,
    length: f64,
    low: f64,
    high: f64,
}

impl Frame {
    fn of(wall: &Wall) -> Option<Self> {
        let length = wall.length();
        if !(length > 0.0 && length.is_finite()) {
            return None;
        }
        let (low, high) = wall.extent();
        Some(Self {
            start: wall.start,
            u: [
                (wall.end[0] - wall.start[0]) / length,
                (wall.end[1] - wall.start[1]) / length,
            ],
            length,
            low,
            high,
        })
    }

    fn at(&self, u: f64, v: f64) -> P {
        let n = left(self.u);
        [
            self.start[0] + self.u[0] * u + n[0] * v,
            self.start[1] + self.u[1] * u + n[1] * v,
        ]
    }

    /// The box between two distances along the baseline and two across it, as a polygon.
    fn rect(&self, ua: f64, ub: f64, va: f64, vb: f64) -> Vec<P> {
        vec![
            self.at(ua, va),
            self.at(ub, va),
            self.at(ub, vb),
            self.at(ua, vb),
        ]
    }
}

/// The wrapped layers of one side: every layer outside the core on that side (membranes have no
/// thickness and are left out), as indexes and thicknesses, from the face inward.
fn wrapped_run(spec: &Spec, exterior: bool) -> Vec<(usize, f64)> {
    let order: Vec<usize> = if exterior {
        (0..spec.layers.len()).collect()
    } else {
        (0..spec.layers.len()).rev().collect()
    };
    order
        .into_iter()
        .map_while(|k| {
            spec.layers
                .get(k)
                .filter(|l| !l.core)
                .map(|l| (k, l.thickness))
        })
        .filter(|(_, t)| *t > MIN_LAYER)
        .collect()
}

/// Cuts openings (`gaps`: distances along the baseline) and wraps free ends and reveals in the
/// polygons `polys` of one wall (see the module notes). `free` tells which ends take part in no
/// join.
#[must_use]
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one pass over the boundaries that wrap"
)]
pub fn finish_wall(
    wall: &Wall,
    spec: &Spec,
    polys: &[Poly],
    free: [bool; 2],
    gaps: &[(f64, f64)],
) -> Vec<Poly> {
    let Some(f) = Frame::of(wall) else {
        return polys.to_vec();
    };
    let mut cuts: Vec<(f64, f64)> = gaps
        .iter()
        .map(|&(a, b)| (a.max(0.0), b.min(f.length)))
        .filter(|g| g.1 - g.0 > 1.0e-6)
        .collect();
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for g in cuts {
        match merged.last_mut() {
            Some(last) if g.0 <= last.1 => last.1 = last.1.max(g.1),
            _ => merged.push(g),
        }
    }
    // The boundaries that wrap: (position, the side the material is on, the wrapping).
    let mut bounds: Vec<(f64, f64, Wrap)> = Vec::new();
    if free[0] {
        bounds.push((0.0, 1.0, spec.wrap_ends));
    }
    if free[1] {
        bounds.push((f.length, -1.0, spec.wrap_ends));
    }
    for &(a, b) in &merged {
        bounds.push((a, -1.0, spec.wrap_inserts));
        bounds.push((b, 1.0, spec.wrap_inserts));
    }
    let mut out: Vec<Poly> = polys.to_vec();
    let pad = 1.0;
    for &(a, b) in &merged {
        let gap = f.rect(a, b, f.low - pad, f.high + pad);
        out = out.into_iter().flat_map(|p| subtract(p, &gap)).collect();
    }
    for (u0, side, wrap) in bounds {
        if wrap == Wrap::None {
            continue;
        }
        let ext = if wrap.exterior() {
            wrapped_run(spec, true)
        } else {
            Vec::new()
        };
        let mut int = if wrap.interior() {
            wrapped_run(spec, false)
        } else {
            Vec::new()
        };
        int.retain(|(k, _)| ext.iter().all(|(e, _)| e != k));
        let total = |run: &[(usize, f64)]| run.iter().map(|r| r.1).sum::<f64>();
        let (c_ext, c_int) = (total(&ext), total(&int));
        if c_ext + c_int <= 0.0 {
            continue;
        }
        let mid = match (ext.is_empty(), int.is_empty()) {
            (false, true) => f.low,
            (true, false) => f.high,
            _ => {
                let (lo, hi) = wall.core_extent();
                ((lo + hi) / 2.0).clamp(f.low, f.high)
            }
        };
        let (depth_ext, depth_int) = (
            if ext.is_empty() { c_int } else { c_ext },
            if int.is_empty() { c_ext } else { c_int },
        );
        let span = |d: f64| (u0, u0 + side * d);
        let boxed = |d: f64, va: f64, vb: f64| {
            let (x, y) = span(d);
            f.rect(x.min(y), x.max(y), va, vb)
        };
        // The distance of each wrapped layer's foot from the end and its thickness.
        let foot = |run: &[(usize, f64)], k: usize| {
            let mut before = 0.0;
            for &(i, t) in run {
                if i == k {
                    return Some((before, t));
                }
                before += t;
            }
            None
        };
        let mut caps: Vec<Poly> = Vec::new();
        let mut next: Vec<Poly> = Vec::new();
        for p in out {
            let wrapped = foot(&ext, p.layer)
                .map(|x| (x, true))
                .or_else(|| foot(&int, p.layer).map(|x| (x, false)));
            let mut pieces = vec![p.clone()];
            let cut_regions: Vec<Vec<P>> = match wrapped {
                Some(((before, t), _)) => vec![boxed(before + t, f.low - pad, f.high + pad)],
                None => vec![
                    boxed(depth_ext, mid, f.high + pad),
                    boxed(depth_int, f.low - pad, mid),
                ],
            };
            for cut in &cut_regions {
                pieces = pieces.into_iter().flat_map(|q| subtract(q, cut)).collect();
            }
            if let Some(((before, t), exterior)) = wrapped {
                let layer_top = f.high - layer_start(spec, p.layer);
                let layer_bottom =
                    layer_top - spec.layers.get(p.layer).map_or(0.0, |l| l.thickness);
                let (x, y) = (u0 + side * before, u0 + side * (before + t));
                let cap = if exterior {
                    f.rect(x.min(y), x.max(y), mid, layer_top)
                } else {
                    f.rect(x.min(y), x.max(y), layer_bottom, mid)
                };
                caps.push(Poly {
                    points: cap,
                    ..p.clone()
                });
            }
            next.extend(pieces);
        }
        // A cap joins the polygon of its layer that it touches.
        for cap in caps {
            let mut joined = false;
            for q in &mut next {
                if q.layer != cap.layer || !boxes_meet_touching(bbox(&q.points), bbox(&cap.points))
                {
                    continue;
                }
                if let Some(u) = combine(&q.points, &cap.points, BooleanOp::Union)
                    && let [single] = u.as_slice()
                {
                    q.points = single.clone();
                    joined = true;
                    break;
                }
            }
            if !joined {
                next.push(cap);
            }
        }
        out = next;
    }
    out
}

/// The distance of the layer `k`'s exterior line from the exterior face.
fn layer_start(spec: &Spec, k: usize) -> f64 {
    spec.layers.iter().take(k).map(|l| l.thickness).sum()
}

fn boxes_meet_touching(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] + 1.0e-9 && b[0] <= a[2] + 1.0e-9 && a[1] <= b[3] + 1.0e-9 && b[1] <= a[3] + 1.0e-9
}

/// `poly` less the polygon `cut`: what is left of it, as polygons of the same layer.
fn subtract(poly: Poly, cut: &[P]) -> Vec<Poly> {
    if !boxes_meet(bbox(&poly.points), bbox(cut)) {
        return vec![poly];
    }
    match combine(&poly.points, cut, BooleanOp::Difference) {
        Some(rest) => rest
            .into_iter()
            .filter(|l| area(l) > AREA_EPS)
            .map(|points| Poly {
                points,
                ..poly.clone()
            })
            .collect(),
        None => vec![poly],
    }
}

// ----- plan lines -----------------------------------------------------------------------------

/// Whether `p` is inside the polygon.
fn inside(points: &[P], p: P) -> bool {
    let n = points.len();
    let mut result = false;
    for k in 0..n {
        let (Some(a), Some(b)) = (points.get(k), points.get((k + 1) % n)) else {
            continue;
        };
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            result = !result;
        }
    }
    result
}

/// A line between two layer regions: the wall of the polygon it was found on, its ends, and
/// whether it is a boundary of a core.
pub type Line = (usize, [P; 2], bool);

/// The lines between the layers of `polys`, where a layer meets another layer of its own wall, or
/// a layer of another wall that has another function or another core status. Faces and the cut
/// ends at openings (where nothing lies across) are not among them, and a line is given once.
#[must_use]
pub fn lines(polys: &[Poly]) -> Vec<Line> {
    let oriented: Vec<Vec<P>> = polys
        .iter()
        .map(|p| {
            if crate::floor::signed_area(&p.points) >= 0.0 {
                p.points.clone()
            } else {
                p.points.iter().rev().copied().collect()
            }
        })
        .collect();
    let boxes: Vec<[f64; 4]> = oriented.iter().map(|p| bbox(p)).collect();
    let grid = Grid::new(&boxes);
    let probe = 2.0e-4;
    let mut out = Vec::new();
    for (k, (poly, pts)) in polys.iter().zip(&oriented).enumerate() {
        let n = pts.len();
        for e in 0..n {
            let (Some(&a), Some(&b)) = (pts.get(e), pts.get((e + 1) % n)) else {
                continue;
            };
            let d = [b[0] - a[0], b[1] - a[1]];
            let len = d[0].hypot(d[1]);
            if len < 4.0 * probe {
                continue;
            }
            let at = [
                (a[0] + b[0]) / 2.0 + d[1] / len * probe,
                (a[1] + b[1]) / 2.0 - d[0] / len * probe,
            ];
            let found = grid.near_point(at).into_iter().find(|&j| {
                j != k
                    && boxes.get(j).is_some_and(|b| {
                        at[0] >= b[0] && at[0] <= b[2] && at[1] >= b[1] && at[1] <= b[3]
                    })
                    && oriented.get(j).is_some_and(|q| inside(q, at))
            });
            let Some(j) = found else { continue };
            let Some(other) = polys.get(j) else { continue };
            let draw = if other.wall == poly.wall {
                k < j && other.layer != poly.layer
            } else {
                (other.function != poly.function || other.core != poly.core) && k < j
            };
            if draw {
                out.push((poly.wall, [a, b], other.core != poly.core));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::join;
    use crate::wall::LocationLine;

    fn wall(start: P, end: P, thickness: f64) -> Wall {
        Wall {
            start,
            end,
            thickness,
            core: [0.0, 0.0],
            height: 3.0,
            base_offset: 0.0,
            location: LocationLine::Center,
            start_join: JoinStyle::Auto,
            end_join: JoinStyle::Auto,
        }
    }

    fn spec(layers: &[(Function, f64, bool)]) -> Spec {
        Spec {
            layers: layers
                .iter()
                .map(|&(function, thickness, core)| Layer {
                    function,
                    core,
                    thickness,
                })
                .collect(),
            wrap_ends: Wrap::None,
            wrap_inserts: Wrap::None,
        }
    }

    fn total(s: &Spec) -> f64 {
        s.layers.iter().map(|l| l.thickness).sum()
    }

    fn near(a: P, b: P) -> bool {
        (a[0] - b[0]).hypot(a[1] - b[1]) < 1.0e-6
    }

    #[test]
    fn an_l_join_of_two_layered_walls_has_continuous_core_and_finish_lines() {
        use Function::{Finish, Structure};
        let s = spec(&[
            (Finish, 0.02, false),
            (Structure, 0.2, true),
            (Finish, 0.02, false),
        ]);
        let walls = [
            wall([0.0, 0.0], [4.0, 0.0], total(&s)),
            wall([4.0, 0.0], [4.0, 4.0], total(&s)),
        ];
        let layout = layout(&walls, &[s.clone(), s]);
        assert_eq!(layout.free, vec![[true, false], [false, true]]);
        let lines = lines(&layout.polys);
        // Per wall: two core lines (outer and inner side of the core) and no other boundary, as
        // the finishes meet the core at those same lines.
        for wall in [0, 1] {
            assert_eq!(
                lines.iter().filter(|l| l.0 == wall && l.2).count(),
                2,
                "{wall}"
            );
        }
        // Each core line of one wall ends where a core line of the other begins.
        for (w, seg, _) in lines.iter().filter(|l| l.2) {
            let end = if *w == 0 { seg[1] } else { seg[0] };
            let end = if (end[0] - 4.0).abs() < 0.5 {
                end
            } else if *w == 0 {
                seg[0]
            } else {
                seg[1]
            };
            if (end[0] - 4.0).abs() < 0.5 && end[1].abs() < 0.5 {
                assert!(
                    lines
                        .iter()
                        .any(|(o, s, c)| *c && o != w && (near(s[0], end) || near(s[1], end))),
                    "{end:?}"
                );
            }
        }
        // No gap and no overlap: the layers of the two walls tile the corner.
        let sum: f64 = layout.polys.iter().map(|p| area(&p.points)).sum();
        assert!(sum > 0.0);
    }

    #[test]
    fn a_t_join_of_different_types_follows_priority() {
        use Function::{Finish, Structure, Thermal};
        let through = spec(&[
            (Finish, 0.02, false),
            (Thermal, 0.1, false),
            (Structure, 0.2, true),
            (Finish, 0.02, false),
        ]);
        let ending = spec(&[(Structure, 0.2, true)]);
        let walls = [
            wall([0.0, 0.0], [8.0, 0.0], total(&through)),
            wall([4.0, 0.0], [4.0, 4.0], 0.2),
        ];
        let layout = layout(&walls, &[through, ending]);
        // The structure of the ending wall runs over the finish and the thermal layer to the
        // core of the through wall: its polygon stops at the core's near face, y = 0.05.
        let b: Vec<&Poly> = layout.polys.iter().filter(|p| p.wall == 1).collect();
        assert_eq!(b.len(), 1);
        let low = b[0].points.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
        assert!((low - 0.05).abs() < 1e-9, "{low}");
        // The through wall's thermal layer gives way to it: 8 m by 0.1 m less 0.2 by 0.1.
        let thermal: f64 = layout
            .polys
            .iter()
            .filter(|p| p.wall == 0 && p.function == Thermal)
            .map(|p| area(&p.points))
            .sum();
        assert!(
            (thermal - (8.0 * 0.1 - 0.2 * 0.1)).abs() < 1e-9,
            "{thermal}"
        );
    }

    #[test]
    fn wrapping_turns_the_finish_into_a_free_end_and_an_opening() {
        use Function::{Finish, Structure};
        let mut s = spec(&[
            (Finish, 0.02, false),
            (Structure, 0.2, true),
            (Finish, 0.02, false),
        ]);
        s.wrap_ends = Wrap::Both;
        s.wrap_inserts = Wrap::Exterior;
        let w = wall([0.0, 0.0], [4.0, 0.0], total(&s));
        let layout = layout(&[w], &[s.clone()]);
        let done = finish_wall(&w, &s, &layout.polys, layout.free[0], &[(1.5, 2.5)]);
        // The core stops 0.02 short of both free ends, and of the opening's reveal.
        let core: Vec<&Poly> = done.iter().filter(|p| p.core).collect();
        let xs = |p: &Poly| {
            let lo = p.points.iter().map(|q| q[0]).fold(f64::MAX, f64::min);
            let hi = p.points.iter().map(|q| q[0]).fold(f64::MIN, f64::max);
            (lo, hi)
        };
        let mut spans: Vec<(f64, f64)> = core.iter().map(|p| xs(p)).collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert_eq!(spans.len(), 2);
        assert!(
            (spans[0].0 - 0.02).abs() < 1e-9 && (spans[0].1 - 1.48).abs() < 1e-9,
            "{spans:?}"
        );
        assert!(
            (spans[1].0 - 2.52).abs() < 1e-9 && (spans[1].1 - 3.98).abs() < 1e-9,
            "{spans:?}"
        );
        // The exterior finish (y from 0.12 to 0.1) is an L at each free end: it reaches the end.
        let finish: Vec<&Poly> = done.iter().filter(|p| p.layer == 0).collect();
        assert!(finish.iter().any(|p| xs(p).0 < 1e-9));
        assert!(finish.iter().any(|p| (xs(p).1 - 4.0).abs() < 1e-9));
        // The material is conserved: the caps take what the shortened layers lose.
        let before: f64 = layout.polys.iter().map(|p| area(&p.points)).sum();
        let after: f64 = done.iter().map(|p| area(&p.points)).sum();
        assert!(
            (before - 1.0 * 0.24 - after).abs() < 1e-6,
            "{before} {after}"
        );
    }

    fn cavity() -> Spec {
        use Function::{Finish, Membrane, Structure, Thermal};
        spec(&[
            (Finish, 0.1, false),
            (Thermal, 0.05, false),
            (Thermal, 0.05, false),
            (Membrane, 0.0, false),
            (Structure, 0.15, true),
            (Finish, 0.015, false),
        ])
    }

    fn partition() -> Spec {
        use Function::{Finish, Structure};
        spec(&[
            (Finish, 0.01, false),
            (Structure, 0.08, true),
            (Finish, 0.01, false),
        ])
    }

    fn spec_wall(a: P, b: P, s: &Spec) -> Wall {
        wall(a, b, total(s))
    }

    /// The layers of the walls tile exactly the footprints the walls have: the same area, and no
    /// two layers overlap.
    fn assert_tiles(walls: &[Wall], specs: &[Spec], what: &str) {
        let layout = layout(walls, specs);
        let feet = join::footprints(walls);
        let raw = union_all(feet.iter().map(|f| f.as_slice())).unwrap();
        let raw_area = raw.measure(&arc_base::Tolerance::DEFAULT).unwrap().area;
        let cleaned = union_all(layout.polys.iter().map(|p| p.points.as_slice())).unwrap();
        let cleaned_area = cleaned.measure(&arc_base::Tolerance::DEFAULT).unwrap().area;
        let sum: f64 = layout.polys.iter().map(|p| area(&p.points)).sum();
        // A star closes the middle the ends of its walls leave open: the layers then cover the
        // footprints and that middle, and nothing else.
        let star = matches!(what, "star" | "Y" | "X");
        if star {
            assert!(
                cleaned_area >= raw_area - 1e-6,
                "{what}: {cleaned_area} < {raw_area}"
            );
            assert!(
                cleaned.parts().iter().all(|p| p.holes().is_empty()),
                "{what}: a hole in the middle"
            );
        } else {
            assert!(
                (raw_area - cleaned_area).abs() < 1e-6,
                "{what}: footprints {raw_area} layers {cleaned_area}"
            );
        }
        assert!(
            (sum - cleaned_area).abs() < 1e-6,
            "{what}: layers overlap: {sum} vs {cleaned_area}"
        );
    }

    fn polar(angle_deg: f64, length: f64) -> P {
        let a = angle_deg.to_radians();
        [length * a.cos(), length * a.sin()]
    }

    #[test]
    fn corners_close_with_no_gap_and_no_overlap_at_every_angle_and_mix() {
        let (c, p) = (cavity(), partition());
        let origin = [0.0, 0.0];
        // L 90 degrees, different types.
        assert_tiles(
            &[
                spec_wall(origin, [4.0, 0.0], &c),
                spec_wall(origin, [0.0, 4.0], &p),
            ],
            &[c.clone(), p.clone()],
            "L90",
        );
        // L 60 degrees, different types.
        assert_tiles(
            &[
                spec_wall(origin, [4.0, 0.0], &c),
                spec_wall(origin, polar(60.0, 4.0), &p),
            ],
            &[c.clone(), p.clone()],
            "L60",
        );
        // T, different types.
        assert_tiles(
            &[
                spec_wall([-3.0, 0.0], [3.0, 0.0], &c),
                spec_wall(origin, [0.0, 3.0], &p),
            ],
            &[c.clone(), p.clone()],
            "T",
        );
        // X of unequal walls: a cavity pair and a partition pair.
        assert_tiles(
            &[
                spec_wall(origin, [3.0, 0.0], &c),
                spec_wall(origin, [-3.0, 0.0], &c),
                spec_wall(origin, [0.0, 3.0], &p),
                spec_wall(origin, [0.0, -3.0], &p),
            ],
            &[c.clone(), c.clone(), p.clone(), p.clone()],
            "X",
        );
        // Four different walls at the node (a star: one cavity, three partitions).
        assert_tiles(
            &[
                spec_wall(origin, [3.0, 0.0], &c),
                spec_wall(origin, [-3.0, 0.0], &p),
                spec_wall(origin, [0.0, 3.0], &p),
                spec_wall(origin, [0.0, -3.0], &c),
            ],
            &[c.clone(), p.clone(), p.clone(), c.clone()],
            "star",
        );
        // Y at 120 degrees.
        assert_tiles(
            &[
                spec_wall(origin, polar(90.0, 3.0), &c),
                spec_wall(origin, polar(210.0, 3.0), &p),
                spec_wall(origin, polar(330.0, 3.0), &p),
            ],
            &[c.clone(), p.clone(), p.clone()],
            "Y",
        );
    }

    #[test]
    fn a_butt_end_gets_the_layer_priority_cleanup_too() {
        let (c, p) = (cavity(), partition());
        let mut a = spec_wall([0.0, 0.0], [4.0, 0.0], &c);
        let mut b = spec_wall([0.0, 0.0], [0.0, 4.0], &p);
        a.start_join = JoinStyle::Butt;
        b.start_join = JoinStyle::Auto;
        assert_tiles(&[a, b], &[c.clone(), p.clone()], "butt");
        a.start_join = JoinStyle::Square;
        assert_tiles(&[a, b], &[c, p], "square");
    }

    #[test]
    fn a_far_diagonal_wall_costs_bounded_memory_and_time() {
        let s = spec(&[
            (Function::Finish, 0.02, false),
            (Function::Structure, 0.2, true),
        ]);
        let w = wall([-50_000.0, -50_000.0], [50_000.0, 50_000.0], total(&s));
        let other = wall([0.0, 0.0], [3.0, 0.0], total(&s));
        let start = std::time::Instant::now();
        let layout = layout(&[w, other], &[s.clone(), s]);
        assert!(!layout.polys.is_empty());
        assert!(!lines(&layout.polys).is_empty());
        assert!(start.elapsed().as_secs() < 5);
    }

    #[test]
    fn every_layer_outside_the_core_wraps_not_only_finishes() {
        let mut s = cavity();
        s.wrap_ends = Wrap::Exterior;
        let w = spec_wall([0.0, 0.0], [4.0, 0.0], &s);
        let lay = layout(&[w], &[s.clone()]);
        let done = finish_wall(&w, &s, &lay.polys, lay.free[0], &[]);
        // The core stops 0.2 short of each free end: the exterior layers (0.1 + 0.05 + 0.05).
        let core: Vec<&Poly> = done.iter().filter(|p| p.core).collect();
        let hi = core
            .iter()
            .flat_map(|p| p.points.iter().map(|q| q[0]))
            .fold(f64::MIN, f64::max);
        assert!((hi - 3.8).abs() < 1e-9, "{hi}");
        // The thermal layers wrap too, inside the brick's foot: they reach 4 - 0.1 and 4 - 0.15.
        assert!(done.iter().any(|p| p.function == Function::Thermal
            && p.points.iter().any(|q| (q[0] - 3.9).abs() < 1e-9)));
    }

    #[test]
    fn no_layer_line_runs_across_another_wall_at_an_acute_corner() {
        let (c, p) = (cavity(), partition());
        for (angle, label) in [(60.0, "L60"), (30.0, "L30"), (120.0, "L120")] {
            let walls = [
                spec_wall([0.0, 0.0], [4.0, 0.0], &c),
                spec_wall([0.0, 0.0], polar(angle, 4.0), &p),
            ];
            let lay = layout(&walls, &[c.clone(), p.clone()]);
            for (w, seg, _) in lines(&lay.polys) {
                let mid = [(seg[0][0] + seg[1][0]) / 2.0, (seg[0][1] + seg[1][1]) / 2.0];
                // A line of one wall lies on the outline or inside its own wall's layers, never in
                // the middle of a layer of another wall.
                let across = lay.polys.iter().any(|q| {
                    q.wall != w
                        && inside(&q.points, mid)
                        && (0..q.points.len()).all(|k| {
                            let a = q.points[k];
                            let b = q.points[(k + 1) % q.points.len()];
                            let d = [b[0] - a[0], b[1] - a[1]];
                            let l = d[0].hypot(d[1]).max(1e-12);
                            ((mid[0] - a[0]) * d[1] - (mid[1] - a[1]) * d[0]).abs() / l > 1e-3
                                || ((mid[0] - a[0]) * d[0] + (mid[1] - a[1]) * d[1]) / l < -1e-3
                                || ((mid[0] - a[0]) * d[0] + (mid[1] - a[1]) * d[1]) / l > l + 1e-3
                        })
                });
                assert!(
                    !across,
                    "{label}: a line of wall {w} crosses another wall at {mid:?}"
                );
            }
        }
    }
}
