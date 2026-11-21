//! The room: authored data read from a record, and everything else derived from the walls.
//!
//! A room record holds a name, a number, a placement point, text fields (department and four
//! finishes), an upper limit (a level, else the level above its storey, else 3 m, plus an offset)
//! and a base offset. The boundary is never stored: it is the smallest closed loop of walls around
//! the placement point, taken at the interior wall faces with the floor's loop finding
//! ([`crate::floor::regions`]). Every wall bounds rooms in this slice.
//!
//! A room is in one of three states ([`State`]). It is *enclosed* when its point lies inside a
//! closed loop of walls, *unenclosed* when it does not, and *redundant* when another room's point
//! lies in the same region. Only an enclosed room has an area, perimeter and volume; a room in
//! another state keeps its identity and data.
//!
//! The functions of the first half work on plain walls and points, so the application, the model
//! checks and the command-line `inspect` (which reads a dump) share them.

use arc_base::ObjectId;
use arc_doc::{Object, Snapshot, Value};

use crate::floor::{self, Boundary, Face, WallRegion};
use crate::spatial;
use crate::types::{self, ROOM, room};
use crate::wall::Wall;

/// The most rooms read from one storey.
pub const MAX_ROOMS: usize = 10_000;
/// The number of the first room of a storey.
pub const FIRST_NUMBER: u64 = 101;
/// The height of a room whose storey has no level above it and that names no upper level, in
/// meters.
pub const DEFAULT_HEIGHT: f64 = crate::defaults::STOREY_HEIGHT;

/// What the walls make of a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The point is in a closed loop of walls that no other room's point is in.
    Enclosed,
    /// The point is in no closed loop of walls.
    Unenclosed,
    /// Another room's point is in the same region.
    Redundant,
}

impl State {
    /// The name used in listings.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Enclosed => "enclosed",
            Self::Unenclosed => "unenclosed",
            Self::Redundant => "redundant",
        }
    }
}

/// What the walls give for one placement point.
#[derive(Debug, Clone, PartialEq)]
pub struct Footprint {
    /// The state.
    pub state: State,
    /// The corners of the boundary at the interior wall faces; empty when unenclosed.
    pub polygon: Vec<[f64; 2]>,
}

impl Footprint {
    /// The area in square meters; `None` unless enclosed.
    #[must_use]
    pub fn area(&self) -> Option<f64> {
        (self.state == State::Enclosed).then(|| floor::signed_area(&self.polygon).abs())
    }

    /// The perimeter in meters; `None` unless enclosed.
    #[must_use]
    pub fn perimeter(&self) -> Option<f64> {
        (self.state == State::Enclosed).then(|| {
            Boundary {
                loops: vec![self.polygon.clone()],
            }
            .perimeter()
        })
    }
}

/// The boundary at the interior faces of the region `region`, when the walls make one around `p`.
fn interior(region: &WallRegion, walls: &[(ObjectId, Wall)], p: [f64; 2]) -> Option<Vec<[f64; 2]>> {
    let chain: Vec<(ObjectId, Wall)> = region
        .walls
        .as_ref()?
        .iter()
        .filter_map(|id| walls.iter().find(|(w, _)| w == id).copied())
        .collect();
    let vertices = floor::vertices_of_walls(&chain, Face::Interior, 0.0).ok()?;
    let polygon: Vec<[f64; 2]> = vertices.iter().map(|v| v.point).collect();
    let boundary = Boundary {
        loops: vec![polygon.clone()],
    };
    (boundary.check().is_ok() && boundary.contains(p)).then_some(polygon)
}

/// The footprints of rooms at `points` among `walls` (the walls of one storey), in the order of
/// the points. The walls' regions are found once.
#[must_use]
pub fn footprints(walls: &[(ObjectId, Wall)], points: &[[f64; 2]]) -> Vec<Footprint> {
    let regions = floor::regions(walls).unwrap_or_default();
    let found: Vec<Option<(usize, Vec<[f64; 2]>)>> = points
        .iter()
        .map(|p| {
            let at = floor::smallest_around(&regions, *p)?;
            Some((at, interior(regions.get(at)?, walls, *p)?))
        })
        .collect();
    found
        .iter()
        .enumerate()
        .map(|(k, f)| match f {
            None => Footprint {
                state: State::Unenclosed,
                polygon: Vec::new(),
            },
            Some((at, polygon)) => Footprint {
                // The first point in a region keeps it; later ones are redundant.
                state: if found[..k].iter().flatten().any(|(other, _)| other == at) {
                    State::Redundant
                } else {
                    State::Enclosed
                },
                polygon: polygon.clone(),
            },
        })
        .collect()
}

/// [`footprints`] for rooms given as `(point, number)`: in a region with several rooms the one
/// with the lowest number (whole numbers by value, then text) keeps it, the earlier in the list
/// on a tie. The footprints are in the order of `items`.
#[must_use]
pub fn footprints_ranked(
    walls: &[(ObjectId, Wall)],
    items: &[([f64; 2], Option<&str>)],
) -> Vec<Footprint> {
    let rank = |k: usize| {
        let n = items[k].1.map(str::trim).unwrap_or_default();
        (
            n.parse::<u64>().ok().map_or((1, 0), |v| (0, v)),
            n.to_owned(),
            k,
        )
    };
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|k| rank(*k));
    let points: Vec<[f64; 2]> = order.iter().map(|k| items[*k].0).collect();
    let mut prints = footprints(walls, &points).into_iter();
    let mut out: Vec<Option<Footprint>> = vec![None; items.len()];
    for k in order {
        out[k] = prints.next();
    }
    out.into_iter().flatten().collect()
}

/// The boundaries of every enclosed region of `walls`, each with a point well inside it: what
/// `place_all` puts a room in.
#[must_use]
pub fn enclosed_regions(walls: &[(ObjectId, Wall)]) -> Vec<([f64; 2], Vec<[f64; 2]>)> {
    let regions = floor::regions(walls).unwrap_or_default();
    regions
        .iter()
        .filter_map(|r| {
            let rough = interior_point(&r.polygon)?;
            let polygon = interior(r, walls, rough)?;
            Some((pole(&polygon)?, polygon))
        })
        .collect()
}

/// The distance from `p` to the nearest edge of `polygon`, negative when `p` is outside.
fn depth(polygon: &[[f64; 2]], p: [f64; 2]) -> f64 {
    let n = polygon.len();
    let nearest = (0..n)
        .map(|k| {
            let (a, b) = (polygon[k], polygon[(k + 1) % n]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let len2 = d[0] * d[0] + d[1] * d[1];
            let t = if len2 > 0.0 {
                (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
        })
        .fold(f64::MAX, f64::min);
    if (Boundary {
        loops: vec![polygon.to_vec()],
    })
    .contains(p)
    {
        nearest
    } else {
        -nearest
    }
}

/// The point of `polygon` farthest from its edges, found on a grid that narrows around the best
/// cell (the pole of inaccessibility, to a few millimeters for rooms): the middle of an L-shaped
/// room's wider arm, not its corner. `None` for a loop with no inside.
#[must_use]
pub fn pole(polygon: &[[f64; 2]]) -> Option<[f64; 2]> {
    if polygon.len() < 3 {
        return None;
    }
    let (mut lo, mut hi) = polygon
        .iter()
        .fold(([f64::MAX; 2], [f64::MIN; 2]), |(l, h), p| {
            (
                [l[0].min(p[0]), l[1].min(p[1])],
                [h[0].max(p[0]), h[1].max(p[1])],
            )
        });
    let mut best: Option<([f64; 2], f64)> = None;
    for _ in 0..6 {
        let n = 12_u32;
        let step = [
            (hi[0] - lo[0]) / f64::from(n),
            (hi[1] - lo[1]) / f64::from(n),
        ];
        for i in 0..=n {
            for j in 0..=n {
                let p = [
                    lo[0] + step[0] * f64::from(i),
                    lo[1] + step[1] * f64::from(j),
                ];
                let d = depth(polygon, p);
                if best.is_none_or(|(_, b)| d > b) {
                    best = Some((p, d));
                }
            }
        }
        let (at, _) = best?;
        lo = [at[0] - step[0], at[1] - step[1]];
        hi = [at[0] + step[0], at[1] + step[1]];
    }
    best.filter(|(_, d)| *d > 0.0).map(|(p, _)| p)
}

/// A point inside `polygon`, away from its edges: the middle of the widest stretch of a few
/// horizontal lines through it; `None` for a loop with no area.
#[must_use]
pub fn interior_point(polygon: &[[f64; 2]]) -> Option<[f64; 2]> {
    let (low, high) = polygon
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p[1]), h.max(p[1])));
    if polygon.len() < 3 || !high.is_finite() || !low.is_finite() || high <= low {
        return None;
    }
    for fraction in [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8] {
        let y = low + (high - low) * fraction;
        let mut xs: Vec<f64> = Vec::new();
        for k in 0..polygon.len() {
            let (a, b) = (polygon[k], polygon[(k + 1) % polygon.len()]);
            if (a[1] > y) != (b[1] > y) {
                xs.push(a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]));
            }
        }
        xs.sort_by(f64::total_cmp);
        let widest = xs
            .chunks_exact(2)
            .max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0])));
        if let Some(pair) = widest {
            let at = [(pair[0] + pair[1]) / 2.0, y];
            if (pair[1] - pair[0]) > 1.0e-6 {
                return Some(at);
            }
        }
    }
    None
}

/// The height of a room in meters: from its base to its top. `upper` is the elevation of the
/// level the room names as its upper limit, `above` the elevation of the level above its storey;
/// without either the room is [`DEFAULT_HEIGHT`] high. Never negative.
#[must_use]
pub fn height(
    storey_elevation: f64,
    upper: Option<f64>,
    above: Option<f64>,
    upper_offset: f64,
    base_offset: f64,
) -> f64 {
    let top = upper.or(above).unwrap_or(storey_elevation + DEFAULT_HEIGHT) + upper_offset;
    (top - (storey_elevation + base_offset)).max(0.0)
}

/// The levels above the level of `storey`, from the lowest up: the choices of a room's upper limit.
#[must_use]
pub fn levels_above(snapshot: &Snapshot, storey: &Object) -> Vec<std::sync::Arc<Object>> {
    let own = spatial::storey_elevation(snapshot, storey);
    storey
        .header
        .owner
        .map(|b| spatial::levels(snapshot, b))
        .unwrap_or_default()
        .into_iter()
        .filter(|l| types::meters(&l.values, types::level::ELEVATION).is_some_and(|e| e > own))
        .collect()
}

/// The number after the highest whole number among `numbers`; [`FIRST_NUMBER`] when there is none.
#[must_use]
pub fn next_number<'a>(numbers: impl IntoIterator<Item = &'a str>) -> String {
    let highest = numbers
        .into_iter()
        .filter_map(|n| n.trim().parse::<u64>().ok())
        .max();
    highest
        .map_or(FIRST_NUMBER, |h| h.saturating_add(1).max(FIRST_NUMBER))
        .to_string()
}

/// The key a number is compared by: surrounding white space removed, case folded.
#[must_use]
pub fn number_key(number: &str) -> String {
    crate::names::fold(number)
}

/// The authored data of a room.
#[derive(Debug, Clone, PartialEq)]
pub struct Room {
    /// The name.
    pub name: String,
    /// The number, unique per storey (a duplicate is reported, not refused).
    pub number: Option<String>,
    /// The placement point.
    pub point: [f64; 2],
    /// The department.
    pub department: Option<String>,
    /// The floor, wall, ceiling and base finishes.
    pub finishes: [Option<String>; 4],
    /// The level the room's top is limited by; the level above the storey when absent.
    pub upper_level: Option<ObjectId>,
    /// The top's offset from that level, in meters.
    pub upper_offset: f64,
    /// The base's offset from the level of the storey, in meters.
    pub base_offset: f64,
}

fn text(values: &[Option<Value>], at: usize) -> Option<String> {
    types::text(values, at)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

impl Room {
    /// Reads a room record; `None` when the name or the point is missing.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        Some(Self {
            name: types::text(values, room::NAME)?.to_owned(),
            number: text(values, room::NUMBER),
            point: [
                types::meters(values, room::X)?,
                types::meters(values, room::Y)?,
            ],
            department: text(values, room::DEPARTMENT),
            finishes: [
                text(values, room::FLOOR_FINISH),
                text(values, room::WALL_FINISH),
                text(values, room::CEILING_FINISH),
                text(values, room::BASE_FINISH),
            ],
            upper_level: match values.get(room::UPPER_LEVEL) {
                Some(Some(Value::Reference(id))) => Some(*id),
                _ => None,
            },
            upper_offset: types::meters(values, room::UPPER_OFFSET).unwrap_or(0.0),
            base_offset: types::meters(values, room::BASE_OFFSET).unwrap_or(0.0),
        })
    }
}

/// A room with everything derived from the document.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The room's identifier.
    pub id: ObjectId,
    /// The authored data.
    pub room: Room,
    /// What the walls make of it.
    pub footprint: Footprint,
    /// The elevation of the base, in meters.
    pub base: f64,
    /// The elevation of the top, in meters.
    pub top: f64,
}

impl Entry {
    /// The state.
    #[must_use]
    pub fn state(&self) -> State {
        self.footprint.state
    }

    /// The area in square meters; `None` unless enclosed.
    #[must_use]
    pub fn area(&self) -> Option<f64> {
        self.footprint.area()
    }

    /// The perimeter in meters; `None` unless enclosed.
    #[must_use]
    pub fn perimeter(&self) -> Option<f64> {
        self.footprint.perimeter()
    }

    /// The volume in cubic meters: the area times the height; `None` unless enclosed.
    #[must_use]
    pub fn volume(&self) -> Option<f64> {
        self.area().map(|a| a * (self.top - self.base).max(0.0))
    }
}

/// The rooms of `storey` with their derived data, in identifier order.
#[must_use]
pub fn of_storey(snapshot: &Snapshot, storey: ObjectId) -> Vec<Entry> {
    let Some(owner) = spatial::get(snapshot, storey) else {
        return Vec::new();
    };
    let records: Vec<(ObjectId, Room)> = spatial::children(snapshot, ROOM, storey)
        .iter()
        .take(MAX_ROOMS)
        .filter_map(|o: &std::sync::Arc<Object>| Some((o.id, Room::from_values(&o.values)?)))
        .collect();
    let items: Vec<([f64; 2], Option<&str>)> = records
        .iter()
        .map(|(_, r)| (r.point, r.number.as_deref()))
        .collect();
    let prints = footprints_ranked(&floor::storey_walls(snapshot, storey), &items);
    let level = spatial::storey_elevation(snapshot, &owner);
    let above = spatial::level_above(snapshot, &owner)
        .and_then(|l| types::meters(&l.values, types::level::ELEVATION));
    records
        .into_iter()
        .zip(prints)
        .map(|((id, room), footprint)| {
            let upper = room
                .upper_level
                .and_then(|l| spatial::get(snapshot, l))
                .and_then(|l| types::meters(&l.values, types::level::ELEVATION));
            let top = level + height(level, upper, above, room.upper_offset, 0.0);
            Entry {
                id,
                footprint,
                base: level + room.base_offset,
                top,
                room,
            }
        })
        .collect()
}

/// The rooms of the entries whose number another room of the list has too, as identifiers.
#[must_use]
pub fn duplicate_numbers(entries: &[Entry]) -> Vec<ObjectId> {
    entries
        .iter()
        .filter(|e| {
            e.room.number.as_deref().is_some_and(|n| {
                entries.iter().any(|o| {
                    o.id != e.id
                        && o.room
                            .number
                            .as_deref()
                            .is_some_and(|m| number_key(m) == number_key(n))
                })
            })
        })
        .map(|e| e.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wall::{JoinStyle, LocationLine};
    use arc_base::id::Identifier;

    fn wall(start: [f64; 2], end: [f64; 2]) -> Wall {
        Wall {
            start,
            end,
            thickness: 0.2,
            height: 3.0,
            base_offset: 0.0,
            location: LocationLine::Center,
            start_join: JoinStyle::Auto,
            end_join: JoinStyle::Auto,
        }
    }

    fn id(n: u64) -> ObjectId {
        ObjectId::from_u128(u128::from(n))
    }

    /// A 10 x 6 house of center-line walls with a partition at x = 5.
    fn house(partition: bool) -> Vec<(ObjectId, Wall)> {
        let mut walls = vec![
            (id(1), wall([0.0, 0.0], [10.0, 0.0])),
            (id(2), wall([10.0, 0.0], [10.0, 6.0])),
            (id(3), wall([10.0, 6.0], [0.0, 6.0])),
            (id(4), wall([0.0, 6.0], [0.0, 0.0])),
        ];
        if partition {
            walls.push((id(5), wall([5.0, 0.0], [5.0, 6.0])));
        }
        walls
    }

    #[test]
    fn two_rooms_either_side_of_a_partition_have_the_hand_calculated_areas() {
        let prints = footprints(&house(true), &[[2.5, 3.0], [7.5, 3.0]]);
        for print in &prints {
            assert_eq!(print.state, State::Enclosed);
            // 4.8 x 5.8 clear, 2 x (4.8 + 5.8) round.
            assert!((print.area().unwrap() - 27.84).abs() < 1e-9);
            assert!((print.perimeter().unwrap() - 21.2).abs() < 1e-9);
        }
    }

    #[test]
    fn without_the_partition_two_points_share_a_region_and_a_point_outside_is_unenclosed() {
        let prints = footprints(&house(false), &[[2.5, 3.0], [7.5, 3.0], [20.0, 3.0]]);
        // The first keeps its area; only the later one is redundant.
        assert_eq!(prints[0].state, State::Enclosed);
        assert!(prints[0].area().is_some());
        assert_eq!(prints[1].state, State::Redundant);
        assert_eq!(prints[1].area(), None);
        let ranked = footprints_ranked(
            &house(false),
            &[([2.5, 3.0], Some("102")), ([7.5, 3.0], Some("101"))],
        );
        assert_eq!(ranked[0].state, State::Redundant);
        assert_eq!(ranked[1].state, State::Enclosed);
        assert_eq!(prints[2].state, State::Unenclosed);
        // A point inside the thickness of a wall is no room either.
        let wall_body = footprints(&house(false), &[[5.0, 0.05]]);
        assert_eq!(wall_body[0].state, State::Unenclosed);
    }

    #[test]
    fn the_pole_of_an_l_shaped_room_is_as_far_from_the_walls_as_it_can_be() {
        // Arms 4 m wide: no point is more than 2 m from the boundary.
        let l = [
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 4.0],
            [4.0, 4.0],
            [4.0, 10.0],
            [0.0, 10.0],
        ];
        let at = pole(&l).unwrap();
        assert!(depth(&l, at) > 1.95, "{at:?} {}", depth(&l, at));
        let brute = (0..=100)
            .flat_map(|i| (0..=100).map(move |j| [f64::from(i) / 10.0, f64::from(j) / 10.0]))
            .map(|p| depth(&l, p))
            .fold(f64::MIN, f64::max);
        assert!(depth(&l, at) >= brute - 0.05);
        // The centroid of the L is closer to a wall.
        assert!(depth(&l, [3.0, 3.0]) < depth(&l, at) + 1e-9);
        assert!(pole(&[[0.0, 0.0], [1.0, 0.0]]).is_none());
    }

    #[test]
    fn every_enclosed_region_has_a_point_inside_it() {
        let regions = enclosed_regions(&house(true));
        assert_eq!(regions.len(), 2);
        for (point, polygon) in regions {
            assert!((floor::signed_area(&polygon).abs() - 27.84).abs() < 1e-9);
            assert!(
                Boundary {
                    loops: vec![polygon]
                }
                .contains(point)
            );
        }
    }

    #[test]
    fn the_numbers_follow_the_highest_whole_number_and_height_has_defaults() {
        assert_eq!(next_number([]), "101");
        assert_eq!(next_number(["101", "A-2", " 104"]), "105");
        assert_eq!(next_number(["7"]), "101");
        assert!((height(0.0, None, None, 0.0, 0.0) - 3.0).abs() < 1e-12);
        assert!((height(0.0, None, Some(3.5), 0.0, 0.5) - 3.0).abs() < 1e-12);
        assert!((height(0.0, Some(4.0), Some(3.5), -0.5, 0.0) - 3.5).abs() < 1e-12);
        assert!(height(0.0, Some(-1.0), None, 0.0, 0.0) >= 0.0);
    }
}
