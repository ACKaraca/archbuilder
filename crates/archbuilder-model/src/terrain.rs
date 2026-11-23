//! Solids between planar height fields: the roof slab between its underside and its top, and a
//! wall whose top follows the underside of a roof.
//!
//! A height field is a set of convex pieces in plan ([`Piece`]), each with the plane its height
//! follows, that together cover a simple region once. [`solid`] makes the closed solid whose top
//! follows the pieces (raised by a constant) and whose bottom is either a level plane or the
//! pieces again (raised by another constant). The pieces are welded at their corners and every
//! corner that lies on another piece's edge is added to that edge, so neighbors share their
//! edges exactly; the outline of the union gets one vertical face per straight run.

use std::collections::HashMap;

use arc_base::math::Point3;
use arc_base::{Error, Limits, Result, Tolerance};
use arc_geom::{FaceTag, PolyFace, Polyhedron};

/// A plane over the plan: the height `z` at `at` and the rise per meter along x and y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// A point of the plan.
    pub at: [f64; 2],
    /// The height there.
    pub z: f64,
    /// The rise per meter along x and along y.
    pub grad: [f64; 2],
}

impl Plane {
    /// A level plane at height `z`.
    #[must_use]
    pub const fn level(z: f64) -> Self {
        Self {
            at: [0.0, 0.0],
            z,
            grad: [0.0, 0.0],
        }
    }

    /// The height over `p`.
    #[must_use]
    pub fn z(&self, p: [f64; 2]) -> f64 {
        self.z + self.grad[0] * (p[0] - self.at[0]) + self.grad[1] * (p[1] - self.at[1])
    }
}

/// A convex piece of a height field: its corners counterclockwise and its plane.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// The corners, counterclockwise.
    pub points: Vec<[f64; 2]>,
    /// The plane the height follows.
    pub plane: Plane,
    /// The plane of the top over the piece, for a solid whose top is not the pieces' planes
    /// raised by a constant.
    pub upper: Option<Plane>,
}

/// The bottom of a solid made by [`solid`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bottom {
    /// A level bottom at this height.
    Level(f64),
    /// The pieces' planes raised by this amount.
    Pieces(f64),
}

fn failed() -> Error {
    Error::new(arc_geom::error::codes::SOLID_TOPOLOGY_INVALID)
}

fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

/// Twice the signed area of a polygon.
#[must_use]
pub fn double_area(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    (0..n)
        .filter_map(|k| Some((points.get(k)?, points.get((k + 1) % n)?)))
        .map(|(a, b)| cross(*a, *b))
        .sum()
}

/// The part of `subject` inside the convex counterclockwise polygon `clip`, by clipping against
/// each edge's line in turn. A convex subject gives a convex result.
#[must_use]
pub fn clip_convex(subject: &[[f64; 2]], clip: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out = subject.to_vec();
    let n = clip.len();
    for k in 0..n {
        let (Some(&a), Some(&b)) = (clip.get(k), clip.get((k + 1) % n)) else {
            return Vec::new();
        };
        let e = sub(b, a);
        let side = |p: [f64; 2]| cross(e, sub(p, a));
        let input = std::mem::take(&mut out);
        let m = input.len();
        for i in 0..m {
            let (Some(&p), Some(&q)) = (input.get(i), input.get((i + 1) % m)) else {
                continue;
            };
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                out.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

/// The two counterclockwise triangles of a simple quadrilateral, split along the diagonal that
/// lies inside it.
#[must_use]
pub fn split_quad(q: [[f64; 2]; 4]) -> [[[f64; 2]; 3]; 2] {
    let ccw = double_area(&q) >= 0.0;
    let q = if ccw { q } else { [q[3], q[2], q[1], q[0]] };
    let positive = |t: [[f64; 2]; 3]| double_area(&t) > 0.0;
    let first = [[q[0], q[1], q[2]], [q[0], q[2], q[3]]];
    if first.iter().all(|t| positive(*t)) {
        first
    } else {
        [[q[1], q[2], q[3]], [q[1], q[3], q[0]]]
    }
}

/// The welded corners of a set of pieces.
struct Welded {
    points: Vec<[f64; 2]>,
    /// The height of the field at each corner.
    z: Vec<f64>,
    /// The height of the top at each corner, when the pieces have one.
    upper: Vec<Option<f64>>,
    loops: Vec<(Vec<usize>, Plane, Option<Plane>)>,
}

fn weld(pieces: &[Piece], tolerance: f64) -> Welded {
    let mut welded = Welded {
        points: Vec::new(),
        z: Vec::new(),
        upper: Vec::new(),
        loops: Vec::new(),
    };
    for piece in pieces {
        let mut ids: Vec<usize> = Vec::with_capacity(piece.points.len());
        for p in &piece.points {
            let found = welded
                .points
                .iter()
                .position(|q| (q[0] - p[0]).hypot(q[1] - p[1]) <= tolerance);
            let id = found.unwrap_or_else(|| {
                welded.points.push(*p);
                welded.z.push(piece.plane.z(*p));
                welded.upper.push(piece.upper.map(|u| u.z(*p)));
                welded.points.len() - 1
            });
            if ids.last() != Some(&id) {
                ids.push(id);
            }
        }
        while ids.len() > 1 && ids.first() == ids.last() {
            ids.pop();
        }
        let points: Vec<[f64; 2]> = ids
            .iter()
            .filter_map(|&i| welded.points.get(i).copied())
            .collect();
        if ids.len() >= 3 && double_area(&points) > tolerance * tolerance {
            welded.loops.push((ids, piece.plane, piece.upper));
        }
    }
    welded
}

/// Adds to every edge of every loop the welded corners that lie on it.
fn repair(welded: &mut Welded, tolerance: f64) {
    let points = welded.points.clone();
    for (ids, _, _) in &mut welded.loops {
        let mut out = Vec::with_capacity(ids.len());
        let n = ids.len();
        for k in 0..n {
            let (Some(&a), Some(&b)) = (ids.get(k), ids.get((k + 1) % n)) else {
                continue;
            };
            out.push(a);
            let (Some(&pa), Some(&pb)) = (points.get(a), points.get(b)) else {
                continue;
            };
            let d = sub(pb, pa);
            let length2 = d[0] * d[0] + d[1] * d[1];
            if length2 <= 0.0 {
                continue;
            }
            let mut on: Vec<(f64, usize)> = points
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != a && *i != b)
                .filter_map(|(i, p)| {
                    let w = sub(*p, pa);
                    let t = (w[0] * d[0] + w[1] * d[1]) / length2;
                    let off = cross(d, w).abs() / length2.sqrt();
                    let inside =
                        t * length2.sqrt() > tolerance && (1.0 - t) * length2.sqrt() > tolerance;
                    (inside && off <= tolerance).then_some((t, i))
                })
                .collect();
            on.sort_by(|x, y| x.0.total_cmp(&y.0));
            out.extend(on.into_iter().map(|(_, i)| i));
        }
        *ids = out;
    }
}

/// The closed solid over the pieces: the top at each corner is the top plane's height (else the
/// field's) plus `lift`,
/// the bottom as `bottom` says. The vertical faces of the outline are tagged by `side_tag` with
/// the two ends of their straight run; the top faces are [`FaceTag::Top`] and the bottom faces
/// [`FaceTag::Bottom`].
///
/// # Errors
/// `geom.solid.topology_invalid` when the pieces do not form one simple region, and the errors
/// of [`Polyhedron::new`] (for example a top that does not stay above the bottom).
#[allow(
    clippy::too_many_lines,
    reason = "weld, outline, vertices, faces: one pass"
)]
pub fn solid(
    pieces: &[Piece],
    lift: f64,
    bottom: Bottom,
    side_tag: &dyn Fn([f64; 2], [f64; 2]) -> FaceTag,
) -> Result<Polyhedron> {
    let tolerance = Tolerance::DEFAULT;
    let mut welded = weld(pieces, tolerance.linear);
    repair(&mut welded, tolerance.linear);
    // Where two neighbors' tops cross along their shared edge, the edge gets the crossing point,
    // so every step between them keeps one side up.
    if split_crossings(&mut welded, lift, tolerance.linear) {
        repair(&mut welded, tolerance.linear);
    }
    // The directed edges and the loop each belongs to; the outline is the edges without a
    // reverse partner.
    let mut owner: HashMap<(usize, usize), usize> = HashMap::new();
    for (li, (ids, _, _)) in welded.loops.iter().enumerate() {
        let n = ids.len();
        for k in 0..n {
            if let (Some(&a), Some(&b)) = (ids.get(k), ids.get((k + 1) % n))
                && owner.insert((a, b), li).is_some()
            {
                return Err(failed());
            }
        }
    }
    let mut next: HashMap<usize, usize> = HashMap::new();
    for &(a, b) in owner.keys() {
        if !owner.contains_key(&(b, a)) && next.insert(a, b).is_some() {
            return Err(failed());
        }
    }
    let mut cycles = outline_loops(&next)?;
    let point = |i: usize| welded.points.get(i).copied().ok_or_else(failed);
    // Start each loop at a corner, so the straight runs do not wrap.
    let turn = |outline: &[usize], k: usize| -> Result<bool> {
        let m = outline.len();
        let (a, b, c) = (
            point(*outline.get((k + m - 1) % m).ok_or_else(failed)?)?,
            point(*outline.get(k).ok_or_else(failed)?)?,
            point(*outline.get((k + 1) % m).ok_or_else(failed)?)?,
        );
        let (u, v) = (sub(b, a), sub(c, b));
        let lengths = u[0].hypot(u[1]) * v[0].hypot(v[1]);
        Ok(cross(u, v).abs() > tolerance.angular * lengths || u[0] * v[0] + u[1] * v[1] < 0.0)
    };
    for outline in &mut cycles {
        let mut first = None;
        for k in 0..outline.len() {
            if turn(outline, k)? {
                first = Some(k);
                break;
            }
        }
        outline.rotate_left(first.ok_or_else(failed)?);
    }
    // The top of corner `i` in loop `li`: that loop's top plane, else its field, plus `lift`.
    let loop_top = |li: usize, i: usize| -> Result<f64> {
        let (_, plane, upper) = welded.loops.get(li).ok_or_else(failed)?;
        let p = point(i)?;
        Ok(upper.map_or_else(|| plane.z(p), |u| u.z(p)) + lift)
    };
    // Vertices: the tops of every corner (one per distinct height), the bottoms it uses.
    let mut vertices: Vec<Point3> = Vec::new();
    let mut tops: HashMap<usize, Vec<(f64, u32)>> = HashMap::new();
    let mut bottom_of: HashMap<usize, u32> = HashMap::new();
    let add = |vertices: &mut Vec<Point3>, p: [f64; 2], z: f64| -> Result<u32> {
        vertices.push(Point3::new(p[0], p[1], z));
        u32::try_from(vertices.len() - 1).map_err(|_| failed())
    };
    let mut top = |vertices: &mut Vec<Point3>, i: usize, z: f64| -> Result<u32> {
        let list = tops.entry(i).or_default();
        if let Some((_, v)) = list.iter().find(|(h, _)| (h - z).abs() <= tolerance.linear) {
            return Ok(*v);
        }
        let v = add(vertices, point(i)?, z)?;
        list.push((z, v));
        Ok(v)
    };
    let height = |i: usize| welded.z.get(i).copied().ok_or_else(failed);
    let mut low = |vertices: &mut Vec<Point3>, i: usize| -> Result<u32> {
        if let Some(&v) = bottom_of.get(&i) {
            return Ok(v);
        }
        let z = match bottom {
            Bottom::Level(z) => z,
            Bottom::Pieces(raise) => height(i)? + raise,
        };
        let v = add(vertices, point(i)?, z)?;
        bottom_of.insert(i, v);
        Ok(v)
    };
    let mut faces = Vec::new();
    for (li, (ids, _, _)) in welded.loops.iter().enumerate() {
        let mut outer = Vec::with_capacity(ids.len());
        for &i in ids {
            let z = loop_top(li, i)?;
            outer.push(top(&mut vertices, i, z)?);
        }
        faces.push(PolyFace::new(outer).with_tag(FaceTag::Top));
        if let Bottom::Pieces(_) = bottom {
            let outer = ids
                .iter()
                .rev()
                .map(|&i| low(&mut vertices, i))
                .collect::<Result<Vec<_>>>()?;
            faces.push(PolyFace::new(outer).with_tag(FaceTag::Bottom));
        }
    }
    // A vertical step where two neighbors' tops differ along their shared edge.
    let mut shared: Vec<(usize, usize, usize, usize)> = owner
        .iter()
        .filter(|((a, b), _)| a < b)
        .filter_map(|(&(a, b), &l1)| owner.get(&(b, a)).map(|&l2| (a, b, l1, l2)))
        .collect();
    shared.sort_unstable();
    for (a, b, l1, l2) in shared {
        let (a1, b1) = (
            top(&mut vertices, a, loop_top(l1, a)?)?,
            top(&mut vertices, b, loop_top(l1, b)?)?,
        );
        let (a2, b2) = (
            top(&mut vertices, a, loop_top(l2, a)?)?,
            top(&mut vertices, b, loop_top(l2, b)?)?,
        );
        if a1 == a2 && b1 == b2 {
            continue;
        }
        let mut outer = vec![b1, a1, a2, b2];
        outer.dedup();
        // A step that is a triangle shares its corner at both ends of the list.
        if outer.len() > 1 && outer.first() == outer.last() {
            outer.pop();
        }
        if outer.len() >= 3 {
            faces.push(PolyFace::new(outer).with_tag(FaceTag::Top));
        }
    }
    if let Bottom::Level(_) = bottom {
        // The loop of the greatest area is the outer one, the others are holes.
        let area = |c: &Vec<usize>| -> f64 {
            let pts: Vec<[f64; 2]> = c.iter().filter_map(|&i| point(i).ok()).collect();
            double_area(&pts)
        };
        let mut rings = Vec::with_capacity(cycles.len());
        for c in &cycles {
            let ring = c
                .iter()
                .rev()
                .map(|&i| low(&mut vertices, i))
                .collect::<Result<Vec<_>>>()?;
            rings.push((area(c), ring));
        }
        rings.sort_by(|x, y| y.0.total_cmp(&x.0));
        let mut rings = rings.into_iter().map(|(_, r)| r);
        let mut face = PolyFace::new(rings.next().ok_or_else(failed)?).with_tag(FaceTag::Bottom);
        face.holes = rings.collect();
        faces.push(face);
    }
    // One vertical face per straight run of the outline; where two pieces' tops meet at
    // different heights on it, the run takes both, in the order the edges go.
    let edge_top = |a: usize, b: usize, at: usize| -> Result<f64> {
        let li = *owner.get(&(a, b)).ok_or_else(failed)?;
        loop_top(li, at)
    };
    for outline in &cycles {
        let m = outline.len();
        let mut k = 0;
        while k < m {
            let mut run = vec![*outline.get(k).ok_or_else(failed)?];
            let mut j = k + 1;
            loop {
                run.push(*outline.get(j % m).ok_or_else(failed)?);
                if j >= m || turn(outline, j % m)? {
                    break;
                }
                j += 1;
            }
            let mut outer = Vec::with_capacity(2 * run.len() + 2);
            for &i in &run {
                outer.push(low(&mut vertices, i)?);
            }
            let r = run.len();
            for idx in (0..r).rev() {
                let here = *run.get(idx).ok_or_else(failed)?;
                // Walking back: first the top of the edge after this corner, then the one before.
                if idx + 1 < r {
                    let after = *run.get(idx + 1).ok_or_else(failed)?;
                    let v = top(&mut vertices, here, edge_top(here, after, here)?)?;
                    if outer.last() != Some(&v) {
                        outer.push(v);
                    }
                }
                if idx > 0 {
                    let before = *run.get(idx - 1).ok_or_else(failed)?;
                    let v = top(&mut vertices, here, edge_top(before, here, here)?)?;
                    if outer.last() != Some(&v) {
                        outer.push(v);
                    }
                }
            }
            let (Some(&a), Some(&b)) = (run.first(), run.last()) else {
                return Err(failed());
            };
            faces.push(PolyFace::new(outer).with_tag(side_tag(point(a)?, point(b)?)));
            k = j;
        }
    }
    Polyhedron::new(vertices, faces, &tolerance, &Limits::DEFAULT)
}

/// The closed loops of the outline edges `next`: one around the pieces and one around each hole
/// (a shaft), each starting at its lowest vertex number.
fn outline_loops(next: &HashMap<usize, usize>) -> Result<Vec<Vec<usize>>> {
    let mut starts: Vec<usize> = next.keys().copied().collect();
    starts.sort_unstable();
    let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut cycles: Vec<Vec<usize>> = Vec::new();
    for start in starts {
        if !seen.insert(start) {
            continue;
        }
        let mut cycle = vec![start];
        let mut at = start;
        loop {
            at = *next.get(&at).ok_or_else(failed)?;
            if at == start {
                break;
            }
            if !seen.insert(at) {
                return Err(failed());
            }
            cycle.push(at);
        }
        cycles.push(cycle);
    }
    if cycles.is_empty() {
        return Err(failed());
    }
    Ok(cycles)
}

/// Adds the points where two neighboring loops' tops cross along a shared edge; true when it
/// added any.
fn split_crossings(welded: &mut Welded, lift: f64, tolerance: f64) -> bool {
    let mut owner: HashMap<(usize, usize), usize> = HashMap::new();
    for (li, (ids, _, _)) in welded.loops.iter().enumerate() {
        let n = ids.len();
        for k in 0..n {
            if let (Some(&a), Some(&b)) = (ids.get(k), ids.get((k + 1) % n)) {
                owner.insert((a, b), li);
            }
        }
    }
    let top = |li: usize, p: [f64; 2]| {
        welded
            .loops
            .get(li)
            .map(|(_, plane, upper)| upper.map_or_else(|| plane.z(p), |u| u.z(p)) + lift)
    };
    let mut added = Vec::new();
    for (&(a, b), &l1) in &owner {
        let Some(&l2) = owner.get(&(b, a)) else {
            continue;
        };
        if a > b {
            continue;
        }
        let (Some(&pa), Some(&pb)) = (welded.points.get(a), welded.points.get(b)) else {
            continue;
        };
        let (Some(a1), Some(a2), Some(b1), Some(b2)) =
            (top(l1, pa), top(l2, pa), top(l1, pb), top(l2, pb))
        else {
            continue;
        };
        let (da, db) = (a1 - a2, b1 - b2);
        if da.abs() > tolerance && db.abs() > tolerance && (da > 0.0) != (db > 0.0) {
            let t = da / (da - db);
            added.push([pa[0] + (pb[0] - pa[0]) * t, pa[1] + (pb[1] - pa[1]) * t]);
        }
    }
    for p in &added {
        welded.points.push(*p);
        welded.z.push(0.0);
        welded.upper.push(None);
    }
    !added.is_empty()
}

/// The part of the convex polygon `poly` on the left of the line from `a` to `b` (`left`) or on
/// its right.
fn half(poly: &[[f64; 2]], a: [f64; 2], b: [f64; 2], left: bool) -> Vec<[f64; 2]> {
    let e = sub(b, a);
    let side = |p: [f64; 2]| {
        let c = cross(e, sub(p, a));
        if left { c } else { -c }
    };
    let mut out = Vec::new();
    let m = poly.len();
    for i in 0..m {
        let (Some(&p), Some(&q)) = (poly.get(i), poly.get((i + 1) % m)) else {
            continue;
        };
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            let t = sp / (sp - sq);
            out.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
        }
    }
    out
}

/// The convex pieces of `pieces` outside the convex counterclockwise polygon `cut`.
#[must_use]
pub fn subtract_convex(
    pieces: Vec<Vec<[f64; 2]>>,
    cut: &[[f64; 2]],
    tolerance: f64,
) -> Vec<Vec<[f64; 2]>> {
    let mut out = Vec::new();
    let n = cut.len();
    for piece in pieces {
        let mut rest = piece;
        for k in 0..n {
            let (Some(&a), Some(&b)) = (cut.get(k), cut.get((k + 1) % n)) else {
                continue;
            };
            let outside = half(&rest, a, b, false);
            if outside.len() >= 3 && double_area(&outside) > tolerance * tolerance {
                out.push(outside);
            }
            rest = half(&rest, a, b, true);
            if rest.len() < 3 {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slab_of_two_pieces_and_a_wall_under_a_ridge() {
        // Two pitches of a gable over 10 x 6, rising 1 per meter toward the ridge at y = 3.
        let south = Piece {
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 3.0], [0.0, 3.0]],
            plane: Plane {
                at: [0.0, 0.0],
                z: 0.0,
                grad: [0.0, 1.0],
            },
            upper: None,
        };
        let north = Piece {
            points: vec![[0.0, 3.0], [10.0, 3.0], [10.0, 6.0], [0.0, 6.0]],
            plane: Plane {
                at: [0.0, 6.0],
                z: 0.0,
                grad: [0.0, -1.0],
            },
            upper: None,
        };
        let slab = solid(
            &[south.clone(), north.clone()],
            0.3,
            Bottom::Pieces(0.0),
            &|_, _| FaceTag::Untagged,
        )
        .unwrap();
        assert!((slab.volume() - 0.3 * 60.0).abs() < 1e-9);
        // A wall across the ridge, 0.2 thick, from a base 1 m below the eaves.
        let quad = [[4.9, 0.0], [5.1, 0.0], [5.1, 6.0], [4.9, 6.0]];
        let mut inside = Vec::new();
        for tri in split_quad(quad) {
            for p in [&south, &north] {
                let part = clip_convex(&tri, &p.points);
                if part.len() >= 3 {
                    inside.push(Piece {
                        points: part,
                        plane: p.plane,
                        upper: None,
                    });
                }
            }
        }
        let wall = solid(&inside, 0.0, Bottom::Level(-1.0), &|_, _| FaceTag::Untagged).unwrap();
        // 0.2 x 6 x 1 below the eaves plus the gable triangle 0.2 x (6 x 3 / 2).
        assert!(
            (wall.volume() - (1.2 + 1.8)).abs() < 1e-9,
            "{}",
            wall.volume()
        );
        // Six side faces would mean the runs were not merged: four sides, two tops, one bottom.
        assert_eq!(wall.faces().len(), 4 + 4 + 1);
    }
}
