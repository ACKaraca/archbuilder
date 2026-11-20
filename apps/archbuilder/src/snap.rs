//! Where the pointer lands on the level plane while a wall is placed.
//!
//! Precedence, strongest first: an end point (or the origin) of the chain or of a wall; the
//! intersection of two guide lines (alignment tracking lines through end points, the direction
//! from the segment's start, a wall's baseline); the nearest point of a wall's baseline; one
//! tracking line; the direction from the segment's start; the grid. A guide line keeps its own
//! coordinate exactly, so a point on a tracking line is exactly aligned with the point that
//! tracks it, and a length along a direction is not rounded when a tracking line fixes it.
//!
//! The grid is 10 mm, and a coarser 1-2-5 step (at least four apertures wide on the screen)
//! when the pointer is within the aperture of one of its lines, so a click a few pixels off a
//! whole meter lands on it.

use crate::interaction::GRID;

/// How far from the pointer a guide or an end point attracts it, in pixels.
pub const APERTURE_PIXELS: f64 = 10.0;
/// How far from a 15 degree step the pointer direction snaps, in degrees.
pub const ANGLE_WINDOW: f64 = 4.0;
/// The most end points looked at for tracking lines.
const MAX_TRACKED: usize = 4096;

type P = [f64; 2];

/// What a snapped point lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An end point or the origin.
    Endpoint,
    /// Where two guide lines meet.
    Intersection,
    /// The nearest point of a wall's baseline.
    Nearest,
    /// A tracking line through an end point.
    Tracking,
    /// A direction from the segment's start.
    Direction,
    /// The grid.
    Grid,
}

/// A snapped point.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapped {
    /// The point, project coordinates.
    pub point: P,
    /// What it lies on.
    pub kind: Kind,
    /// The tracking lines to draw, each from the end point that tracks to the point.
    pub guides: Vec<[P; 2]>,
    /// The angle in degrees when the point is on a direction from the segment's start.
    pub degrees: Option<i32>,
}

/// What the snap reads.
pub struct Input<'a> {
    /// The pointer on the plane.
    pub pointer: P,
    /// The scale, in screen pixels per meter; `None` when unknown (nothing but exact points).
    pub pixels_per_unit: Option<f64>,
    /// The end points that attract and track: the chain's start, the walls' ends, the origin.
    pub ends: &'a [P],
    /// The baselines of the walls.
    pub walls: &'a [[P; 2]],
    /// The start of the segment being placed, when there is one.
    pub from: Option<P>,
}

/// A guide line: a point on it, its unit direction, the point that tracks it, and the angle.
#[derive(Clone, Copy)]
struct Guide {
    at: P,
    dir: P,
    tracker: Option<P>,
    degrees: Option<i32>,
}

impl Guide {
    fn axis(self) -> Option<bool> {
        // `Some(true)` for a horizontal line, `Some(false)` for a vertical one.
        if self.dir[1] == 0.0 {
            Some(true)
        } else if self.dir[0] == 0.0 {
            Some(false)
        } else {
            None
        }
    }
}

/// `to` moved onto the nearest 15 degree direction from `from` when the pointer is within the
/// window of one: the unit direction and the angle in degrees.
fn direction(from: P, to: P) -> Option<(P, i32)> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    if dx.hypot(dy) < GRID {
        return None;
    }
    let degrees = dy.atan2(dx).to_degrees();
    let step = (degrees / 15.0).round();
    if (degrees - step * 15.0).abs() > ANGLE_WINDOW {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a whole number from -12 to 12"
    )]
    let k = (step as i32).rem_euclid(24);
    let dir = match k {
        0 => [1.0, 0.0],
        6 => [0.0, 1.0],
        12 => [-1.0, 0.0],
        18 => [0.0, -1.0],
        _ => {
            let r = (f64::from(k) * 15.0).to_radians();
            [r.cos(), r.sin()]
        }
    };
    Some((dir, k * 15))
}

/// The point where two lines meet; the coordinate of an axis-aligned line is kept exactly.
fn meet(a: &Guide, b: &Guide) -> Option<P> {
    match (a.axis(), b.axis()) {
        (Some(true), Some(false)) => return Some([b.at[0], a.at[1]]),
        (Some(false), Some(true)) => return Some([a.at[0], b.at[1]]),
        (Some(x), Some(y)) if x == y => return None,
        _ => {}
    }
    let c = a.dir[0] * b.dir[1] - a.dir[1] * b.dir[0];
    if c.abs() < 1e-9 {
        return None;
    }
    let d = [b.at[0] - a.at[0], b.at[1] - a.at[1]];
    let t = (d[0] * b.dir[1] - d[1] * b.dir[0]) / c;
    let mut p = [a.at[0] + a.dir[0] * t, a.at[1] + a.dir[1] * t];
    for g in [a, b] {
        match g.axis() {
            Some(true) => p[1] = g.at[1],
            Some(false) => p[0] = g.at[0],
            None => {}
        }
    }
    Some(p)
}

/// The nearest point of the segment to `p`, with its distance, strictly inside the segment.
fn nearest_on(seg: &[P; 2], p: P) -> Option<(f64, P)> {
    let d = [seg[1][0] - seg[0][0], seg[1][1] - seg[0][1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    if len2 <= 0.0 {
        return None;
    }
    let t = ((p[0] - seg[0][0]) * d[0] + (p[1] - seg[0][1]) * d[1]) / len2;
    if !(t > 0.0 && t < 1.0) {
        return None;
    }
    let q = [seg[0][0] + d[0] * t, seg[0][1] + d[1] * t];
    Some(((p[0] - q[0]).hypot(p[1] - q[1]), q))
}

/// The coarse grid step for a scale: the smallest of 1, 2, 5, 10, ... meters that is at least
/// four apertures wide on the screen, so the grid does not catch the whole plane.
fn major_step(pixels_per_unit: f64) -> Option<f64> {
    if !(pixels_per_unit.is_finite() && pixels_per_unit > 0.0) {
        return None;
    }
    let mut decade = 1.0e-2;
    for _ in 0..12 {
        for m in [1.0, 2.0, 5.0] {
            let step = decade * m;
            if step * pixels_per_unit >= 4.0 * APERTURE_PIXELS {
                return Some(step);
            }
        }
        decade *= 10.0;
    }
    None
}

/// A coordinate on the coarse grid when within `reach` of one of its lines, else on the 10 mm
/// grid.
fn round_coordinate(v: f64, step: Option<f64>, reach: f64) -> f64 {
    if let Some(step) = step {
        let r = (v / step).round() * step;
        if (r - v).abs() <= reach {
            return r;
        }
    }
    (v / GRID).round() * GRID
}

/// Where the pointer lands; `None` for a pointer that is not a finite point.
#[allow(
    clippy::too_many_lines,
    reason = "one numbered rule after another, in order of precedence"
)]
#[must_use]
pub fn snap(input: &Input<'_>) -> Option<Snapped> {
    let p = input.pointer;
    if !(p[0].is_finite() && p[1].is_finite()) {
        return None;
    }
    let reach = input.pixels_per_unit.map_or(0.0, |s| APERTURE_PIXELS / s);
    let step = input.pixels_per_unit.and_then(major_step);
    let made = |point: P, kind: Kind| Snapped {
        point,
        kind,
        guides: Vec::new(),
        degrees: None,
    };
    // 1. An end point.
    let near = input
        .ends
        .iter()
        .map(|e| ((e[0] - p[0]).hypot(e[1] - p[1]), *e))
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, end)) = near {
        return Some(made(end, Kind::Endpoint));
    }
    // The guide lines: tracking lines through end points near the pointer, and the direction
    // from the segment's start.
    let mut guides: Vec<Guide> = Vec::new();
    let mut add = |g: Guide| {
        let same = |h: &Guide| {
            g.axis().is_some()
                && g.axis() == h.axis()
                && (g.at[0] - h.at[0]).abs() * g.dir[1].abs()
                    + (g.at[1] - h.at[1]).abs() * g.dir[0].abs()
                    < 1e-9
        };
        if !guides.iter().any(same) {
            guides.push(g);
        }
    };
    for e in input.ends.iter().take(MAX_TRACKED) {
        if (p[1] - e[1]).abs() <= reach {
            add(Guide {
                at: *e,
                dir: [1.0, 0.0],
                tracker: Some(*e),
                degrees: None,
            });
        }
        if (p[0] - e[0]).abs() <= reach {
            add(Guide {
                at: *e,
                dir: [0.0, 1.0],
                tracker: Some(*e),
                degrees: None,
            });
        }
    }
    let ray = input
        .from
        .and_then(|from| direction(from, p).map(|(dir, degrees)| (from, dir, degrees)));
    let ray_guide = ray.map(|(from, dir, degrees)| Guide {
        at: from,
        dir,
        tracker: None,
        degrees: Some(degrees),
    });
    if let Some(g) = ray_guide {
        // A tracking line that runs the same way as the direction is no help.
        guides.retain(|h| h.axis().is_none() || h.axis() != g.axis() || h.tracker.is_none());
        guides.push(g);
    }
    let drawn = |ids: &[&Guide], at: P| -> Vec<[P; 2]> {
        ids.iter()
            .filter_map(|g| g.tracker.map(|t| [t, at]))
            .collect()
    };
    // 2. The intersection of two guides, or of a guide and a baseline, nearest the pointer.
    let mut best: Option<(f64, P, Vec<Guide>, Kind)> = None;
    let mut offer = |at: P, with: Vec<Guide>, kind: Kind| {
        let d = (at[0] - p[0]).hypot(at[1] - p[1]);
        if d <= reach * 1.5 && best.as_ref().is_none_or(|b| d < b.0) {
            best = Some((d, at, with, kind));
        }
    };
    for (i, a) in guides.iter().enumerate() {
        for b in guides.iter().skip(i + 1) {
            if let Some(at) = meet(a, b) {
                offer(at, vec![*a, *b], Kind::Intersection);
            }
        }
        for wall in input.walls {
            let seg = [wall[0], wall[1]];
            let d = [seg[1][0] - seg[0][0], seg[1][1] - seg[0][1]];
            let len = d[0].hypot(d[1]);
            if len <= 0.0 {
                continue;
            }
            let line = Guide {
                at: seg[0],
                dir: [d[0] / len, d[1] / len],
                tracker: None,
                degrees: None,
            };
            if let Some(at) = meet(a, &line)
                && nearest_on(&seg, at).is_some_and(|(off, _)| off < 1e-6)
            {
                offer(at, vec![*a], Kind::Intersection);
            }
        }
    }
    if let Some((_, at, with, kind)) = best {
        let refs: Vec<&Guide> = with.iter().collect();
        return Some(Snapped {
            point: at,
            kind,
            guides: drawn(&refs, at),
            degrees: with.iter().find_map(|g| g.degrees),
        });
    }
    // 3. The nearest point of a baseline.
    let on_wall = input
        .walls
        .iter()
        .filter_map(|w| nearest_on(w, p))
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, q)) = on_wall {
        return Some(made(q, Kind::Nearest));
    }
    // 4. One tracking line (two that cross were an intersection above).
    if ray_guide.is_none()
        && let Some(g) = guides.first()
        && let Some(horizontal) = g.axis()
    {
        let at = if horizontal {
            [round_coordinate(p[0], step, reach), g.at[1]]
        } else {
            [g.at[0], round_coordinate(p[1], step, reach)]
        };
        let refs = [g];
        return Some(Snapped {
            point: at,
            kind: Kind::Tracking,
            guides: drawn(&refs, at),
            degrees: None,
        });
    }
    // 5. The direction from the segment's start; along an axis the coordinate on the grid, else
    // the length on the 10 mm grid.
    if let Some((from, dir, degrees)) = ray {
        let at = match (dir[0] == 0.0, dir[1] == 0.0) {
            (_, true) => [round_coordinate(p[0], step, reach), from[1]],
            (true, _) => [from[0], round_coordinate(p[1], step, reach)],
            _ => {
                let t = (p[0] - from[0]) * dir[0] + (p[1] - from[1]) * dir[1];
                let t = (t / GRID).round() * GRID;
                [from[0] + dir[0] * t, from[1] + dir[1] * t]
            }
        };
        return Some(Snapped {
            point: at,
            kind: Kind::Direction,
            guides: Vec::new(),
            degrees: Some(degrees),
        });
    }
    // 6. The grid.
    Some(made(
        [
            round_coordinate(p[0], step, reach),
            round_coordinate(p[1], step, reach),
        ],
        Kind::Grid,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PPU: f64 = 20.0;

    fn at(pointer: P, ends: &[P], walls: &[[P; 2]], from: Option<P>) -> Snapped {
        snap(&Input {
            pointer,
            pixels_per_unit: Some(PPU),
            ends,
            walls,
            from,
        })
        .unwrap()
    }

    #[test]
    fn four_clicks_a_few_pixels_off_and_the_start_make_an_exact_rectangle() {
        // The scenario of a house closed by the mouse: every click a few pixels off.
        let mut ends = vec![[0.0, 0.0]];
        let mut points: Vec<P> = Vec::new();
        let mut from: Option<P> = None;
        let mut walls: Vec<[P; 2]> = Vec::new();
        for pointer in [
            [-0.04, 0.03],
            [10.15, 0.12],
            [10.09, 6.14],
            [-0.12, 6.07],
            [0.06, 0.2],
        ] {
            let s = at(pointer, &ends, &walls, from);
            if let Some(f) = from {
                walls.push([f, s.point]);
                ends.push(s.point);
            }
            if points.is_empty() {
                ends.push(s.point);
            }
            points.push(s.point);
            from = Some(s.point);
        }
        assert_eq!(
            points,
            [[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0], [0.0, 0.0]]
        );
    }

    #[test]
    fn the_first_point_lands_on_the_origin_or_a_whole_meter() {
        assert_eq!(at([0.3, -0.2], &[[0.0, 0.0]], &[], None).point, [0.0, 0.0]);
        // At this scale the coarse grid is 2 m.
        let s = at([2.1, 4.04], &[], &[], None);
        assert_eq!((s.point, s.kind), ([2.0, 4.0], Kind::Grid));
        // Away from its lines the 10 mm grid applies.
        let s = at([3.3, 5.0], &[], &[], None);
        assert!((s.point[0] - 3.3).abs() < 1e-9 && s.point[1] == 5.0);
    }

    #[test]
    fn a_tracking_line_aligns_with_the_end_it_tracks_and_draws_a_guide() {
        let ends = [[1.234, 5.0]];
        let s = at([1.3, 9.2], &ends, &[], None);
        assert_eq!((s.point[0], s.kind), (1.234, Kind::Tracking));
        assert_eq!(s.guides, vec![[[1.234, 5.0], s.point]]);
        // With the direction from the start: the intersection, exactly aligned.
        let s = at([1.3, 2.1], &ends, &[], Some([4.0, 2.0]));
        assert_eq!((s.point, s.kind), ([1.234, 2.0], Kind::Intersection));
    }

    #[test]
    fn a_point_on_a_baseline_is_a_nearest_point_and_a_direction_meets_it() {
        let wall = [[[0.0, 0.0], [10.0, 0.0]]];
        let s = at([3.3, 0.2], &[[0.0, 0.0], [10.0, 0.0]], &wall, None);
        assert_eq!((s.point[1], s.kind), (0.0, Kind::Nearest));
        // Coming up from a point at x = 4.2: the direction meets the baseline there.
        let s = at(
            [4.25, 0.15],
            &[[0.0, 0.0], [10.0, 0.0]],
            &wall,
            Some([4.2, 3.0]),
        );
        assert_eq!((s.point, s.kind), ([4.2, 0.0], Kind::Intersection));
    }

    #[test]
    fn an_end_point_beats_everything_and_a_far_pointer_goes_to_the_grid() {
        let ends = [[2.0, 2.0]];
        let s = at([2.2, 2.2], &ends, &[], Some([2.0, 0.0]));
        assert_eq!((s.point, s.kind), ([2.0, 2.0], Kind::Endpoint));
        let s = at([7.4, 3.33], &[], &[], None);
        assert!((s.point[0] - 7.4).abs() < 1e-9 && (s.point[1] - 3.33).abs() < 1e-9);
        assert_eq!(s.kind, Kind::Grid);
    }
}
