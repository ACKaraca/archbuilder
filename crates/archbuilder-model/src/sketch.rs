//! The sketch of a floor boundary: the outline and the openings as loops of vertices that can be
//! edited before they are applied. A sketch holds what a floor record holds (points, arcs as
//! bulges, the wall an edge follows), with the points of edges that follow walls already derived,
//! so what the user sees is what the floor is.
//!
//! The operations are pure and return what they changed. A wall-following edge stays linked while
//! nothing touches it; an operation that moves or reshapes one frees it, and says so in its
//! result so the person can be told. Arc edges are free, since a wall is straight.

use arc_base::ObjectId;

use crate::floor::{
    Association, Boundary, Face, Floor, MAX_BULGE, MAX_LOOPS, MAX_VERTICES, MIN_BULGE, Refusal,
    Vertex,
};
use crate::wall::Wall;

type P = [f64; 2];

/// A sketch of a floor boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct Sketch {
    /// The vertices of all loops, loop by loop, as in the floor record.
    pub vertices: Vec<Vertex>,
}

/// What an operation did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Changed {
    /// How many edges that followed a wall are free now.
    pub freed: usize,
}

/// The index range of the loop of vertex `at`.
fn loop_of(v: &[Vertex], at: usize) -> Option<std::ops::Range<usize>> {
    let l = v.get(at)?.loop_index;
    let start = v.iter().position(|x| x.loop_index == l)?;
    let end = v.iter().rposition(|x| x.loop_index == l)? + 1;
    Some(start..end)
}

/// The bulge of the arc from `a` to `b` that passes through `via`; 0 when the three points are
/// in line, `None` when the arc is too wide to store or the end points coincide.
#[must_use]
pub fn bulge_through(a: P, via: P, b: P) -> Option<f64> {
    let turn = (via[0] - a[0]) * (b[1] - via[1]) - (via[1] - a[1]) * (b[0] - via[0]);
    let chord = (b[0] - a[0]).hypot(b[1] - a[1]);
    if chord <= 0.0 {
        return None;
    }
    if turn.abs() < 1.0e-9 * chord {
        return Some(0.0);
    }
    // The circumcenter.
    let (bx, by) = (via[0] - a[0], via[1] - a[1]);
    let (cx, cy) = (b[0] - a[0], b[1] - a[1]);
    let d = 2.0 * (bx * cy - by * cx);
    let ux = (cy * (bx * bx + by * by) - by * (cx * cx + cy * cy)) / d;
    let uy = (bx * (cx * cx + cy * cy) - cx * (bx * bx + by * by)) / d;
    let center = [a[0] + ux, a[1] + uy];
    let angle = |p: P| (p[1] - center[1]).atan2(p[0] - center[0]);
    let tau = std::f64::consts::TAU;
    let sweep = if turn > 0.0 {
        (angle(b) - angle(a)).rem_euclid(tau)
    } else {
        -(angle(a) - angle(b)).rem_euclid(tau)
    };
    let bulge = (sweep / 4.0).tan();
    (bulge.is_finite() && bulge.abs() <= MAX_BULGE).then_some(bulge)
}

impl Sketch {
    /// The sketch of `floor` with the points of its boundary `boundary` (derived from the walls).
    #[must_use]
    pub fn of(floor: &Floor, boundary: &Boundary) -> Self {
        let mut vertices = floor.vertices.clone();
        let (mut current, mut k) = (0_u32, 0_usize);
        for v in &mut vertices {
            if v.loop_index != current {
                current = v.loop_index;
                k = 0;
            }
            if let Some(p) = boundary
                .loops
                .get(v.loop_index as usize)
                .and_then(|l| l.get(k))
            {
                v.point = *p;
            }
            k += 1;
        }
        Self { vertices }
    }

    fn free(&mut self, edge: usize) -> usize {
        match self.vertices.get_mut(edge) {
            Some(v) if v.edge.is_some() => {
                v.edge = None;
                1
            }
            _ => 0,
        }
    }

    /// The vertex before `at` in its loop.
    fn before(&self, at: usize) -> Option<usize> {
        let r = loop_of(&self.vertices, at)?;
        Some(if at == r.start { r.end - 1 } else { at - 1 })
    }

    /// The vertex after `at` in its loop.
    fn after(&self, at: usize) -> Option<usize> {
        let r = loop_of(&self.vertices, at)?;
        Some(if at + 1 == r.end { r.start } else { at + 1 })
    }

    /// Moves vertex `at` to `to`; the two edges at it become free.
    pub fn move_vertex(&mut self, at: usize, to: P) -> Changed {
        let (Some(b), true) = (self.before(at), at < self.vertices.len()) else {
            return Changed::default();
        };
        if let Some(v) = self.vertices.get_mut(at) {
            v.point = to;
        }
        Changed {
            freed: self.free(at) + self.free(b),
        }
    }

    /// Moves the edge that starts at vertex `at` by `by`: both its vertices move, and it and the
    /// edges before and after it become free.
    pub fn move_edge(&mut self, at: usize, by: P) -> Changed {
        let (Some(b), Some(n)) = (self.before(at), self.after(at)) else {
            return Changed::default();
        };
        for k in [at, n] {
            if let Some(v) = self.vertices.get_mut(k) {
                v.point = [v.point[0] + by[0], v.point[1] + by[1]];
            }
        }
        Changed {
            freed: self.free(at) + self.free(b) + self.free(n),
        }
    }

    /// Adds a vertex at `to` on the edge that starts at vertex `at`, which splits it in two. A
    /// straight edge stays what it was (a followed wall stays followed); an arc becomes two arcs
    /// of its circle. Returns the index of the new vertex.
    pub fn insert_vertex(&mut self, at: usize, to: P) -> Option<usize> {
        if self.vertices.len() >= MAX_VERTICES {
            return None;
        }
        let next = self.vertices.get(self.after(at)?)?.point;
        let v = *self.vertices.get(at)?;
        let (first, second) = if v.bulge.abs() >= MIN_BULGE {
            // The arc through the three points splits into the arcs through `to`.
            split_arc(v.point, next, v.bulge, to)?
        } else {
            (0.0, 0.0)
        };
        let new = Vertex {
            loop_index: v.loop_index,
            point: to,
            edge: v.edge,
            bulge: second,
        };
        self.vertices.get_mut(at)?.bulge = first;
        self.vertices.insert(at + 1, new);
        Some(at + 1)
    }

    /// Deletes vertex `at`; the edge before it then runs to the vertex after it, free. Refused
    /// (`false`) when the loop would be left with fewer than three vertices.
    pub fn delete_vertex(&mut self, at: usize) -> Option<Changed> {
        let r = loop_of(&self.vertices, at)?;
        if r.len() <= 3 {
            return None;
        }
        let b = self.before(at)?;
        let mut changed = Changed::default();
        // The edge that ran to the deleted vertex now runs past it: it is no longer its wall's.
        changed.freed += self.free(b);
        if let Some(v) = self.vertices.get_mut(b) {
            v.bulge = 0.0;
        }
        self.vertices.remove(at);
        Some(changed)
    }

    /// Makes the edge that starts at vertex `at` the arc through `via`, free. A `via` in line with
    /// the edge makes it straight.
    pub fn set_arc_through(&mut self, at: usize, via: P) -> Option<Changed> {
        let next = self.vertices.get(self.after(at)?)?.point;
        let a = self.vertices.get(at)?.point;
        let bulge = bulge_through(a, via, next)?;
        let freed = self.free(at);
        self.vertices.get_mut(at)?.bulge = bulge;
        Some(Changed { freed })
    }

    /// Makes the edge that starts at vertex `at` follow `wall` at `face` and `offset`; its
    /// vertices are then derived from the walls.
    pub fn follow_wall(&mut self, at: usize, wall: ObjectId, face: Face, offset: f64) -> bool {
        match self.vertices.get_mut(at) {
            Some(v) => {
                v.edge = Some(Association { wall, face, offset });
                v.bulge = 0.0;
                true
            }
            None => false,
        }
    }

    /// The floor's vertices, ready to store.
    #[must_use]
    pub fn floor(&self, base: &Floor) -> Floor {
        Floor {
            vertices: self.vertices.clone(),
            ..base.clone()
        }
    }

    /// The boundary of the sketch with its walls read through `wall_of`.
    #[must_use]
    pub fn boundary(
        &self,
        base: &Floor,
        wall_of: &dyn Fn(ObjectId) -> Option<Wall>,
    ) -> Option<Boundary> {
        self.floor(base).boundary(wall_of)
    }

    /// Checks the sketch: closed loops (always, by construction), no crossing, openings inside the
    /// outline.
    ///
    /// # Errors
    /// The [`Refusal`] that names what is wrong.
    pub fn validate(
        &self,
        base: &Floor,
        wall_of: &dyn Fn(ObjectId) -> Option<Wall>,
    ) -> Result<Boundary, Refusal> {
        let loops = self
            .vertices
            .last()
            .map_or(0, |v| v.loop_index as usize + 1);
        if loops > MAX_LOOPS || self.vertices.len() > MAX_VERTICES {
            return Err(Refusal::Malformed);
        }
        let b = self.boundary(base, wall_of).ok_or(Refusal::Malformed)?;
        b.check()?;
        Ok(b)
    }
}

/// The bulges of the two arcs that the arc from `a` to `b` with `bulge` splits into at `at`;
/// `None` when `at` is not on the arc's circle within a millimeter or the arc is straight.
fn split_arc(a: P, b: P, bulge: f64, at: P) -> Option<(f64, f64)> {
    let (center, sweep, radius) = crate::floor::arc_geometry(a, b, bulge)?;
    if ((at[0] - center[0]).hypot(at[1] - center[1]) - radius).abs() > 1.0e-3 {
        return None;
    }
    let angle = |p: P| (p[1] - center[1]).atan2(p[0] - center[0]);
    // The angle from `a` to `at` in the direction of the sweep, within the sweep.
    let mut first = (angle(at) - angle(a)) * sweep.signum();
    first = first.rem_euclid(std::f64::consts::TAU);
    if first >= sweep.abs() {
        return None;
    }
    let second = sweep.abs() - first;
    let bulge_of = |s: f64| (s / 4.0).tan();
    Some((
        bulge_of(first * sweep.signum()),
        bulge_of(second * sweep.signum()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> (Floor, Sketch) {
        let vertices: Vec<Vertex> = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]
            .iter()
            .map(|p| Vertex {
                loop_index: 0,
                point: *p,
                edge: None,
                bulge: 0.0,
            })
            .collect();
        let floor = Floor {
            thickness: 0.2,
            top_offset: 0.0,
            vertices: vertices.clone(),
        };
        (floor, Sketch { vertices })
    }

    #[test]
    fn a_three_point_arc_has_the_bulge_of_its_circle() {
        // A half circle over the chord from (0,0) to (2,0) through (1,-1): counterclockwise.
        let b = bulge_through([0.0, 0.0], [1.0, -1.0], [2.0, 0.0]).unwrap();
        assert!((b - 1.0).abs() < 1e-9, "{b}");
        let b = bulge_through([0.0, 0.0], [1.0, 1.0], [2.0, 0.0]).unwrap();
        assert!((b + 1.0).abs() < 1e-9, "{b}");
        assert_eq!(bulge_through([0.0, 0.0], [1.0, 0.0], [2.0, 0.0]), Some(0.0));
        // A quarter turn of radius 1: the point at 45 degrees.
        let q = bulge_through(
            [1.0, 0.0],
            [
                std::f64::consts::FRAC_1_SQRT_2,
                std::f64::consts::FRAC_1_SQRT_2,
            ],
            [0.0, 1.0],
        )
        .unwrap();
        assert!(
            (q - (std::f64::consts::FRAC_PI_2 / 4.0).tan()).abs() < 1e-9,
            "{q}"
        );
    }

    #[test]
    fn edits_free_wall_edges_and_the_checks_refuse_a_bad_loop() {
        let (floor, mut s) = square();
        use arc_base::id::Identifier;
        let wall = ObjectId::from_u128(9);
        assert!(s.follow_wall(0, wall, Face::Interior, 0.0));
        let changed = s.move_vertex(1, [5.0, -1.0]);
        assert_eq!(changed.freed, 1);
        assert!(s.set_arc_through(1, [5.5, 2.0]).is_some());
        let none = |_: ObjectId| None;
        assert!(s.validate(&floor, &none).is_ok());
        // A vertex dragged across the opposite edge crosses the loop.
        s.move_vertex(2, [-1.0, -1.0]);
        assert_eq!(s.validate(&floor, &none), Err(Refusal::SelfIntersecting));
        // A loop of three vertices keeps them.
        let (_, mut t) = square();
        assert!(t.delete_vertex(1).is_some());
        assert!(t.delete_vertex(1).is_none());
    }

    #[test]
    fn inserting_a_vertex_on_an_arc_splits_it_on_its_circle() {
        let (_, mut s) = square();
        s.set_arc_through(0, [2.0, -1.0]);
        let b = s.vertices[0].bulge;
        let (center, sweep, r) = crate::floor::arc_geometry([0.0, 0.0], [4.0, 0.0], b).unwrap();
        let top = [center[0], center[1] - r];
        let at = s.insert_vertex(0, top).unwrap();
        assert_eq!(at, 1);
        let total = 4.0 * s.vertices[0].bulge.atan() + 4.0 * s.vertices[1].bulge.atan();
        assert!((total - sweep).abs() < 1e-9, "{total} {sweep}");
    }
}
