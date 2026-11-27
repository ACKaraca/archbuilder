//! The solids of the shapes a wall can take beyond the plain prism: leaning (slanted or tapered)
//! layers, a sketched elevation profile, and bands on a face. Nothing here is stored; every
//! solid derives from the wall record, its type's layers and the openings it hosts.
//!
//! - **Leaning.** A layer polygon at the base is moved sideways at each height by
//!   [`Wall::lean_shift`]: the faces lean by their own angles and the layers between follow in
//!   proportion, so a slant (equal angles) keeps the thickness and a taper changes it. Every side
//!   of the result is planar. Joined ends keep their vertical cut at the base and lean with the
//!   faces; an arc wall does not lean.
//! - **Profile.** The profile is extruded across each layer. Ends stay square: a wall with a
//!   profile has no cleaning of its joins, and no cut by slabs or roofs.
//! - **Bands.** A projecting band is united with the layer polygon it stands on over the heights
//!   it covers; a recessed band is cut out of the layers it meets. Both stop at doors and
//!   windows in the cut solids. A band does not continue around a corner.

use arc_base::math::Point3;
use arc_base::{Limits, Result, Tolerance};
use arc_geom::{FaceTag, PolyFace, Polyhedron};

use crate::insert::Insert;
use crate::layered::Spec;
use crate::wall::{Band, BandFace, Profile, Wall};

type P = [f64; 2];

/// One solid of a shaped wall: the layer it belongs to (the layer of the face for a band), whether
/// that layer is in the core, whether it is a finish, and the solid.
pub struct Shaped {
    /// The layer, by index in the wall type's layers.
    pub layer: usize,
    /// Whether the layer is in the core.
    pub core: bool,
    /// Whether the layer is a finish.
    pub finish: bool,
    /// A part number, for the key of the piece.
    pub part: usize,
    /// The solid.
    pub solid: Polyhedron,
}

/// The prism over the polygon `points` of a leaning `wall` between the heights `z0` and `z1`
/// (elevations; `base` is the elevation of the wall's base): each corner moved sideways by
/// [`Wall::lean_shift`] at its height.
///
/// # Errors
/// The solid kernel's errors for a polygon that makes no solid.
pub fn leaning_prism(wall: &Wall, points: &[P], z0: f64, z1: f64, base: f64) -> Result<Polyhedron> {
    let mut pts: Vec<P> = points.to_vec();
    if crate::floor::signed_area(&pts) < 0.0 {
        pts.reverse();
    }
    let length = wall.length().max(f64::MIN_POSITIVE);
    let (ux, uy) = (
        (wall.end[0] - wall.start[0]) / length,
        (wall.end[1] - wall.start[1]) / length,
    );
    let (nx, ny) = (-uy, ux);
    let at = |p: P, z: f64| {
        let across = (p[0] - wall.start[0]) * nx + (p[1] - wall.start[1]) * ny;
        let s = wall.lean_shift(across, z - base);
        Point3::new(p[0] + nx * s, p[1] + ny * s, z)
    };
    let n = u32::try_from(pts.len())
        .map_err(|_| arc_base::Error::new(arc_geom::error::codes::SOLID_LIMIT_EXCEEDED))?;
    let mut vertices: Vec<Point3> = pts.iter().map(|p| at(*p, z0)).collect();
    vertices.extend(pts.iter().map(|p| at(*p, z1)));
    let mut faces = vec![
        PolyFace::new((0..n).rev().collect()).with_tag(FaceTag::Bottom),
        PolyFace::new((n..2 * n).collect()).with_tag(FaceTag::Top),
    ];
    for k in 0..n {
        let next = (k + 1) % n;
        faces.push(
            PolyFace::new(vec![k, next, next + n, k + n]).with_tag(FaceTag::Side {
                loop_index: 0,
                curve: k,
                facet: 0,
            }),
        );
    }
    Polyhedron::new(vertices, faces, &Tolerance::DEFAULT, &Limits::DEFAULT)
}

/// The layers of a wall with a profile: for each layer of `spec` with a thickness the profile
/// extruded across it, with the openings `inserts` cut when `cut_openings`.
///
/// # Errors
/// The solid kernel's errors.
pub fn profile_solids(
    wall: &Wall,
    profile: &Profile,
    level: f64,
    spec: &Spec,
    inserts: &[Insert],
    cut_openings: bool,
) -> Result<Vec<Shaped>> {
    let (_, high) = wall.extent();
    let mut at = high;
    let mut out = Vec::new();
    for (k, layer) in spec.layers.iter().enumerate() {
        if layer.thickness <= 1.0e-9 {
            continue;
        }
        let (from, to) = (at - layer.thickness, at);
        at = from;
        let mut solid = profile.solid(wall, level, (from, to))?;
        if cut_openings {
            solid = crate::insert::cut(wall, level, solid, inserts)?;
        }
        out.push(Shaped {
            layer: k,
            core: layer.core,
            finish: layer.function == crate::element_type::Function::Finish,
            part: 0,
            solid,
        });
    }
    Ok(out)
}

/// The plan rectangle `[a, b]` along the baseline and `[v0, v1]` across it of a straight wall.
pub(crate) fn rect(wall: &Wall, a: f64, b: f64, v0: f64, v1: f64) -> Vec<P> {
    let l = wall.chord().max(f64::MIN_POSITIVE);
    let u = [
        (wall.end[0] - wall.start[0]) / l,
        (wall.end[1] - wall.start[1]) / l,
    ];
    let at = |x: f64, y: f64| {
        [
            wall.start[0] + u[0] * x - u[1] * y,
            wall.start[1] + u[1] * x + u[0] * y,
        ]
    };
    vec![at(a, v0), at(b, v0), at(b, v1), at(a, v1)]
}

/// The rectangles `(along, height)` left of `whole` when `holes` are taken out of it.
fn rects_minus(
    whole: (f64, f64, f64, f64),
    holes: &[(f64, f64, f64, f64)],
) -> Vec<(f64, f64, f64, f64)> {
    let mut pieces = vec![whole];
    for h in holes {
        let mut next = Vec::new();
        for (a0, a1, z0, z1) in pieces {
            if h.1 <= a0 || h.0 >= a1 || h.3 <= z0 || h.2 >= z1 {
                next.push((a0, a1, z0, z1));
                continue;
            }
            if h.0 > a0 {
                next.push((a0, h.0, z0, z1));
            }
            if h.1 < a1 {
                next.push((h.1, a1, z0, z1));
            }
            let (m0, m1) = (a0.max(h.0), a1.min(h.1));
            if h.2 > z0 {
                next.push((m0, m1, z0, h.2));
            }
            if h.3 < z1 {
                next.push((m0, m1, h.3, z1));
            }
        }
        pieces = next;
    }
    pieces
        .into_iter()
        .filter(|r| r.1 - r.0 > 1.0e-6 && r.3 - r.2 > 1.0e-6)
        .collect()
}

/// The plan rectangles of the projecting bands of `wall` over the heights from `a` to `b` above
/// its base, each with the face it stands on: the rectangle of a horizontal band that covers the
/// whole stretch, or of a vertical band, less the stretches where an opening in `gaps` (distances
/// along the baseline) lies. A band stops at doors and windows.
#[must_use]
pub fn band_rects(
    wall: &Wall,
    bands: &[Band],
    a: f64,
    b: f64,
    gaps: &[(f64, f64)],
) -> Vec<(BandFace, Vec<P>)> {
    let (low, high) = wall.extent();
    let length = wall.length();
    let mut out = Vec::new();
    for band in bands.iter().filter(|band| band.depth > 0.0) {
        let along = if band.vertical {
            band.range(length)
        } else {
            let (s0, s1) = band.range(wall.height);
            if !(s0 <= a + 1.0e-9 && s1 >= b - 1.0e-9) {
                continue;
            }
            (0.0, length)
        };
        let (v0, v1) = match band.face {
            BandFace::Exterior => (high, high + band.depth),
            BandFace::Interior => (low - band.depth, low),
        };
        let holes: Vec<(f64, f64, f64, f64)> = gaps.iter().map(|g| (g.0, g.1, 0.0, 1.0)).collect();
        for (x0, x1, _, _) in rects_minus((along.0, along.1, 0.0, 1.0), &holes) {
            out.push((band.face, rect(wall, x0, x1, v0, v1)));
        }
    }
    out
}

/// The plan rectangles that the recessed bands of `wall` cut out of its layers over the heights
/// from `a` to `b` above the base: for a horizontal band covering the whole stretch, the strip
/// along the wall; for a vertical one, the strip over its width. The strip is as deep as the
/// band and reaches past the face.
#[must_use]
pub fn recess_cuts(wall: &Wall, bands: &[Band], a: f64, b: f64) -> Vec<Vec<P>> {
    let (low, high) = wall.extent();
    let length = wall.length();
    let pad = 1.0;
    bands
        .iter()
        .filter(|band| band.depth < 0.0)
        .filter_map(|band| {
            let d = -band.depth;
            let (v0, v1) = match band.face {
                BandFace::Exterior => (high - d, high + pad),
                BandFace::Interior => (low - pad, low + d),
            };
            if band.vertical {
                let (s0, s1) = band.range(length);
                Some(rect(wall, s0, s1, v0, v1))
            } else {
                let (s0, s1) = band.range(wall.height);
                (s0 <= a + 1.0e-9 && s1 >= b - 1.0e-9)
                    .then(|| rect(wall, -pad, length + pad, v0, v1))
            }
        })
        .collect()
}
