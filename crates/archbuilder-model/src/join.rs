//! Wall joins, derived from the walls of one storey and never stored.
//!
//! Only the choice of each end ([`JoinStyle`]) is authored. [`footprints`] reads the baselines,
//! thicknesses, location lines and styles of the walls and returns, for each wall, the four
//! corners of its footprint with the ends closed against the walls they meet.
//!
//! The faces of a wall are the two lines parallel to its baseline at the offsets of
//! [`Wall::extent`]; every rule below uses those lines, never the baseline, so walls of different
//! thickness and location line close without a gap or an overlap.
//!
//! - Ends that coincide (within [`JOIN_TOLERANCE`]) form a node. Around a node the walls are taken
//!   in order of direction; between two neighbors the face of one that looks at the other meets
//!   the face of the other, and that point is the corner of both. Two walls therefore miter,
//!   three or four walls meet in a closed star, and two collinear walls of equal faces meet flush.
//! - An end that lies on the interior of another wall's baseline is a T: it stops at the face of
//!   that wall on its own side and never enters it.
//! - [`JoinStyle::Butt`] and [`JoinStyle::Square`] at a corner of two walls make this wall stop at
//!   the face of the other, which runs through to the far face of this one.
//! - [`JoinStyle::Disallowed`] takes the end out of every join.
//! - A corner that would reach more than [`MITER_LIMIT`] thicknesses past the end, or past the
//!   middle of the wall, is refused: every end of that node stays square. This covers angles
//!   close to 0 and to 180 degrees whose faces do not line up.
//!
//! The result of a node depends only on the walls that meet there, so a wall's footprint is the
//! same whichever wall's extraction computes it. Known limits: a wall that crosses another
//! without ending on it is not joined, and a through wall has no notch where it is crossed.

use std::collections::HashMap;

use crate::types::MIN_WALL_LENGTH;
use crate::wall::{JoinStyle, Wall};

/// Ends closer than this are one node, and an end this close to a baseline lies on it, in meters.
pub const JOIN_TOLERANCE: f64 = 1e-4;
/// The most a corner may reach past its end, in thicknesses of the walls that meet.
pub const MITER_LIMIT: f64 = 2.0;
/// Ends of two walls closer than this, but not joined, are an open corner (see [`open_corners`]).
pub const OPEN_CORNER: f64 = 0.5;
/// The most walls [`open_corners`] compares.
const MAX_COMPARED: usize = 2000;
/// A corner must stay this share of the wall's length from the other end.
const MAX_REACH: f64 = 0.45;
/// Two unit directions whose cross product is smaller are parallel.
const PARALLEL: f64 = 1e-9;
/// The edge of a cell of the grid that finds the baselines an end lies on, in meters.
const CELL: f64 = 4.0;
/// A baseline that would fill more cells goes to the list every query checks.
const MAX_CELLS: usize = 1024;
/// The most such long baselines that are checked; further ones are not considered for T joins.
const MAX_LONG: usize = 512;

type P = [f64; 2];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
/// The left normal of a unit direction.
fn left(d: P) -> P {
    [-d[1], d[0]]
}
/// `p` moved by `d * s`.
fn along(p: P, d: P, s: f64) -> P {
    [p[0] + d[0] * s, p[1] + d[1] * s]
}

/// A line through a point.
#[derive(Clone, Copy)]
struct Line {
    p: P,
    d: P,
}

impl Line {
    /// The point where two lines meet; `None` when they are parallel.
    fn hit(self, other: Self) -> Option<P> {
        let c = cross(self.d, other.d);
        if c.abs() < PARALLEL {
            return None;
        }
        let t = cross(sub(other.p, self.p), other.d) / c;
        Some(along(self.p, self.d, t))
    }
}

/// One end of a wall seen from its node: a ray into the wall.
#[derive(Clone, Copy)]
struct Ray {
    wall: usize,
    at_end: bool,
    /// The baseline's end point.
    point: P,
    /// The unit direction from the end into the wall.
    dir: P,
    /// The offsets of the right and left faces along the left normal of `dir`.
    lo: f64,
    hi: f64,
    length: f64,
    thickness: f64,
    style: JoinStyle,
}

/// The two corners of a ray's end: the left one (on the face at `hi`) and the right one.
type Corners = [P; 2];

impl Ray {
    fn new(walls: &[Wall], wall: usize, at_end: bool) -> Option<Self> {
        let w = &walls[wall];
        let length = w.length();
        if length.is_nan() || length < MIN_WALL_LENGTH || !length.is_finite() {
            return None;
        }
        let u = [
            (w.end[0] - w.start[0]) / length,
            (w.end[1] - w.start[1]) / length,
        ];
        let (low, high) = w.extent();
        Some(if at_end {
            Self {
                wall,
                at_end,
                point: w.end,
                dir: [-u[0], -u[1]],
                lo: -high,
                hi: -low,
                length,
                thickness: w.thickness,
                style: w.end_join,
            }
        } else {
            Self {
                wall,
                at_end,
                point: w.start,
                dir: u,
                lo: low,
                hi: high,
                length,
                thickness: w.thickness,
                style: w.start_join,
            }
        })
    }

    fn face(&self, offset: f64) -> Line {
        Line {
            p: along(self.point, left(self.dir), offset),
            d: self.dir,
        }
    }

    /// The end cut square across at the baseline's end point.
    fn square(&self) -> Corners {
        [
            along(self.point, left(self.dir), self.hi),
            along(self.point, left(self.dir), self.lo),
        ]
    }

    /// The end cut along `cut`: the points where the two faces meet it.
    fn cut_along(&self, cut: Line) -> Option<Corners> {
        Some([self.face(self.hi).hit(cut)?, self.face(self.lo).hit(cut)?])
    }

    /// The end cut square across as far as the nearer corner of `cut_along`.
    fn square_at(&self, cut: Line) -> Option<Corners> {
        let [l, r] = self.cut_along(cut)?;
        let t = dot(sub(l, self.point), self.dir).max(dot(sub(r, self.point), self.dir));
        let at = along(self.point, self.dir, t);
        Some([
            along(at, left(self.dir), self.hi),
            along(at, left(self.dir), self.lo),
        ])
    }

    /// Whether `corners` stay within the reach this wall allows, `reference` being the
    /// thickness the limit is counted in.
    fn fits(&self, corners: Corners, reference: f64) -> bool {
        corners.iter().all(|c| {
            let t = dot(sub(*c, self.point), self.dir);
            c[0].is_finite()
                && c[1].is_finite()
                && t <= MAX_REACH * self.length
                && t >= -MITER_LIMIT * reference
        })
    }
}

/// The corners of the rays of a node of two or more ends, in the order of `rays`; `None` when
/// the node cannot be joined without a spike or an overlap.
fn resolve_node(rays: &[Ray]) -> Option<Vec<Corners>> {
    // Three or more ends with a straight run: the run passes flush and the others stop at it.
    if rays.len() >= 3
        && let Some((i, j, through)) = straight(rays)
    {
        return rays
            .iter()
            .enumerate()
            .map(|(k, r)| {
                if k == i || k == j {
                    Some(r.square())
                } else {
                    tee(r, through)
                }
            })
            .collect();
    }
    if let [a, b] = rays {
        if let Some(done) = butt(a, b) {
            return Some(done);
        }
        if let Some(mut done) = butt(b, a) {
            done.reverse();
            return Some(done);
        }
    }
    star(rays)
}

/// A corner of two walls where exactly one end is butt or square: that one stops at the face of
/// the other, which is extended to the far face of the first.
fn butt(a: &Ray, b: &Ray) -> Option<Vec<Corners>> {
    let trimmed = |r: &Ray| matches!(r.style, JoinStyle::Butt | JoinStyle::Square);
    if !trimmed(a) || trimmed(b) {
        return None;
    }
    let turn = cross(b.dir, a.dir);
    if turn.abs() < PARALLEL {
        return None;
    }
    // The face of `b` that looks at `a`, and the face of `a` that looks away from `b`.
    let near = b.face(if turn > 0.0 { b.hi } else { b.lo });
    let far = a.face(if turn > 0.0 { a.hi } else { a.lo });
    let reference = a.thickness + b.thickness;
    let ca = if a.style == JoinStyle::Square {
        a.square_at(near)?
    } else {
        a.cut_along(near)?
    };
    let cb = b.cut_along(far)?;
    (a.fits(ca, reference) && b.fits(cb, reference)).then(|| vec![ca, cb])
}

/// The closed star of two or more ends, taken in order of direction; see the module notes.
fn star(rays: &[Ray]) -> Option<Vec<Corners>> {
    let mut order: Vec<usize> = (0..rays.len()).collect();
    order.sort_by(|&i, &j| {
        let angle = |k: usize| rays[k].dir[1].atan2(rays[k].dir[0]);
        angle(i)
            .total_cmp(&angle(j))
            .then(rays[i].wall.cmp(&rays[j].wall))
    });
    let mut out = vec![[[0.0; 2]; 2]; rays.len()];
    for (k, &i) in order.iter().enumerate() {
        let j = order[(k + 1) % order.len()];
        let (a, b) = (&rays[i], &rays[j]);
        // The wedge between `a` and the next ray counterclockwise: the left face of `a` meets the
        // right face of `b`.
        let (ca, cb) = if cross(a.dir, b.dir).abs() < PARALLEL {
            if dot(a.dir, b.dir) > 0.0 {
                return None;
            }
            // Straight through: each keeps its own square corner.
            (
                along(a.point, left(a.dir), a.hi),
                along(b.point, left(b.dir), b.lo),
            )
        } else {
            let c = a.face(a.hi).hit(b.face(b.lo))?;
            (c, c)
        };
        out[i][0] = ca;
        out[j][1] = cb;
    }
    for (ray, corners) in rays.iter().zip(&out) {
        let reference = rays
            .iter()
            .map(|r| r.thickness)
            .fold(ray.thickness, f64::max);
        if !ray.fits(*corners, reference) {
            return None;
        }
    }
    Some(out)
}

/// A wall as seen by an end that stops at its face: the unit direction of the baseline, the
/// offsets of its faces along the left normal, and its thickness.
#[derive(Clone, Copy)]
struct Through {
    dir: P,
    low: f64,
    high: f64,
    thickness: f64,
}

impl Through {
    fn of(wall: &Wall) -> Self {
        let length = wall.length();
        let (low, high) = wall.extent();
        Self {
            dir: [
                (wall.end[0] - wall.start[0]) / length,
                (wall.end[1] - wall.start[1]) / length,
            ],
            low,
            high,
            thickness: wall.thickness,
        }
    }
}

/// An end on the interior of a through wall: it stops at the face on its own side.
fn tee(a: &Ray, through: Through) -> Option<Corners> {
    let n = left(through.dir);
    let facing = dot(a.dir, n);
    if facing.abs() < PARALLEL {
        return None;
    }
    let cut = Line {
        p: along(
            a.point,
            n,
            if facing > 0.0 {
                through.high
            } else {
                through.low
            },
        ),
        d: through.dir,
    };
    let corners = if a.style == JoinStyle::Square {
        a.square_at(cut)?
    } else {
        a.cut_along(cut)?
    };
    a.fits(corners, a.thickness + through.thickness)
        .then_some(corners)
}

/// Two ends of a node that continue each other with the same faces: a wall running straight
/// through the node, as `(first, second, the through wall)`.
fn straight(rays: &[Ray]) -> Option<(usize, usize, Through)> {
    for (i, a) in rays.iter().enumerate() {
        for (j, b) in rays.iter().enumerate().skip(i + 1) {
            if cross(a.dir, b.dir).abs() < PARALLEL
                && dot(a.dir, b.dir) < 0.0
                && (a.hi + b.lo).abs() <= JOIN_TOLERANCE
                && (a.lo + b.hi).abs() <= JOIN_TOLERANCE
            {
                let through = Through {
                    dir: a.dir,
                    low: a.lo,
                    high: a.hi,
                    thickness: a.thickness.max(b.thickness),
                };
                return Some((i, j, through));
            }
        }
    }
    None
}

/// The bucket of a coordinate for buckets of edge `size`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "a float to integer cast saturates, so far-away and non-finite coordinates stay bounded"
)]
fn bucket(v: f64, size: f64) -> i64 {
    (v / size).floor() as i64
}

/// The cell of a point.
fn cell(p: P) -> (i64, i64) {
    (bucket(p[0], CELL), bucket(p[1], CELL))
}

/// The baselines of the walls by grid cell, to find the ones an end lies on.
struct Index {
    cells: HashMap<(i64, i64), Vec<usize>>,
    long: Vec<usize>,
}

impl Index {
    fn new(walls: &[Wall], valid: &[bool]) -> Self {
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        let mut long = Vec::new();
        for (i, w) in walls.iter().enumerate().filter(|(i, _)| valid[*i]) {
            let lo = cell([
                w.start[0].min(w.end[0]) - JOIN_TOLERANCE,
                w.start[1].min(w.end[1]) - JOIN_TOLERANCE,
            ]);
            let hi = cell([
                w.start[0].max(w.end[0]) + JOIN_TOLERANCE,
                w.start[1].max(w.end[1]) + JOIN_TOLERANCE,
            ]);
            let count = usize::try_from(hi.0.saturating_sub(lo.0).saturating_add(1))
                .unwrap_or(usize::MAX)
                .saturating_mul(
                    usize::try_from(hi.1.saturating_sub(lo.1).saturating_add(1))
                        .unwrap_or(usize::MAX),
                );
            if count > MAX_CELLS {
                // shortcut: only the first MAX_LONG very long walls are found as through walls.
                if long.len() < MAX_LONG {
                    long.push(i);
                }
                continue;
            }
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    cells.entry((x, y)).or_default().push(i);
                }
            }
        }
        Self { cells, long }
    }

    /// The wall whose baseline interior `p` lies on, other than `skip`: the nearest one, the
    /// first of equals.
    fn through(&self, walls: &[Wall], p: P, skip: &[usize]) -> Option<usize> {
        let near = self.cells.get(&cell(p)).map_or(&[][..], Vec::as_slice);
        let mut best: Option<(f64, usize)> = None;
        for &j in near.iter().chain(&self.long) {
            if skip.contains(&j) {
                continue;
            }
            let w = &walls[j];
            let length = w.length();
            let u = [
                (w.end[0] - w.start[0]) / length,
                (w.end[1] - w.start[1]) / length,
            ];
            let rel = sub(p, w.start);
            let (x, off) = (dot(rel, u), cross(u, rel).abs());
            if off <= JOIN_TOLERANCE
                && x > JOIN_TOLERANCE
                && x < length - JOIN_TOLERANCE
                && best.is_none_or(|(d, k)| off < d || (off == d && j < k))
            {
                best = Some((off, j));
            }
        }
        best.map(|(_, j)| j)
    }
}

/// The groups of ends within [`JOIN_TOLERANCE`] of each other; each group lists indexes into
/// `rays` in increasing order, and groups come in order of their first ray.
fn nodes(rays: &[Ray]) -> Vec<Vec<usize>> {
    let key = |p: P| {
        (
            bucket(p[0], JOIN_TOLERANCE * 4.0),
            bucket(p[1], JOIN_TOLERANCE * 4.0),
        )
    };
    let mut buckets: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (k, r) in rays.iter().enumerate() {
        buckets.entry(key(r.point)).or_default().push(k);
    }
    // Union by the smallest member.
    let mut root: Vec<usize> = (0..rays.len()).collect();
    fn find(root: &mut [usize], mut k: usize) -> usize {
        while root[k] != k {
            root[k] = root[root[k]];
            k = root[k];
        }
        k
    }
    for (k, r) in rays.iter().enumerate() {
        let (cx, cy) = key(r.point);
        for x in cx.saturating_sub(1)..=cx.saturating_add(1) {
            for y in cy.saturating_sub(1)..=cy.saturating_add(1) {
                for &m in buckets.get(&(x, y)).map_or(&[][..], Vec::as_slice) {
                    let d = sub(rays[m].point, r.point);
                    if m > k && d[0].abs() <= JOIN_TOLERANCE && d[1].abs() <= JOIN_TOLERANCE {
                        let (a, b) = (find(&mut root, k), find(&mut root, m));
                        root[a.max(b)] = a.min(b);
                    }
                }
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut at: HashMap<usize, usize> = HashMap::new();
    for k in 0..rays.len() {
        let r = find(&mut root, k);
        let g = *at.entry(r).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(k);
    }
    groups
}

/// The footprints of `walls`, which are the walls of one storey: for each wall the four corners
/// in the order of [`Wall::plain_footprint`], with its joined ends. A wall without a length, and
/// every end that is not joined, keeps the square footprint.
#[must_use]
pub fn footprints(walls: &[Wall]) -> Vec<[P; 4]> {
    let mut out: Vec<[P; 4]> = walls.iter().map(Wall::plain_footprint).collect();
    let valid: Vec<bool> = walls
        .iter()
        .map(|w| w.length() >= MIN_WALL_LENGTH && w.length().is_finite())
        .collect();
    // The ends that take part in joins; a disallowed end takes part in none.
    let rays: Vec<Ray> = (0..walls.len())
        .flat_map(|i| [(i, false), (i, true)])
        .filter_map(|(i, at_end)| Ray::new(walls, i, at_end))
        .filter(|r| r.style != JoinStyle::Disallowed)
        .collect();
    let index = Index::new(walls, &valid);
    for group in nodes(&rays) {
        let members: Vec<Ray> = group.iter().map(|&k| rays[k]).collect();
        let owners: Vec<usize> = members.iter().map(|r| r.wall).collect();
        let point = members[0].point;
        let corners: Vec<Corners> = match index.through(walls, point, &owners) {
            // A T: every end of the node stops at the through wall, or stays square.
            Some(through) => members
                .iter()
                .map(|r| tee(r, Through::of(&walls[through])).unwrap_or_else(|| r.square()))
                .collect(),
            None if members.len() >= 2 => {
                resolve_node(&members).unwrap_or_else(|| members.iter().map(Ray::square).collect())
            }
            None => continue,
        };
        for (r, [l, rt]) in members.iter().zip(corners) {
            let f = &mut out[r.wall];
            // The footprint runs start right, end right, end left, start left; the left corner
            // of a start ray is on the left, that of an end ray on the right.
            if r.at_end {
                f[1] = l;
                f[2] = rt;
            } else {
                f[0] = rt;
                f[3] = l;
            }
        }
    }
    out
}

/// The footprints of the walls of one storey at one snapshot, by wall.
type Storey = std::collections::BTreeMap<arc_base::ObjectId, [P; 4]>;

/// The most storeys whose footprints the cache keeps.
const CACHE_STOREYS: usize = 64;

/// The footprints of the storeys read last, each with the snapshot it was computed from: the
/// extraction of every wall of a storey at one revision shares one pass over its walls.
static CACHE: std::sync::Mutex<
    Vec<(
        arc_base::ObjectId,
        arc_doc::Snapshot,
        std::sync::Arc<Storey>,
    )>,
> = std::sync::Mutex::new(Vec::new());

/// The footprints of all the walls of `storey` at `snapshot`, in one pass.
fn storey_footprints(snapshot: &arc_doc::Snapshot, storey: arc_base::ObjectId) -> Storey {
    let mut ids = Vec::new();
    let mut walls = Vec::new();
    for w in crate::spatial::walls(snapshot, storey) {
        if let Some(data) = Wall::from_values(&w.values) {
            ids.push(w.id);
            walls.push(data);
        }
    }
    ids.into_iter().zip(footprints(&walls)).collect()
}

/// The joined footprint of one wall record: the corners of [`footprints`] over the walls of its
/// storey, computed once per storey and snapshot and shared by the walls of the storey. `None`
/// when the record is not a readable wall.
#[must_use]
pub fn footprint_of(snapshot: &arc_doc::Snapshot, object: &arc_doc::Object) -> Option<[P; 4]> {
    let own = Wall::from_values(&object.values)?;
    let Some(storey) = object.header.owner else {
        return Some(own.plain_footprint());
    };
    let cached = CACHE.lock().ok().and_then(|cache| {
        cache
            .iter()
            .find(|(s, snap, _)| *s == storey && snap.ptr_eq(snapshot))
            .map(|(_, _, f)| std::sync::Arc::clone(f))
    });
    let feet = cached.unwrap_or_else(|| {
        let feet = std::sync::Arc::new(storey_footprints(snapshot, storey));
        if let Ok(mut cache) = CACHE.lock() {
            // A newer snapshot of the storey replaces the older one.
            cache.retain(|(s, snap, _)| *s != storey || snap.ptr_eq(snapshot));
            if cache.len() >= CACHE_STOREYS {
                cache.remove(0);
            }
            cache.push((storey, snapshot.clone(), std::sync::Arc::clone(&feet)));
        }
        feet
    });
    Some(
        feet.get(&object.id)
            .copied()
            .unwrap_or_else(|| own.plain_footprint()),
    )
}

/// [`footprint_of`] without the cache: every call joins the whole storey again.
#[must_use]
pub fn footprint_uncached(
    snapshot: &arc_doc::Snapshot,
    object: &arc_doc::Object,
) -> Option<[P; 4]> {
    let own = Wall::from_values(&object.values)?;
    let storey = object.header.owner?;
    storey_footprints(snapshot, storey)
        .get(&object.id)
        .copied()
        .or_else(|| Some(own.plain_footprint()))
}

/// The distance from `p` to the segment `a`-`b`.
fn segment_distance(p: P, a: P, b: P) -> f64 {
    let ab = sub(b, a);
    let len2 = dot(ab, ab);
    let t = if len2 > 0.0 {
        (dot(sub(p, a), ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = along(a, ab, t);
    (p[0] - q[0]).hypot(p[1] - q[1])
}

/// The pairs of walls (by index, `i < j`) that have an open corner: an end of each lies within
/// [`OPEN_CORNER`] of the other but not within [`JOIN_TOLERANCE`], neither end lies on the other's
/// baseline, and the walls are not parallel. Such walls look joined and are not.
#[must_use]
pub fn open_corners(walls: &[Wall]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let n = walls.len().min(MAX_COMPARED);
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&walls[i], &walls[j]);
            let (da, db) = (sub(a.end, a.start), sub(b.end, b.start));
            let (la, lb) = (da[0].hypot(da[1]), db[0].hypot(db[1]));
            if la <= 0.0 || lb <= 0.0 || (cross(da, db) / (la * lb)).abs() < 1e-3 {
                continue;
            }
            let open = [a.start, a.end].iter().any(|&pa| {
                [b.start, b.end].iter().any(|&pb| {
                    let d = (pa[0] - pb[0]).hypot(pa[1] - pb[1]);
                    d > JOIN_TOLERANCE
                        && d <= OPEN_CORNER
                        && segment_distance(pa, b.start, b.end) > JOIN_TOLERANCE
                        && segment_distance(pb, a.start, a.end) > JOIN_TOLERANCE
                })
            });
            if open {
                out.push((i, j));
            }
        }
    }
    out
}

/// Where the baseline of `wall` meets the infinite line through `from` with direction `dir`:
/// the point and its signed distance along `wall`'s direction from `wall.start`; `None` when
/// they are parallel.
#[must_use]
pub fn line_hit(wall: &Wall, from: P, dir: P) -> Option<(P, f64)> {
    let d = sub(wall.end, wall.start);
    let len = d[0].hypot(d[1]);
    if len <= 0.0 {
        return None;
    }
    let u = [d[0] / len, d[1] / len];
    let dl = dir[0].hypot(dir[1]);
    if dl <= 0.0 {
        return None;
    }
    let v = [dir[0] / dl, dir[1] / dl];
    let c = cross(u, v);
    if c.abs() < 1e-9 {
        return None;
    }
    // wall.start + u*s lies on the line: cross(start + u s - from, v) = 0.
    let s = cross(sub(from, wall.start), v) / c;
    Some((along(wall.start, u, s), s))
}

/// The distance from `p` to the baseline of `wall`.
#[must_use]
pub fn baseline_distance(wall: &Wall, p: P) -> f64 {
    segment_distance(p, wall.start, wall.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wall::LocationLine;

    fn wall(start: P, end: P, thickness: f64, location: LocationLine) -> Wall {
        Wall {
            start,
            end,
            thickness,
            height: 3.0,
            base_offset: 0.0,
            location,
            start_join: JoinStyle::Auto,
            end_join: JoinStyle::Auto,
        }
    }

    fn volume(walls: &[Wall]) -> f64 {
        let feet = footprints(walls);
        walls
            .iter()
            .zip(feet)
            .map(|(w, f)| w.solid_in(f, 0.0).map_or(f64::NAN, |s| s.volume()))
            .sum()
    }

    fn area(points: &[P]) -> f64 {
        let n = points.len();
        (0..n)
            .map(|k| cross(points[k], points[(k + 1) % n]))
            .sum::<f64>()
            .abs()
            / 2.0
    }

    /// A closed counterclockwise chain of four walls on a 10 by 6 rectangle.
    fn house(thickness: [f64; 4], location: LocationLine) -> Vec<Wall> {
        let c = [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
        (0..4)
            .map(|k| wall(c[k], c[(k + 1) % 4], thickness[k], location))
            .collect()
    }

    #[test]
    fn a_closed_chain_has_the_volume_of_the_ring_for_each_location_line() {
        // Center: the ring between the rectangles offset by half the thickness each way.
        let v = volume(&house([0.2; 4], LocationLine::Center));
        assert!((v - (10.2 * 6.2 - 9.8 * 5.8) * 3.0).abs() < 1e-9, "{v}");
        // Counterclockwise: the body of an exterior line is on the right, outside the baseline.
        let v = volume(&house([0.2; 4], LocationLine::Exterior));
        assert!((v - (10.4 * 6.4 - 10.0 * 6.0) * 3.0).abs() < 1e-9, "{v}");
        let v = volume(&house([0.2; 4], LocationLine::Interior));
        assert!((v - (10.0 * 6.0 - 9.6 * 5.6) * 3.0).abs() < 1e-9, "{v}");
        // Mixed thickness: 0.3 along x, 0.2 along y.
        let v = volume(&house([0.3, 0.2, 0.3, 0.2], LocationLine::Center));
        assert!((v - (10.2 * 6.3 - 9.8 * 5.7) * 3.0).abs() < 1e-9, "{v}");
    }

    #[test]
    fn mixed_location_lines_still_close() {
        let mut walls = house([0.2, 0.3, 0.2, 0.3], LocationLine::Center);
        walls[0].location = LocationLine::Exterior;
        walls[1].location = LocationLine::Interior;
        let feet = footprints(&walls);
        // The ends of neighbors share their two corners: no gap and no overlap.
        for k in 0..4 {
            let (a, b) = (feet[k], feet[(k + 1) % 4]);
            let same = |p: P, q: P| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9;
            let shared = [a[1], a[2]]
                .iter()
                .all(|p| [b[0], b[3]].iter().any(|q| same(*p, *q)));
            assert!(shared, "corner {k}");
        }
        let total: f64 = feet.iter().map(|f| area(f)).sum();
        // The footprints tile a ring: no overlap means the areas add up to the ring's area.
        let outer = area(&[feet[0][0], feet[1][0], feet[2][0], feet[3][0]]);
        assert!(total > 0.0 && outer > 0.0);
    }

    #[test]
    fn a_t_stops_at_the_near_face_and_does_not_enter_the_through_wall() {
        let walls = [
            wall([0.0, 0.0], [10.0, 0.0], 0.2, LocationLine::Center),
            wall([5.0, 0.0], [5.0, 4.0], 0.1, LocationLine::Center),
        ];
        let f = footprints(&walls);
        assert_eq!(f[0], walls[0].plain_footprint());
        assert!(f[1].iter().all(|p| p[1] >= 0.1 - 1e-12));
        assert!((volume(&walls) - (10.0 * 0.2 + 3.9 * 0.1) * 3.0).abs() < 1e-9);
        // From the other side: the near face is the one at -0.1.
        let below = [
            walls[0],
            wall([5.0, 0.0], [5.0, -4.0], 0.1, LocationLine::Center),
        ];
        assert!((volume(&below) - (10.0 * 0.2 + 3.9 * 0.1) * 3.0).abs() < 1e-9);
    }

    #[test]
    fn collinear_walls_of_equal_faces_meet_flush_and_a_step_stays_square() {
        let flush = [
            wall([0.0, 0.0], [4.0, 0.0], 0.2, LocationLine::Center),
            wall([4.0, 0.0], [9.0, 0.0], 0.2, LocationLine::Center),
        ];
        assert!((volume(&flush) - 9.0 * 0.2 * 3.0).abs() < 1e-9);
        let f = footprints(&flush);
        assert_eq!(f[0], flush[0].plain_footprint());
        // Different thickness: each ends square at the shared point.
        let step = [
            flush[0],
            wall([4.0, 0.0], [9.0, 0.0], 0.3, LocationLine::Center),
        ];
        assert!((volume(&step) - (4.0 * 0.2 + 5.0 * 0.3) * 3.0).abs() < 1e-9);
    }

    #[test]
    fn three_and_four_walls_close_a_star() {
        // A T of three walls at one point: the stem meets the two halves.
        let t = [
            wall([0.0, 0.0], [5.0, 0.0], 0.2, LocationLine::Center),
            wall([5.0, 0.0], [10.0, 0.0], 0.2, LocationLine::Center),
            wall([5.0, 0.0], [5.0, 4.0], 0.2, LocationLine::Center),
        ];
        assert!((volume(&t) - (10.0 * 0.2 + 3.9 * 0.2) * 3.0).abs() < 1e-9);
        let plus = [
            t[0],
            t[1],
            t[2],
            wall([5.0, 0.0], [5.0, -4.0], 0.2, LocationLine::Center),
        ];
        assert!((volume(&plus) - (10.0 * 0.2 + 2.0 * 3.9 * 0.2) * 3.0).abs() < 1e-9);
    }

    #[test]
    fn butt_runs_one_wall_through_in_either_order() {
        let base = [
            wall([0.0, 0.0], [4.0, 0.0], 0.2, LocationLine::Center),
            wall([4.0, 0.0], [4.0, 3.0], 0.2, LocationLine::Center),
        ];
        let ring = (4.1 * 0.2 + 2.9 * 0.2) * 3.0;
        // The first wall stops at the second, which reaches the first's far face.
        let mut a = base;
        a[0].end_join = JoinStyle::Butt;
        let f = footprints(&a);
        assert!((f[0][1][0] - 3.9).abs() < 1e-9 && (f[1][0][1] + 0.1).abs() < 1e-9);
        assert!((volume(&a) - ring).abs() < 1e-9, "{}", volume(&a));
        // The second stops at the first, which runs to the second's far face.
        let mut b = base;
        b[1].start_join = JoinStyle::Butt;
        let f = footprints(&b);
        assert!(
            (f[1][0][1] - 0.1).abs() < 1e-9 && (f[0][1][0] - 4.1).abs() < 1e-9,
            "{f:?}"
        );
        assert!((volume(&b) - ring).abs() < 1e-9);
        // Both butt: they cancel into the miter.
        let mut both = base;
        both[0].end_join = JoinStyle::Butt;
        both[1].start_join = JoinStyle::Butt;
        assert!((volume(&both) - ring).abs() < 1e-9);
        assert!((footprints(&both)[0][2][0] - 3.9).abs() < 1e-9);
    }

    #[test]
    fn a_disallowed_end_stays_square_and_may_overlap() {
        let mut walls = [
            wall([0.0, 0.0], [4.0, 0.0], 0.2, LocationLine::Center),
            wall([4.0, 0.0], [4.0, 3.0], 0.2, LocationLine::Center),
        ];
        walls[0].end_join = JoinStyle::Disallowed;
        let f = footprints(&walls);
        assert_eq!(f[0], walls[0].plain_footprint());
        // The other wall is not joined to it either: its end stays square, so they overlap.
        assert_eq!(f[1], walls[1].plain_footprint());
        assert!((volume(&walls) - (4.0 * 0.2 + 3.0 * 0.2) * 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_square_end_stops_where_its_nearer_corner_touches() {
        let mut walls = [
            wall([0.0, 0.0], [10.0, 0.0], 0.2, LocationLine::Center),
            wall([5.0, 0.0], [8.0, 4.0], 0.2, LocationLine::Center),
        ];
        walls[1].start_join = JoinStyle::Square;
        let f = footprints(&walls);
        // The two start corners are on one square cut, both outside the through wall.
        assert!((f[1][0][1] - f[1][3][1]).abs() > 0.0);
        assert!(f[1].iter().all(|p| p[1] >= 0.1 - 1e-9));
    }

    #[test]
    fn a_very_sharp_corner_falls_back_to_square_ends() {
        let walls = [
            wall([0.0, 0.0], [10.0, 0.0], 0.2, LocationLine::Center),
            wall([0.0, 0.0], [10.0, 0.3], 0.2, LocationLine::Center),
        ];
        let f = footprints(&walls);
        assert_eq!(f[0], walls[0].plain_footprint());
        assert_eq!(f[1], walls[1].plain_footprint());
        // A bend of about 20 degrees is within the limit and is mitered.
        let bent = [
            wall([0.0, 0.0], [10.0, 0.0], 0.2, LocationLine::Center),
            wall([10.0, 0.0], [20.0, 3.6], 0.2, LocationLine::Center),
        ];
        assert!(footprints(&bent)[0] != bent[0].plain_footprint());
        // Folded back onto itself: no spike either.
        let folded = [
            wall([0.0, 0.0], [5.0, 0.0], 0.2, LocationLine::Center),
            wall([5.0, 0.0], [0.0, 0.0], 0.2, LocationLine::Center),
        ];
        assert_eq!(footprints(&folded)[0], folded[0].plain_footprint());
    }

    #[test]
    fn a_nearly_straight_bend_with_equal_faces_has_no_spike() {
        let walls = [
            wall([0.0, 0.0], [5.0, 0.0], 0.2, LocationLine::Center),
            wall([5.0, 0.0], [10.0, 0.001], 0.2, LocationLine::Center),
        ];
        for foot in footprints(&walls) {
            assert!(foot.iter().all(|p| p[0].abs() < 10.1 && p[1].abs() < 0.2));
        }
    }

    #[test]
    fn hundreds_of_walls_join_through_the_grid() {
        // A 20 by 20 grid of unit cells: 21 horizontal and 21 vertical walls of 20 units, in
        // pieces of one unit, so every node is a corner, a T or a cross of walls.
        let mut walls = Vec::new();
        for i in 0..=20 {
            for k in 0..20 {
                let (a, b) = (f64::from(k), f64::from(k + 1));
                let c = f64::from(i);
                walls.push(wall([a, c], [b, c], 0.2, LocationLine::Center));
                walls.push(wall([c, a], [c, b], 0.2, LocationLine::Center));
            }
        }
        assert_eq!(walls.len(), 840);
        let v = volume(&walls);
        // Every junction is closed without overlap: the volume is that of the union of the bars.
        let union = (20.2 * 20.2 - 400.0 * 0.64) * 3.0;
        assert!((v - union).abs() < 1e-6, "{v} vs {union}");
    }
}
