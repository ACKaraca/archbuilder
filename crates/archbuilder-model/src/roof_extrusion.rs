//! The roof by extrusion: an open profile sketched in a vertical plane, extruded along the plane's
//! normal between a start and an end distance, thickened normal to the profile.
//!
//! The record holds authored data only: the plane (an origin on the storey and the direction of its
//! horizontal axis), the profile as points `(u, z)` (`u` along the axis, `z` above the storey's
//! level), the start and end distances along the plane's normal (the axis turned a quarter
//! counterclockwise), the thickness and the roof type. The profile is the roof's top surface. The
//! body is derived each time ([`Derived::of`]): the profile offset downward, normal to each
//! segment and mitered at the corners by the thickness (the total of the type's layers when the
//! roof has a type, the stored one else), closed into a section and extruded. Its underside, the
//! offset profile, bounds the rooms under it ([`Derived::pieces`]). The side a roof is thickened
//! to is the lower one: right of the profile's direction when it runs toward +u, left otherwise.

use arc_base::math::Point3;
use arc_base::{Limits, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::{PolyFace, Polyhedron};

use crate::floor::{Boundary, signed_area};
use crate::terrain::{Piece, Plane};
use crate::types::{self, roof_extrusion as f};

/// The most points of a profile.
pub const MAX_PROFILE: usize = 128;
/// The range of a profile's `u`, in meters.
pub const PROFILE_RANGE: (f64, f64) = (-1000.0, 1000.0);
/// The range of the start and end distances, in meters.
pub const SPAN_RANGE: (f64, f64) = (-1000.0, 1000.0);
/// The longest extrusion, in meters.
pub const MAX_SPAN: f64 = 500.0;
/// The shortest extrusion, in meters.
pub const MIN_SPAN: f64 = 0.01;
/// The longest miter at a corner, as a multiple of the thickness: sharper corners are cut square.
const MAX_MITER: f64 = 8.0;

/// The authored data of a roof by extrusion, lengths in meters.
#[derive(Debug, Clone, PartialEq)]
pub struct RoofExtrusion {
    /// The thickness stored (a copy of the type's total).
    pub thickness: f64,
    /// The start distance.
    pub start: f64,
    /// The end distance.
    pub end: f64,
    /// The direction of the plane's axis, an angle from the x axis.
    pub direction: f64,
    /// The plane's origin.
    pub origin: [f64; 2],
    /// The profile `(u, z)`.
    pub profile: Vec<[f64; 2]>,
}

impl RoofExtrusion {
    /// The roof of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let m = |at| types::meters(values, at);
        let Some(Some(Value::List(points))) = values.get(f::PROFILE) else {
            return None;
        };
        if points.len() > MAX_PROFILE {
            return None;
        }
        let profile = points
            .iter()
            .map(|p| match p {
                Value::Record(r) => Some([types::meters(r, 0)?, types::meters(r, 1)?]),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            thickness: m(f::THICKNESS)?,
            start: m(f::START)?,
            end: m(f::END)?,
            direction: match values.get(f::DIRECTION) {
                Some(Some(Value::Quantity(q))) => q.value(),
                _ => 0.0,
            },
            origin: [m(f::X)?, m(f::Y)?],
            profile,
        })
    }

    /// The record's value of a profile.
    #[must_use]
    pub fn profile_value(profile: &[[f64; 2]]) -> Value {
        Value::List(
            profile
                .iter()
                .map(|p| Value::Record(vec![Some(types::length(p[0])), Some(types::length(p[1]))]))
                .collect(),
        )
    }

    /// The roof with its plane taken from the vertical work plane it was sketched on, when its
    /// record names one that resolves: moving the work plane's datum moves the roof. Any other
    /// plane (none, gone, not vertical) leaves the stored one.
    #[must_use]
    pub fn on_work_plane(self, snapshot: &Snapshot, values: &[Option<Value>]) -> Self {
        let plane = match values.get(f::WORK_PLANE) {
            Some(Some(Value::Reference(id))) => {
                crate::spatial::get(snapshot, *id).and_then(|w| crate::datum::resolve(snapshot, &w))
            }
            _ => None,
        };
        match plane {
            Some(p) if p.normal[2].abs() < 1.0e-9 => Self {
                origin: [p.origin[0], p.origin[1]],
                direction: p.u[1].atan2(p.u[0]),
                ..self
            },
            _ => self,
        }
    }

    /// The plane's axis and normal as unit vectors in plan.
    #[must_use]
    pub fn axes(&self) -> ([f64; 2], [f64; 2]) {
        let (s, c) = self.direction.sin_cos();
        ([c, s], [-s, c])
    }
}

/// The thickness of the roof `object`: the total of its type's layers, else the stored one.
#[must_use]
pub fn thickness_of(snapshot: &Snapshot, object: &Object, stored: f64) -> f64 {
    crate::element_type::type_of(
        snapshot,
        crate::element_type::TypeKind::Roof,
        &object.values,
    )
    .and_then(|t| crate::element_type::Layers::from_values(&t.values))
    .map_or(stored, |l| l.total())
}

/// Why a roof by extrusion has no body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// A field is missing or out of its bounds.
    Malformed,
    /// The profile has fewer than two points, a segment shorter than the tolerance or a point
    /// out of range.
    Profile,
    /// The section (the profile with its thickness) crosses itself: the thickness is too large
    /// for the profile's bends.
    Section,
    /// The extrusion is shorter than [`MIN_SPAN`] or longer than [`MAX_SPAN`].
    Span,
}

/// What a roof by extrusion reads from a snapshot.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The authored data.
    pub roof: RoofExtrusion,
    /// The elevation of the storey's level.
    pub level: f64,
    /// The thickness in force, normal to the profile.
    pub thickness: f64,
    /// The offset profile: the underside, `(u, z)` relative to the level.
    pub under: Vec<[f64; 2]>,
}

/// The miter vector at a profile point between the unit normals `a` and `b`: the offset that is
/// one unit square to both segments; the first normal when the corner is too sharp.
fn miter(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let c = 1.0 + a[0] * b[0] + a[1] * b[1];
    if c > 2.0 / (MAX_MITER * MAX_MITER) {
        [(a[0] + b[0]) / c, (a[1] + b[1]) / c]
    } else {
        a
    }
}

/// The offset of `profile` by `t` toward its lower side, mitered at the corners; `None` for a
/// profile of fewer than two points or a segment of no length.
fn offset(profile: &[[f64; 2]], t: f64) -> Option<Vec<[f64; 2]>> {
    let mut normals = Vec::with_capacity(profile.len());
    let rightward = profile
        .windows(2)
        .find(|w| (w[1][0] - w[0][0]).abs() > 1.0e-9)
        .is_none_or(|w| w[1][0] > w[0][0]);
    for w in profile.windows(2) {
        let (du, dz) = (w[1][0] - w[0][0], w[1][1] - w[0][1]);
        let l = du.hypot(dz);
        if l < types::MIN_WALL_LENGTH {
            return None;
        }
        let right = [dz / l, -du / l];
        normals.push(if rightward {
            right
        } else {
            [-right[0], -right[1]]
        });
    }
    let n = profile.len();
    (n >= 2).then(|| {
        (0..n)
            .map(|k| {
                let m = match (
                    k.checked_sub(1).and_then(|j| normals.get(j)),
                    normals.get(k),
                ) {
                    (Some(a), Some(b)) => miter(*a, *b),
                    (Some(a), None) => *a,
                    (None, Some(b)) => *b,
                    (None, None) => [0.0, 0.0],
                };
                [profile[k][0] + m[0] * t, profile[k][1] + m[1] * t]
            })
            .collect()
    })
}

impl Derived {
    /// Reads the roof `object` of `snapshot`.
    ///
    /// # Errors
    /// The [`Refusal`] that says why the roof has no body.
    pub fn of(snapshot: &Snapshot, object: &Object) -> std::result::Result<Self, Refusal> {
        let roof = RoofExtrusion::from_values(&object.values)
            .ok_or(Refusal::Malformed)?
            .on_work_plane(snapshot, &object.values);
        let level = object
            .header
            .owner
            .and_then(|s| crate::spatial::get(snapshot, s))
            .map_or(0.0, |s| crate::spatial::storey_elevation(snapshot, &s));
        let span = roof.end - roof.start;
        if !span.is_finite() || !(MIN_SPAN..=MAX_SPAN).contains(&span) {
            return Err(Refusal::Span);
        }
        if roof.profile.len() < 2
            || roof
                .profile
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > PROFILE_RANGE.1)
        {
            return Err(Refusal::Profile);
        }
        let thickness = thickness_of(snapshot, object, roof.thickness);
        let under = offset(&roof.profile, thickness).ok_or(Refusal::Profile)?;
        let derived = Self {
            roof,
            level,
            thickness,
            under,
        };
        if Boundary::polygons(vec![derived.section()]).check().is_err() {
            return Err(Refusal::Section);
        }
        Ok(derived)
    }

    /// The closed section in `(u, z)`: the profile, then the underside back.
    #[must_use]
    pub fn section(&self) -> Vec<[f64; 2]> {
        self.roof
            .profile
            .iter()
            .chain(self.under.iter().rev())
            .copied()
            .collect()
    }

    /// The area of the section, in square meters.
    #[must_use]
    pub fn section_area(&self) -> f64 {
        signed_area(&self.section()).abs()
    }

    /// The length of the extrusion, in meters.
    #[must_use]
    pub fn span(&self) -> f64 {
        self.roof.end - self.roof.start
    }

    /// The volume of the body: the section's area times the extrusion.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.section_area() * self.span()
    }

    /// The area of the top surface, in square meters: the profile's length times the extrusion.
    #[must_use]
    pub fn surface_area(&self) -> f64 {
        let length: f64 = self
            .roof
            .profile
            .windows(2)
            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            .sum();
        length * self.span()
    }

    /// The point of the plane `(u, z)` at the distance `d` along the normal, in the world; `z` is
    /// relative to the level.
    #[must_use]
    pub fn world(&self, u: f64, z: f64, d: f64) -> Point3 {
        let (ax, nm) = self.roof.axes();
        Point3::new(
            self.roof.origin[0] + ax[0] * u + nm[0] * d,
            self.roof.origin[1] + ax[1] * u + nm[1] * d,
            self.level + z,
        )
    }

    /// The lowest and the highest elevation of the body.
    #[must_use]
    pub fn z_range(&self) -> (f64, f64) {
        let zs = self.section().into_iter().map(|p| self.level + p[1]);
        (
            zs.clone().fold(f64::MAX, f64::min),
            zs.fold(f64::MIN, f64::max),
        )
    }

    /// The body: the section extruded from the start to the end distance.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn solid(&self) -> Result<Polyhedron> {
        let section = self.section();
        let n = section.len();
        let mut vertices = Vec::with_capacity(2 * n);
        for d in [self.roof.start, self.roof.end] {
            vertices.extend(section.iter().map(|p| self.world(p[0], p[1], d)));
        }
        let at = |end: usize, k: usize| u32::try_from(end * n + k % n).unwrap_or(u32::MAX);
        let mut faces: Vec<Vec<u32>> = (0..n)
            .map(|k| vec![at(0, k), at(0, k + 1), at(1, k + 1), at(1, k)])
            .collect();
        faces.push((0..n).rev().map(|k| at(0, k)).collect());
        faces.push((0..n).map(|k| at(1, k)).collect());
        let build = |faces: Vec<Vec<u32>>| {
            Polyhedron::new(
                vertices.clone(),
                faces.into_iter().map(PolyFace::new).collect(),
                &Tolerance::DEFAULT,
                &Limits::DEFAULT,
            )
        };
        let solid = build(faces.clone());
        // The section may run either way round, and the axes may be mirrored: turn the shell
        // inside out when its volume came out negative.
        if solid.as_ref().is_ok_and(|s| s.volume() < 0.0) || solid.is_err() {
            return build(
                faces
                    .into_iter()
                    .map(|f| f.into_iter().rev().collect())
                    .collect(),
            );
        }
        solid
    }

    /// The underside as convex pieces over the plan (a rectangle for each segment that is not
    /// vertical), each with its plane: what bounds a room under the roof. Heights are absolute.
    #[must_use]
    pub fn pieces(&self) -> Vec<Piece> {
        let (ax, _) = self.roof.axes();
        self.under
            .windows(2)
            .filter(|w| (w[1][0] - w[0][0]).abs() > 1.0e-6)
            .map(|w| {
                let corner = |u: f64, d: f64| {
                    let p = self.world(u, 0.0, d);
                    [p.x, p.y]
                };
                let mut points = vec![
                    corner(w[0][0], self.roof.start),
                    corner(w[1][0], self.roof.start),
                    corner(w[1][0], self.roof.end),
                    corner(w[0][0], self.roof.end),
                ];
                if crate::terrain::double_area(&points) < 0.0 {
                    points.reverse();
                }
                let slope = (w[1][1] - w[0][1]) / (w[1][0] - w[0][0]);
                let at = corner(w[0][0], self.roof.start);
                Piece {
                    points,
                    plane: Plane {
                        at,
                        z: self.level + w[0][1],
                        grad: [slope * ax[0], slope * ax[1]],
                    },
                    upper: None,
                }
            })
            .collect()
    }
}
