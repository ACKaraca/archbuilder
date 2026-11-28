//! Where the model is: the internal origin, the project base point, the survey point, true north
//! and the geodetic reference (AB-DATUM-014, -015, -016, -018).
//!
//! The internal origin is fixed: it is where every stored coordinate is measured from. The
//! project base point and the survey point are positions in internal coordinates that the project
//! record keeps; moving either changes the coordinates reported for a point and never an
//! element's position. True north is the angle from project north (the internal y axis) to true
//! north, counterclockwise. The geodetic reference names a coordinate reference system and gives
//! the eastings, northings and height of the survey point in it, with a scale factor; together
//! with true north it maps an internal point to map coordinates the way an IFC map conversion
//! does. All fields are optional: a project without them has both points at the origin, north up
//! and no georeference.

use arc_doc::{Snapshot, Value};

use crate::spatial;
use crate::types::{self, PROJECT, project};

/// The range of the scale factor.
pub const SCALE_RANGE: (f64, f64) = (0.001, 1000.0);
/// The longest coordinate reference system text (an EPSG code or a WKT definition), in bytes.
pub const MAX_CRS: u64 = 16_384;
/// The range of eastings and northings, in meters.
pub const MAP_RANGE: (f64, f64) = (-1.0e9, 1.0e9);
/// The distance from the internal origin beyond which geometry is reported, when a project does
/// not set one, in meters.
pub const DEFAULT_PRECISION_DISTANCE: f64 = 20_000.0;

type P3 = [f64; 3];

/// The location of a project: its two points, north and geodetic reference.
#[derive(Debug, Clone, PartialEq)]
pub struct Coordinates {
    /// The project base point, in internal coordinates.
    pub base: P3,
    /// The survey point, in internal coordinates.
    pub survey: P3,
    /// The angle from project north to true north, counterclockwise, in radians.
    pub true_north: f64,
    /// The scale factor from model to map lengths.
    pub scale: f64,
    /// The coordinate reference system: an EPSG code or a WKT text.
    pub crs: Option<String>,
    /// The eastings, northings and height of the survey point in that system, in meters; `None`
    /// until the eastings and northings are set.
    pub map: Option<P3>,
    /// The distance from the internal origin beyond which geometry is reported.
    pub precision_distance: f64,
}

impl Default for Coordinates {
    fn default() -> Self {
        Self {
            base: [0.0; 3],
            survey: [0.0; 3],
            true_north: 0.0,
            scale: 1.0,
            crs: None,
            map: None,
            precision_distance: DEFAULT_PRECISION_DISTANCE,
        }
    }
}

/// One point in every system the project knows.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// Internal coordinates: from the internal origin.
    pub internal: P3,
    /// Project coordinates: from the project base point, along the same axes.
    pub project: P3,
    /// Survey coordinates: from the survey point, along the same axes.
    pub survey: P3,
    /// Map coordinates (eastings, northings, height); `None` without a georeference.
    pub map: Option<P3>,
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

impl Coordinates {
    /// The location a project record's field values hold.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Self {
        let m = |at: usize| types::meters(values, at);
        let finite = |v: Option<f64>, default: f64| v.filter(|v| v.is_finite()).unwrap_or(default);
        let north = match values.get(project::TRUE_NORTH) {
            Some(Some(Value::Quantity(q))) if q.value().is_finite() => q.value(),
            _ => 0.0,
        };
        let scale = match values.get(project::SCALE) {
            Some(Some(Value::F64(s))) if s.is_finite() && *s > 0.0 => *s,
            _ => 1.0,
        };
        let map = match (m(project::EASTINGS), m(project::NORTHINGS)) {
            (Some(e), Some(n)) => Some([e, n, finite(m(project::HEIGHT), 0.0)]),
            _ => None,
        };
        Self {
            base: [
                finite(m(project::BASE_X), 0.0),
                finite(m(project::BASE_Y), 0.0),
                finite(m(project::BASE_Z), 0.0),
            ],
            survey: [
                finite(m(project::SURVEY_X), 0.0),
                finite(m(project::SURVEY_Y), 0.0),
                finite(m(project::SURVEY_Z), 0.0),
            ],
            true_north: north,
            scale,
            crs: types::text(values, project::CRS)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned),
            map,
            precision_distance: finite(m(project::PRECISION_DISTANCE), DEFAULT_PRECISION_DISTANCE),
        }
    }

    /// The location of the project of `snapshot`; the default for a document without one.
    #[must_use]
    pub fn of(snapshot: &Snapshot) -> Self {
        spatial::objects(snapshot, PROJECT)
            .first()
            .map(|p| Self::from_values(&p.values))
            .unwrap_or_default()
    }

    /// Whether a coordinate reference system and the survey point's place in it are both set.
    #[must_use]
    pub fn is_georeferenced(&self) -> bool {
        self.crs.is_some() && self.map.is_some()
    }

    /// The map coordinates of the internal point `p`: the survey point's, plus the offset from it
    /// turned by true north and scaled. `None` without a map position for the survey point.
    #[must_use]
    pub fn map_of(&self, p: P3) -> Option<P3> {
        let [e0, n0, h0] = self.map?;
        let [dx, dy, dz] = sub(p, self.survey);
        let (s, c) = self.true_north.sin_cos();
        Some([
            e0 + self.scale * (dx * c + dy * s),
            n0 + self.scale * (-dx * s + dy * c),
            h0 + self.scale * dz,
        ])
    }

    /// The internal point at the map coordinates `map`: the inverse of [`Self::map_of`].
    #[must_use]
    pub fn internal_of(&self, map: P3) -> Option<P3> {
        let [e0, n0, h0] = self.map?;
        let (s, c) = self.true_north.sin_cos();
        let (de, dn, dh) = (
            (map[0] - e0) / self.scale,
            (map[1] - n0) / self.scale,
            (map[2] - h0) / self.scale,
        );
        Some([
            self.survey[0] + de * c - dn * s,
            self.survey[1] + de * s + dn * c,
            self.survey[2] + dh,
        ])
    }

    /// The internal point `p` in every system.
    #[must_use]
    pub fn read(&self, p: P3) -> Reading {
        Reading {
            internal: p,
            project: sub(p, self.base),
            survey: sub(p, self.survey),
            map: self.map_of(p),
        }
    }

    /// How far the plan of a view oriented to `north` is turned from the internal axes: a
    /// view in project north is not, a view in true north turns clockwise by the true north angle
    /// so that true north points up. In radians, counterclockwise positive.
    #[must_use]
    pub fn view_rotation(&self, north: crate::view::North) -> f64 {
        match north {
            crate::view::North::Project => 0.0,
            crate::view::North::True => -self.true_north,
        }
    }
}

/// The most elements the far-geometry check looks at.
const MAX_CHECKED: usize = 200_000;
/// The most elements one finding lists.
const MAX_LISTED: usize = 1_000;

/// The authored plan points that stand for the element `object` of type `key`: where it lies. An
/// element that follows another (a door, a window) has none.
#[must_use]
pub fn anchors(object: &arc_doc::Object, key: &str) -> Vec<[f64; 2]> {
    let v = &object.values;
    match key {
        types::WALL => crate::wall::Wall::from_values(v)
            .map(|w| vec![w.start, w.end])
            .unwrap_or_default(),
        types::FLOOR | types::CEILING | types::SHAFT | types::VERTICAL_OPENING => {
            crate::floor::slots_of(key)
                .and_then(|s| crate::floor::Floor::read(v, s))
                .map(|f| f.vertices.iter().map(|p| p.point).collect())
                .unwrap_or_default()
        }
        types::ROOM => crate::room::Room::from_values(v)
            .map(|r| vec![r.point])
            .unwrap_or_default(),
        types::STAIR => crate::stair::Stair::from_values(v)
            .map(|s| vec![s.start])
            .unwrap_or_default(),
        types::RAMP => crate::ramp::Ramp::from_values(v)
            .map(|r| vec![r.start])
            .unwrap_or_default(),
        types::RAILING => crate::railing::Railing::from_values(v)
            .map(|r| r.path.iter().map(|p| [p.point[0], p.point[1]]).collect())
            .unwrap_or_default(),
        types::ROOF => crate::roof::Roof::from_values(v)
            .map(|r| r.edges.iter().map(|e| e.vertex.point).collect())
            .unwrap_or_default(),
        types::ROOF_EXTRUSION => crate::roof_extrusion::RoofExtrusion::from_values(v)
            .map(|r| vec![r.origin])
            .unwrap_or_default(),
        types::GRID => crate::datum::Grid::from_values(v)
            .map(|g| g.points())
            .unwrap_or_default(),
        types::REFERENCE_PLANE => crate::datum::RefPlane::from_values(v)
            .map(|r| vec![r.start, r.end])
            .unwrap_or_default(),
        types::FOREIGN_ELEMENT => crate::foreign::bounds(v)
            .map(|(lo, hi)| vec![[lo[0], lo[1]], [hi[0], hi[1]]])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// The elements that lie beyond the precision distance of the internal origin, with how far the
/// farthest of each lies; the distance is measured in plan.
#[must_use]
pub fn distant(snapshot: &Snapshot) -> Vec<(arc_base::ObjectId, f64)> {
    let limit = Coordinates::of(snapshot).precision_distance;
    let mut out = Vec::new();
    for key in types::ELEMENTS {
        for object in spatial::objects(snapshot, key).iter().take(MAX_CHECKED) {
            let far = anchors(object, key)
                .iter()
                .map(|p| p[0].hypot(p[1]))
                .fold(0.0_f64, f64::max);
            if far > limit {
                out.push((object.id, far));
            }
        }
    }
    out
}

/// The warning of the register for geometry beyond the precision distance: one finding that names
/// how many elements and how far the farthest lies, and lists the elements; `None` when all are
/// near.
#[must_use]
pub fn distant_findings(snapshot: &Snapshot) -> Option<(arc_base::Error, Vec<arc_base::ObjectId>)> {
    let found = distant(snapshot);
    let farthest = found.iter().map(|(_, d)| *d).fold(0.0_f64, f64::max);
    (!found.is_empty()).then(|| {
        let error = arc_base::Error::new(crate::error::codes::LOCATION_DISTANT)
            .with_arg("count", found.len().to_string())
            .with_arg("distance", format!("{:.0}", farthest.round()))
            .with_arg(
                "limit",
                format!(
                    "{:.0}",
                    Coordinates::of(snapshot).precision_distance.round()
                ),
            );
        (
            error,
            found
                .into_iter()
                .take(MAX_LISTED)
                .map(|(id, _)| id)
                .collect(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn located() -> Coordinates {
        Coordinates {
            base: [10.0, 20.0, 1.0],
            survey: [100.0, 200.0, 5.0],
            true_north: std::f64::consts::FRAC_PI_2,
            scale: 1.0,
            crs: Some("EPSG:3067".into()),
            map: Some([500_000.0, 6_800_000.0, 40.0]),
            precision_distance: 20_000.0,
        }
    }

    #[test]
    fn a_point_reads_in_every_system() {
        let c = located();
        let r = c.read([110.0, 200.0, 6.0]);
        assert_eq!(r.project, [100.0, 180.0, 5.0]);
        assert_eq!(r.survey, [10.0, 0.0, 1.0]);
        // True north is 90 degrees counterclockwise of project north, so project +x points
        // south on the map.
        let map = r.map.unwrap();
        assert!((map[0] - 500_000.0).abs() < 1e-6);
        assert!((map[1] - (6_800_000.0 - 10.0)).abs() < 1e-6);
        assert!((map[2] - 41.0).abs() < 1e-9);
    }

    #[test]
    fn map_coordinates_convert_back_and_a_project_without_a_georeference_has_none() {
        let mut c = located();
        c.true_north = 0.3;
        c.scale = 1.0001;
        let p = [12.5, -7.0, 3.0];
        let back = c.internal_of(c.map_of(p).unwrap()).unwrap();
        for k in 0..3 {
            assert!((back[k] - p[k]).abs() < 1e-9);
        }
        assert_eq!(Coordinates::default().read(p).map, None);
        assert_eq!(Coordinates::default().read(p).project, p);
    }

    #[test]
    fn a_true_north_view_turns_the_plan_so_north_is_up() {
        let c = located();
        assert_eq!(c.view_rotation(crate::view::North::Project), 0.0);
        assert!((c.view_rotation(crate::view::North::True) + c.true_north).abs() < 1e-12);
    }
}
