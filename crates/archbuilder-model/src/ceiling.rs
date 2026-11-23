//! Ceilings: a boundary on a storey (the wall faces of a room, or a sketch with arcs), the height
//! of its underside above the level, a layered type with an optional grid pattern, and optionally
//! a slope given by an arrow.
//!
//! The boundary is the one of a floor ([`crate::floor::Floor`]): the same vertex and arc records,
//! so the same readers, the same association with wall faces and the same sketch editing apply.
//! The ceiling is drawn above the plan's cut plane. Its body lies above its underside: the
//! thickness of the type, measured vertically. A slope arrow is a start line (through a point,
//! square to the arrow), a direction and either a slope angle or the height of the underside at
//! the arrow's end; the underside is the plane through the start line at the ceiling's height.
//!
//! Shafts and vertical openings that pass through the ceiling are taken out of it
//! ([`crate::opening`]). The derived result is kept per ceiling while the objects it reads are
//! unchanged ([`derived`]).

use std::sync::{Arc, Mutex};

use arc_base::math::Point3;
use arc_base::{Limits, ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::Polyhedron;

use crate::cleanup::{Deps, Store, cached};
use crate::element_type::{CeilingType, Grid, TypeKind, type_of};
use crate::floor::{Boundary, FLAT_CHORD, Floor};
use crate::spatial;
use crate::terrain::{Piece, Plane};
use crate::types::{self, ceiling};

/// The steepest slope of a ceiling: 85 degrees.
pub const MAX_SLOPE: f64 = 1.483_529_864_195_180_3;
/// The thickness of a ceiling without a type, in meters.
pub const DEFAULT_THICKNESS: f64 = 0.02;
/// The most grid lines of one direction drawn; a ceiling that needs more draws no pattern.
pub const MAX_GRID_LINES: usize = 2_000;
/// The least slope that counts as a slope: a gradient below it is flat.
const FLAT: f64 = 1.0e-9;

/// How the arrow gives the rise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rise {
    /// The slope angle.
    Angle(f64),
    /// The height of the underside above the level at the arrow's end, in meters.
    EndHeight(f64),
}

/// A slope arrow: the start line passes through `start` square to `direction`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrow {
    /// A point of the start line.
    pub start: [f64; 2],
    /// The direction the underside rises in, an angle from the x axis.
    pub direction: f64,
    /// The length of the arrow, in meters.
    pub length: f64,
    /// The rise.
    pub rise: Rise,
}

impl Arrow {
    /// The rise per meter along the arrow, for a ceiling whose underside is `height` above its
    /// level at the start line.
    #[must_use]
    pub fn gradient(&self, height: f64) -> f64 {
        match self.rise {
            Rise::Angle(a) => a.tan(),
            Rise::EndHeight(end) => (end - height) / self.length.max(1.0e-9),
        }
    }

    /// The arrow's end.
    #[must_use]
    pub fn end(&self) -> [f64; 2] {
        [
            self.start[0] + self.direction.cos() * self.length,
            self.start[1] + self.direction.sin() * self.length,
        ]
    }
}

/// The authored data of a ceiling, lengths in meters.
#[derive(Debug, Clone, PartialEq)]
pub struct Ceiling {
    /// The boundary data: `thickness` is the body's, `top_offset` the underside's height above
    /// the level.
    pub shape: Floor,
    /// The grid pattern's origin.
    pub grid_origin: [f64; 2],
    /// The grid pattern's rotation, added to the angle of the type's pattern.
    pub grid_rotation: f64,
    /// The slope arrow; `None` for a flat ceiling.
    pub arrow: Option<Arrow>,
}

fn angle_of(values: &[Option<Value>], at: usize) -> Option<f64> {
    match values.get(at) {
        Some(Some(Value::Quantity(q))) => Some(q.value()),
        _ => None,
    }
}

impl Ceiling {
    /// The ceiling of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let shape = Floor::from_values(values)?;
        let m = |at| types::meters(values, at);
        let arrow = match (m(ceiling::SLOPE_X), m(ceiling::SLOPE_Y)) {
            (Some(x), Some(y)) => {
                let rise = match (angle_of(values, ceiling::SLOPE), m(ceiling::END_HEIGHT)) {
                    (Some(a), _) => Some(Rise::Angle(a)),
                    (None, Some(h)) => Some(Rise::EndHeight(h)),
                    (None, None) => None,
                };
                rise.map(|rise| Arrow {
                    start: [x, y],
                    direction: angle_of(values, ceiling::SLOPE_DIRECTION).unwrap_or(0.0),
                    length: m(ceiling::SLOPE_LENGTH).unwrap_or(1.0).max(0.001),
                    rise,
                })
            }
            _ => None,
        };
        Some(Self {
            shape,
            grid_origin: [
                m(ceiling::GRID_X).unwrap_or(0.0),
                m(ceiling::GRID_Y).unwrap_or(0.0),
            ],
            grid_rotation: angle_of(values, ceiling::GRID_ROTATION).unwrap_or(0.0),
            arrow,
        })
    }

    /// The height of the underside above the level at the slope's start line.
    #[must_use]
    pub fn height(&self) -> f64 {
        self.shape.top_offset
    }

    /// The body's thickness.
    #[must_use]
    pub fn thickness(&self) -> f64 {
        self.shape.thickness
    }

    /// The underside as a plane over the plan, for a level at `level`.
    #[must_use]
    pub fn plane(&self, level: f64) -> Plane {
        let z = level + self.height();
        match self.arrow {
            Some(a) => {
                let g = a.gradient(self.height());
                Plane {
                    at: a.start,
                    z,
                    grad: [g * a.direction.cos(), g * a.direction.sin()],
                }
            }
            None => Plane::level(z),
        }
    }

    /// Whether the slope arrow's slope is within [`MAX_SLOPE`] (a flat or arrow-less ceiling is).
    #[must_use]
    pub fn slope_ok(&self) -> bool {
        self.arrow.is_none_or(|a| {
            let g = a.gradient(self.height());
            g.is_finite() && g.atan().abs() <= MAX_SLOPE + 1.0e-9
        })
    }

    /// Whether the underside slopes.
    #[must_use]
    pub fn sloped(&self) -> bool {
        self.arrow
            .is_some_and(|a| a.gradient(self.height()).abs() > FLAT)
    }
}

/// What a ceiling reads from a snapshot: its data, its underside and its boundary.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The ceiling.
    pub id: ObjectId,
    /// The authored data.
    pub ceiling: Ceiling,
    /// The elevation of its storey's level.
    pub level: f64,
    /// The underside.
    pub plane: Plane,
    /// The grid pattern of its type, with the angle of this ceiling.
    pub grid: Option<Grid>,
    /// The boundary derived from the walls, before openings are taken out.
    pub raw: Boundary,
    /// The boundary with the shafts and vertical openings that pass through taken out.
    pub boundary: Boundary,
    /// The footprints of those openings.
    pub cuts: Vec<Vec<[f64; 2]>>,
}

impl Derived {
    /// The area in square meters, openings taken out.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.boundary.area()
    }

    /// The volume of the body: the area times the thickness.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.area() * self.ceiling.thickness()
    }

    /// The elevation of the underside over `p`.
    #[must_use]
    pub fn underside_at(&self, p: [f64; 2]) -> f64 {
        self.plane.z(p)
    }

    /// The lowest and the highest elevation of the body.
    #[must_use]
    pub fn z_range(&self) -> (f64, f64) {
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for p in self.raw.loops.iter().flatten() {
            let z = self.plane.z(*p);
            lo = lo.min(z);
            hi = hi.max(z);
        }
        if lo > hi {
            let z = self.plane.z;
            (lo, hi) = (z, z);
        }
        (lo, hi + self.ceiling.thickness())
    }

    /// The boundary as a region of the solid kernel.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn region(&self) -> Result<arc_geom::Region> {
        self.boundary.region()
    }

    /// The underside as convex pieces over the boundary, each with its plane.
    #[must_use]
    pub fn pieces(&self) -> Vec<Piece> {
        self.boundary.pieces(self.plane)
    }

    /// The body: a prism over the boundary, sheared along the slope when there is one.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn solid(&self) -> Result<Polyhedron> {
        let thickness = self.ceiling.thickness();
        if !self.ceiling.sloped() {
            return self.boundary.solid(self.plane.z + thickness, thickness);
        }
        let flat = self
            .boundary
            .faceted(FLAT_CHORD)
            .solid(thickness, thickness)?;
        // A slope shears the flat body (openings already out of its boundary): planes stay planes
        // and the volume stays, so no cut meets a seam between pieces.
        let vertices = flat
            .vertices()
            .iter()
            .map(|v| Point3::new(v.x, v.y, v.z + self.plane.z([v.x, v.y])))
            .collect();
        Polyhedron::new(
            vertices,
            flat.faces().to_vec(),
            &Tolerance::DEFAULT,
            &Limits::DEFAULT,
        )
    }

    /// The lines of the grid pattern inside the boundary, as segments; empty for a plain ceiling.
    /// The lines of each direction are as many as the extent and the tile size make, never more
    /// than [`MAX_GRID_LINES`].
    #[must_use]
    pub fn grid_lines(&self) -> Vec<[[f64; 2]; 2]> {
        let Some(grid) = self.grid else {
            return Vec::new();
        };
        let loops = self.boundary.flattened(FLAT_CHORD);
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in loops.iter().flatten() {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        let origin = self.ceiling.grid_origin;
        let mut out = Vec::new();
        // The lines along the first axis are one tile's second size apart, and the other way.
        for (angle, pitch) in [
            (grid.angle, grid.spacing[1]),
            (grid.angle + std::f64::consts::FRAC_PI_2, grid.spacing[0]),
        ] {
            let (dir, normal) = ([angle.cos(), angle.sin()], [-angle.sin(), angle.cos()]);
            let along = |p: [f64; 2], v: [f64; 2]| p[0] * v[0] + p[1] * v[1];
            let corners = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]];
            let (nlo, nhi) = corners.iter().fold((f64::MAX, f64::MIN), |(a, b), c| {
                let n = along(*c, normal);
                (a.min(n), b.max(n))
            });
            let n0 = along(origin, normal);
            let first = ((nlo - n0) / pitch).ceil();
            let last = ((nhi - n0) / pitch).floor();
            if !(first.is_finite() && last.is_finite()) || last - first >= MAX_GRID_LINES as f64 {
                continue;
            }
            let mut k = first;
            while k <= last {
                let offset = n0 + k * pitch;
                k += 1.0;
                let mut hits: Vec<f64> = Vec::new();
                for l in &loops {
                    for (a, b) in l.iter().zip(l.iter().cycle().skip(1)) {
                        let (da, db) = (along(*a, normal) - offset, along(*b, normal) - offset);
                        if (da >= 0.0) != (db >= 0.0) {
                            let t = da / (da - db);
                            let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                            hits.push(along(p, dir));
                        }
                    }
                }
                hits.sort_by(f64::total_cmp);
                for pair in hits.chunks_exact(2) {
                    let (Some(&s0), Some(&s1)) = (pair.first(), pair.get(1)) else {
                        continue;
                    };
                    let at = |s: f64| {
                        [
                            normal[0] * offset + dir[0] * s,
                            normal[1] * offset + dir[1] * s,
                        ]
                    };
                    if s1 - s0 > Tolerance::DEFAULT.linear {
                        out.push([at(s0), at(s1)]);
                    }
                }
            }
        }
        out
    }
}

/// The dependencies of the derived result of ceiling `object`: everything [`derive`] reads.
fn deps(snapshot: &Snapshot, object: &Arc<Object>) -> Deps {
    let mut deps: Deps = vec![Arc::clone(object)];
    if let Some(storey) = object.header.owner.and_then(|s| spatial::get(snapshot, s)) {
        deps.extend(spatial::level_of(snapshot, &storey));
        deps.push(storey);
    }
    if let Some(c) = Ceiling::from_values(&object.values) {
        deps.extend(
            c.shape
                .walls()
                .into_iter()
                .filter_map(|w| spatial::get(snapshot, w)),
        );
    }
    deps.extend(type_of(snapshot, TypeKind::Ceiling, &object.values));
    deps.extend(spatial::children(
        snapshot,
        types::VERTICAL_OPENING,
        object.id,
    ));
    crate::opening::shaft_deps(snapshot, &mut deps);
    deps
}

fn derive(snapshot: &Snapshot, object: &Object) -> Option<Derived> {
    let ceiling = Ceiling::from_values(&object.values)?;
    let level = object
        .header
        .owner
        .and_then(|id| spatial::get(snapshot, id))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    let plane = ceiling.plane(level);
    let raw = ceiling.shape.boundary_in(snapshot)?;
    let grid = type_of(snapshot, TypeKind::Ceiling, &object.values)
        .and_then(|t| CeilingType::from_values(&t.values))
        .and_then(|t| t.grid)
        .map(|g| Grid {
            angle: g.angle + ceiling.grid_rotation,
            ..g
        });
    let mut derived = Derived {
        id: object.id,
        ceiling,
        level,
        plane,
        grid,
        boundary: raw.clone(),
        raw,
        cuts: Vec::new(),
    };
    let z = derived.z_range();
    derived.cuts = crate::opening::cuts(snapshot, object, z);
    derived.boundary = derived.raw.with_openings(&derived.cuts).0;
    Some(derived)
}

static CEILINGS: Store<Option<Arc<Derived>>> = Mutex::new(Vec::new());

/// The derived result of the ceiling `object`; `None` when the record or its edges are
/// unreadable. Computed again only when the ceiling, the walls its edges follow, its type, its
/// storey and level, its vertical openings or the shafts changed: a room renamed, a floor edited
/// or a ceiling of another storey finds the same result.
#[must_use]
pub fn derived(snapshot: &Snapshot, object: &Arc<Object>) -> Option<Arc<Derived>> {
    let stored = cached(&CEILINGS, object.id, 0, deps(snapshot, object), || {
        derive(snapshot, object).map(Arc::new)
    });
    stored.as_ref().clone()
}

/// The ceilings of `storey`, derived, in identifier order.
#[must_use]
pub fn of_storey(snapshot: &Snapshot, storey: ObjectId) -> Vec<Arc<Derived>> {
    spatial::children(snapshot, types::CEILING, storey)
        .iter()
        .take(crate::room::MAX_ROOMS)
        .filter_map(|o| derived(snapshot, o))
        .collect()
}
