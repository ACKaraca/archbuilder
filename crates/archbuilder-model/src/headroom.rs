//! The headroom check of a stair: the clear height between the walking surface and what is above
//! it, along each run and landing.
//!
//! The walking surface is sampled at the middle and near both edges of every tread and at the
//! corners and the middle of every landing. Above each sample the check looks at the floors (their
//! undersides, with the shaft openings already cut out of them, so the opening of the stair's own
//! top level is clear) and the roofs of the document; what lies below the walking surface is
//! excluded. The least clearance, where it is and what causes it are reported; nothing is stored.

use arc_base::ObjectId;
use arc_doc::Snapshot;

use crate::spatial;
use crate::stair::{Layout, Placed};
use crate::types::{FLOOR, ROOF};

/// The most floors and roofs the check reads.
const MAX_ELEMENTS: usize = 2_000;

/// The least clear height found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clearance {
    /// The clear height, in meters.
    pub clear: f64,
    /// Where it is, in plan.
    pub at: [f64; 2],
    /// The elevation of the walking surface there.
    pub z: f64,
    /// The element in the way.
    pub by: ObjectId,
}

fn samples(layout: &Layout, base: f64) -> Vec<([f64; 2], f64)> {
    let h = layout.stair.width / 2.0 * 0.9;
    let mut out = Vec::new();
    for run in &layout.runs {
        let n = [-run.dir[1], run.dir[0]];
        for k in 0..run.risers.saturating_sub(1) {
            let a = (k as f64 + 0.5) * layout.stair.tread;
            let z = base + run.base + (k + 1) as f64 * layout.riser;
            for c in [-h, 0.0, h] {
                out.push((
                    [
                        run.start[0] + run.dir[0] * a + n[0] * c,
                        run.start[1] + run.dir[1] * a + n[1] * c,
                    ],
                    z,
                ));
            }
        }
    }
    let mut landings: Vec<(&[[f64; 2]], f64)> = layout
        .landings
        .iter()
        .map(|(p, t)| (p.as_slice(), *t))
        .collect();
    landings.push((layout.landing.as_slice(), layout.landing_top));
    for (corners, top) in landings {
        if corners.len() < 3 {
            continue;
        }
        let n = corners.len() as f64;
        let c = corners
            .iter()
            .fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
        out.push((c, base + top));
        for p in corners {
            out.push((
                [c[0] + (p[0] - c[0]) * 0.9, c[1] + (p[1] - c[1]) * 0.9],
                base + top,
            ));
        }
    }
    out
}

/// The least clear height above the walking surface of `placed`; `None` for a stair without a
/// layout or with nothing above it.
#[must_use]
pub fn minimum(snapshot: &Snapshot, placed: &Placed) -> Option<Clearance> {
    let layout = placed.layout.as_ref().ok()?;
    let floors: Vec<_> = spatial::objects(snapshot, FLOOR)
        .into_iter()
        .take(MAX_ELEMENTS)
        .filter_map(|o| Some((o.id, crate::floor::Derived::of(snapshot, &o)?)))
        .collect();
    let roofs: Vec<_> = spatial::objects(snapshot, ROOF)
        .into_iter()
        .take(MAX_ELEMENTS)
        .filter_map(|o| Some((o.id, crate::roof::Derived::of(snapshot, &o).ok()?)))
        .collect();
    let mut least: Option<Clearance> = None;
    for (p, z) in samples(layout, placed.base) {
        let mut consider = |under: f64, by: ObjectId| {
            let clear = under - z;
            if clear > 1.0e-6 && least.is_none_or(|l| clear < l.clear) {
                least = Some(Clearance {
                    clear,
                    at: p,
                    z,
                    by,
                });
            }
        };
        for (id, d) in &floors {
            if d.boundary.contains(p) {
                consider(d.plane.z(p) - d.floor.thickness, *id);
            }
        }
        for (id, d) in &roofs {
            if let Some(under) = d.surface.underside(p) {
                consider(under, *id);
            }
        }
    }
    least
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stair::{Shape, Stair, Turn};

    #[test]
    fn samples_follow_the_treads_and_stay_on_the_stair() {
        let stair = Stair {
            shape: Shape::Straight,
            turn: Turn::Left,
            start: [0.0, 0.0],
            direction: 0.0,
            width: 1.0,
            tread: 0.28,
            max_riser: 0.18,
            waist: 0.15,
            railings: [true, true],
            top_level: None,
        };
        let layout = Layout::new(&stair, 3.0).unwrap();
        let s = samples(&layout, 0.0);
        // Sixteen treads, three samples each (the straight stair has no landing).
        assert_eq!(s.len(), 16 * 3);
        assert!((s[0].1 - 3.0 / 17.0).abs() < 1.0e-9);
        assert!(
            s.iter()
                .all(|(p, _)| p[0] >= 0.0 && p[0] <= 4.48 && p[1].abs() <= 0.5)
        );
    }
}
