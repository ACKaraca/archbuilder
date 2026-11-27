//! Reading the bodies of an import as the shapes the model has: a mesh in world coordinates is
//! recognized as a vertical prism (a footprint with holes between two horizontal planes), and a
//! prism with a rectangular footprint as a wall body. Everything else stays a mesh.
//!
//! All work is on plain arrays and bounded by the mesh size; no function panics.

use std::collections::HashMap;

use arc_base::math::Point3;
use arc_base::{Limits, Result};
use arc_io_ifc::GeometryItem;
use arc_mesh::TriMesh;

/// Points closer than this are one point, in meters.
const SNAP: f64 = 1.0e-6;
/// How far a normal may deviate from the vertical (or the horizontal) for a face to count as
/// horizontal (or vertical): the cosine of the angle.
const FLAT: f64 = 1.0e-4;
/// The relative tolerance of the volume check and of the right angles of a rectangle.
const REL: f64 = 1.0e-3;

/// A triangle soup: positions and triangles by index.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Soup {
    /// The positions, in meters.
    pub p: Vec<[f64; 3]>,
    /// The triangles, counterclockwise seen from outside.
    pub t: Vec<[u32; 3]>,
}

impl Soup {
    /// The meshes of `items` in world coordinates, merged; empty for items without a mesh.
    ///
    /// # Errors
    /// The limits of the mesh the items are transformed into.
    pub fn of(items: &[&GeometryItem], limits: &Limits) -> Result<Self> {
        let mut soup = Self::default();
        for item in items {
            let Some(mesh) = item.world_mesh(limits)? else {
                continue;
            };
            let base = u32::try_from(soup.p.len()).unwrap_or(u32::MAX);
            soup.p
                .extend(mesh.positions().iter().map(|p| [p.x, p.y, p.z]));
            soup.t.extend(
                mesh.triangles()
                    .iter()
                    .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
            );
        }
        Ok(soup)
    }

    /// Whether there are no triangles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.t.is_empty()
    }

    /// The vertex `i`; the origin for an index out of range.
    fn at(&self, i: u32) -> [f64; 3] {
        self.p.get(i as usize).copied().unwrap_or([0.0; 3])
    }

    /// The signed volume: positive for a mesh wound counterclockwise seen from outside.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.t
            .iter()
            .map(|t| {
                let (a, b, c) = (self.at(t[0]), self.at(t[1]), self.at(t[2]));
                dot(a, cross(b, c)) / 6.0
            })
            .sum()
    }

    /// The bounding box, minimum then maximum; `None` without vertices.
    #[must_use]
    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        let first = *self.p.first()?;
        let mut b = (first, first);
        for p in &self.p {
            b.0 = [b.0[0].min(p[0]), b.0[1].min(p[1]), b.0[2].min(p[2])];
            b.1 = [b.1[0].max(p[0]), b.1[1].max(p[1]), b.1[2].max(p[2])];
        }
        Some(b)
    }

    /// The mesh with the triangles that are not degenerate.
    ///
    /// # Errors
    /// The validation of the mesh.
    pub fn to_mesh(&self, limits: &Limits) -> Result<TriMesh> {
        let positions = self
            .p
            .iter()
            .map(|p| Point3::new(p[0], p[1], p[2]))
            .collect();
        TriMesh::new(positions, self.t.clone(), limits)
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A vertical prism: a footprint between two horizontal planes.
#[derive(Debug, Clone, PartialEq)]
pub struct Prism {
    /// The height of the bottom plane.
    pub z0: f64,
    /// The height of the top plane.
    pub z1: f64,
    /// The outer loop, counterclockwise, without repeated or collinear points.
    pub outer: Vec<[f64; 2]>,
    /// The holes, clockwise.
    pub holes: Vec<Vec<[f64; 2]>>,
}

/// The signed area of a loop, positive when counterclockwise.
#[must_use]
pub fn area(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a[0] * b[1] - a[1] * b[0]
        })
        .sum::<f64>()
        / 2.0
}

/// The loop without points that repeat the previous one or lie on the line of their neighbors.
fn simplify(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::new();
    for p in points {
        if out
            .last()
            .is_none_or(|q| (q[0] - p[0]).hypot(q[1] - p[1]) > SNAP)
        {
            out.push(*p);
        }
    }
    while out.len() > 1
        && out
            .first()
            .zip(out.last())
            .is_some_and(|(a, b)| (a[0] - b[0]).hypot(a[1] - b[1]) <= SNAP)
    {
        out.pop();
    }
    loop {
        let n = out.len();
        if n < 4 {
            return out;
        }
        let drop = (0..n).find(|&i| {
            let (a, b, c) = (out[(i + n - 1) % n], out[i], out[(i + 1) % n]);
            let cross = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
            let reach = (c[0] - a[0]).hypot(c[1] - a[1]);
            cross.abs() <= 1.0e-7 * reach.max(1.0)
        });
        match drop {
            Some(i) => {
                out.remove(i);
            }
            None => return out,
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "a coordinate over 1e-6 m grid, within the model's range of 1e5 m"
)]
/// The boundary loops of the triangles `picked` seen in plan, oriented so that the outer loops
/// are counterclockwise.
fn loops(soup: &Soup, picked: &[usize], flip: bool) -> Option<Vec<Vec<[f64; 2]>>> {
    let mut ids: HashMap<(i64, i64), usize> = HashMap::new();
    let mut points: Vec<[f64; 2]> = Vec::new();
    let mut id_of = |p: [f64; 3]| -> usize {
        let key = ((p[0] / SNAP).round() as i64, (p[1] / SNAP).round() as i64);
        *ids.entry(key).or_insert_with(|| {
            points.push([p[0], p[1]]);
            points.len() - 1
        })
    };
    let mut edges: HashMap<(usize, usize), i32> = HashMap::new();
    for &k in picked {
        let t = soup.t.get(k)?;
        let v = [
            id_of(soup.at(t[0])),
            id_of(soup.at(t[1])),
            id_of(soup.at(t[2])),
        ];
        for (a, b) in [(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
            if a == b {
                continue;
            }
            let (a, b) = if flip { (b, a) } else { (a, b) };
            if let Some(count) = edges.get_mut(&(b, a)) {
                *count -= 1;
            } else {
                *edges.entry((a, b)).or_insert(0) += 1;
            }
        }
    }
    let mut next: HashMap<usize, Vec<usize>> = HashMap::new();
    for ((a, b), count) in &edges {
        if *count > 0 {
            next.entry(*a).or_default().push(*b);
        }
    }
    let mut out = Vec::new();
    let mut starts: Vec<usize> = next.keys().copied().collect();
    starts.sort_unstable();
    for start in starts {
        while let Some(first) = next.get_mut(&start).and_then(Vec::pop) {
            let mut ring = vec![start];
            let mut current = first;
            while current != start {
                ring.push(current);
                current = next.get_mut(&current).and_then(Vec::pop)?;
                if ring.len() > points.len() + 1 {
                    return None;
                }
            }
            let ring = simplify(&ring.iter().map(|&i| points[i]).collect::<Vec<_>>());
            if ring.len() >= 3 {
                out.push(ring);
            }
        }
    }
    Some(out)
}

/// Recognizes a closed vertical prism: every face horizontal or vertical, one bottom and one top
/// plane, and a volume equal to the footprint times the height.
#[must_use]
pub fn prism(soup: &Soup) -> Option<Prism> {
    let (mut bottom, mut top) = (Vec::new(), Vec::new());
    for (k, t) in soup.t.iter().enumerate() {
        let (a, b, c) = (soup.at(t[0]), soup.at(t[1]), soup.at(t[2]));
        let n = cross(sub(b, a), sub(c, a));
        let len = dot(n, n).sqrt();
        if len < 1.0e-12 {
            continue;
        }
        let nz = n[2] / len;
        if nz < -(1.0 - FLAT) {
            bottom.push(k);
        } else if nz > 1.0 - FLAT {
            top.push(k);
        } else if nz.abs() > FLAT {
            return None;
        }
    }
    let plane = |picked: &[usize]| -> Option<f64> {
        let zs: Vec<f64> = picked
            .iter()
            .flat_map(|&k| soup.t.get(k).into_iter().flatten().map(|&i| soup.at(i)[2]))
            .collect();
        let (lo, hi) = zs
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), z| (l.min(*z), h.max(*z)));
        (!zs.is_empty() && hi - lo <= 1.0e-5).then_some((lo + hi) / 2.0)
    };
    let (z0, z1) = (plane(&bottom)?, plane(&top)?);
    if z1 - z0 <= 1.0e-6 {
        return None;
    }
    // Seen from below a bottom face runs clockwise; flipped, the outer loop is counterclockwise.
    let mut rings = loops(soup, &bottom, true)?;
    rings.sort_by(|a, b| area(b).abs().total_cmp(&area(a).abs()));
    let outer = rings.first()?.clone();
    if area(&outer) <= 0.0 {
        return None;
    }
    let holes: Vec<Vec<[f64; 2]>> = rings.into_iter().skip(1).collect();
    if holes.iter().any(|h| area(h) >= 0.0) {
        return None;
    }
    let footprint = area(&outer) + holes.iter().map(|h| area(h)).sum::<f64>();
    let expected = footprint * (z1 - z0);
    let volume = soup.volume();
    if (volume - expected).abs() > REL * expected.abs().max(1.0e-9) {
        return None;
    }
    Some(Prism {
        z0,
        z1,
        outer,
        holes,
    })
}

/// A strip footprint: two parallel long sides with an end across each, which may be slanted (a
/// wall mitered at its corners). The middle line runs between the middles of the two ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strip {
    /// The start of the middle line.
    pub start: [f64; 2],
    /// The end of the middle line.
    pub end: [f64; 2],
    /// The distance between the long sides.
    pub thickness: f64,
    /// Whether the start end is square to the long sides.
    pub start_square: bool,
    /// Whether the end end is square to the long sides.
    pub end_square: bool,
}

/// The strip a loop of four points is, if it is one.
#[must_use]
pub fn strip(points: &[[f64; 2]]) -> Option<Strip> {
    let [a, b, c, d] = points else {
        return None;
    };
    let edge = |p: [f64; 2], q: [f64; 2]| [q[0] - p[0], q[1] - p[1]];
    let len = |e: [f64; 2]| e[0].hypot(e[1]);
    let e = [edge(*a, *b), edge(*b, *c), edge(*c, *d), edge(*d, *a)];
    let l = [len(e[0]), len(e[1]), len(e[2]), len(e[3])];
    if l.iter().any(|x| *x <= SNAP) {
        return None;
    }
    let parallel =
        |i: usize, j: usize| (e[i][0] * e[j][1] - e[i][1] * e[j][0]).abs() / (l[i] * l[j]) <= REL;
    // The long sides are the parallel pair; when both pairs are parallel, the longer pair.
    let sides = match (parallel(0, 2), parallel(1, 3)) {
        (true, true) => {
            if l[0] + l[2] >= l[1] + l[3] {
                0
            } else {
                1
            }
        }
        (true, false) => 0,
        (false, true) => 1,
        (false, false) => return None,
    };
    let p = [*a, *b, *c, *d];
    // Side `sides` runs p[sides]->p[sides+1] and p[sides+2]->p[sides+3]; the ends are between.
    let s0 = [p[sides], p[(sides + 1) % 4]];
    let s1 = [p[(sides + 2) % 4], p[(sides + 3) % 4]];
    let dir = edge(s0[0], s0[1]);
    let dl = len(dir);
    let u = [dir[0] / dl, dir[1] / dl];
    // The distance of the second side from the first.
    let thickness = ((s1[0][0] - s0[0][0]) * -u[1] + (s1[0][1] - s0[0][1]) * u[0]).abs();
    if thickness <= SNAP {
        return None;
    }
    let mid = |p: [f64; 2], q: [f64; 2]| [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
    // The end across the near ends of the two sides, and the one across the far ends.
    let (near, far) = (mid(s0[0], s1[1]), mid(s0[1], s1[0]));
    let square = |x: [f64; 2], y: [f64; 2]| {
        let v = edge(x, y);
        let vl = len(v);
        vl > 0.0 && (v[0] * u[0] + v[1] * u[1]).abs() / vl <= REL
    };
    Some(Strip {
        start: near,
        end: far,
        thickness,
        start_square: square(s0[0], s1[1]),
        end_square: square(s0[1], s1[0]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box from (0,0,0) to (4,0.2,3) as twelve triangles, wound outward.
    pub(crate) fn cuboid(x: f64, y: f64, z: f64) -> Soup {
        let p = vec![
            [0.0, 0.0, 0.0],
            [x, 0.0, 0.0],
            [x, y, 0.0],
            [0.0, y, 0.0],
            [0.0, 0.0, z],
            [x, 0.0, z],
            [x, y, z],
            [0.0, y, z],
        ];
        let t = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Soup { p, t }
    }

    #[test]
    fn a_box_is_a_prism_with_a_rectangular_footprint() {
        let soup = cuboid(4.0, 0.2, 3.0);
        assert!((soup.volume() - 2.4).abs() < 1e-9);
        let prism = prism(&soup).expect("prism");
        assert!((prism.z0).abs() < 1e-9 && (prism.z1 - 3.0).abs() < 1e-9);
        assert_eq!(prism.outer.len(), 4);
        assert!(area(&prism.outer) > 0.0);
        let strip = strip(&prism.outer).expect("strip");
        assert!((strip.thickness - 0.2).abs() < 1e-9);
        let length = (strip.end[0] - strip.start[0]).hypot(strip.end[1] - strip.start[1]);
        assert!((length - 4.0).abs() < 1e-9);
        assert!(strip.start_square && strip.end_square);
    }

    #[test]
    fn a_mitered_wall_is_a_strip_with_slanted_ends() {
        // Long sides 0 and 0.2 from the axis, the start cut at 45 degrees.
        let outer = [[0.1, -0.1], [4.0, -0.1], [3.9, 0.1], [0.0, 0.1]];
        let strip = strip(&outer).expect("strip");
        assert!((strip.thickness - 0.2).abs() < 1e-9);
        assert!(!strip.start_square && !strip.end_square);
        assert!((strip.start[0] - 0.05).abs() < 1e-9 && strip.start[1].abs() < 1e-9);
    }

    #[test]
    fn a_slanted_top_is_not_a_prism() {
        let mut soup = cuboid(4.0, 0.2, 3.0);
        soup.p[6][2] = 4.0;
        soup.p[7][2] = 4.0;
        assert!(prism(&soup).is_none());
    }
}
