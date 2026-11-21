//! The plane motions the modify commands apply to the 2D data of walls and floors: a translation,
//! a rotation about a point and a reflection across a line. Each maps points; a reflection also
//! reverses the handedness of a wall baseline ([`Xform::reflects`]).
//!
//! A quarter turn is exact: its sine and cosine are 0 and ±1, so integer coordinates stay
//! integers. A reflection across a horizontal or vertical line is exact as well.

use std::f64::consts::FRAC_PI_2;

/// A plane motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Xform {
    /// Moves every point by the vector.
    Translate([f64; 2]),
    /// Turns every point counterclockwise by `angle` radians about `base`.
    Rotate {
        /// The center.
        base: [f64; 2],
        /// The angle, in radians.
        angle: f64,
    },
    /// Reflects every point across the line through `origin` with the unit direction `dir`.
    Mirror {
        /// A point of the line.
        origin: [f64; 2],
        /// The unit direction of the line.
        dir: [f64; 2],
    },
}

impl Xform {
    /// The reflection across the line through two points; `None` when they are closer than
    /// `1e-9` or not finite.
    #[must_use]
    pub fn mirror(first: [f64; 2], second: [f64; 2]) -> Option<Self> {
        let (dx, dy) = (second[0] - first[0], second[1] - first[1]);
        let length = dx.hypot(dy);
        (length.is_finite() && length > 1.0e-9).then(|| Self::Mirror {
            origin: first,
            dir: [dx / length, dy / length],
        })
    }

    /// Whether the motion reverses handedness.
    #[must_use]
    pub const fn reflects(&self) -> bool {
        matches!(self, Self::Mirror { .. })
    }

    /// Whether every coordinate of the motion is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Translate(d) => d.iter().all(|v| v.is_finite()),
            Self::Rotate { base, angle } => base.iter().all(|v| v.is_finite()) && angle.is_finite(),
            Self::Mirror { origin, dir } => origin.iter().chain(dir).all(|v| v.is_finite()),
        }
    }

    /// The image of `p`.
    #[must_use]
    pub fn point(&self, p: [f64; 2]) -> [f64; 2] {
        match *self {
            Self::Translate(d) => [p[0] + d[0], p[1] + d[1]],
            Self::Rotate { base, angle } => {
                let (s, c) = sin_cos(angle);
                let (x, y) = (p[0] - base[0], p[1] - base[1]);
                [base[0] + c * x - s * y, base[1] + s * x + c * y]
            }
            Self::Mirror { origin, dir } => {
                let (x, y) = (p[0] - origin[0], p[1] - origin[1]);
                let along = x * dir[0] + y * dir[1];
                [
                    2.0 * (origin[0] + along * dir[0]) - p[0],
                    2.0 * (origin[1] + along * dir[1]) - p[1],
                ]
            }
        }
    }
}

/// The sine and cosine of `angle`, exact at the multiples of a quarter turn.
fn sin_cos(angle: f64) -> (f64, f64) {
    let quarters = angle / FRAC_PI_2;
    let nearest = quarters.round();
    if (quarters - nearest).abs() < 1.0e-12 && nearest.abs() < 1.0e6 {
        // `nearest` is a whole number below a million in magnitude.
        #[allow(clippy::cast_possible_truncation, reason = "a whole number below 1e6")]
        return match (nearest as i64).rem_euclid(4) {
            0 => (0.0, 1.0),
            1 => (1.0, 0.0),
            2 => (0.0, -1.0),
            _ => (-1.0, 0.0),
        };
    }
    angle.sin_cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quarter_turn_and_an_axis_mirror_are_exact() {
        let turn = Xform::Rotate {
            base: [1.0, 1.0],
            angle: FRAC_PI_2,
        };
        assert_eq!(turn.point([4.0, 1.0]), [1.0, 4.0]);
        let back = Xform::Rotate {
            base: [0.0, 0.0],
            angle: -FRAC_PI_2,
        };
        assert_eq!(back.point([2.0, 0.0]), [0.0, -2.0]);
        let mirror = Xform::mirror([5.0, 0.0], [5.0, 3.0]);
        assert_eq!(mirror.map(|m| m.point([2.0, 1.0])), Some([8.0, 1.0]));
        assert!(Xform::mirror([1.0, 1.0], [1.0, 1.0]).is_none());
    }
}
