//! Roof edge elements: a fascia or a gutter profile swept along sloped edges of one roof.
//!
//! The record holds the authored data only: the role, a profile (a closed loop of points
//! `(u, v)` in the plane square to the edge: `u` outward from the eave, `v` up from the roof's
//! underside at the eave) and the edges, each by its number in the roof's edge list. The roof owns
//! the record. The sweep reuses the machinery of floor edges ([`crate::floor_edge::sweep`]): the
//! roof's outline with its overhangs, consecutive selected edges one chain mitered at the corners,
//! the height of every corner read from the roof's surface. Only an edge that defines a slope has
//! an eave; a gable edge, or a number the roof no longer has, is an orphan: it is reported and
//! never rebound.

use arc_base::{ObjectId, Result};
use arc_doc::{Object, Snapshot, Value};
use arc_geom::Polyhedron;

use crate::floor_edge::{Chain, FloorEdge};
use crate::spatial;
use crate::types::{self, roof_edge};

coded! {
    /// What a roof edge element is.
    Role {
        /// A board that closes the rafter ends along the eave.
        #[default]
        Fascia = 0, "fascia";
        /// A channel that collects the water running off the roof.
        Gutter = 1, "gutter";
    }
}

/// The thickness of a fascia board by default, in meters.
pub const FASCIA_THICKNESS: f64 = 0.025;
/// The height of a fascia board by default, in meters.
pub const FASCIA_HEIGHT: f64 = 0.15;

/// The authored data of a roof edge element.
#[derive(Debug, Clone, PartialEq)]
pub struct RoofEdge {
    /// What it is.
    pub role: Role,
    /// The profile `(u, v)`, in order.
    pub profile: Vec<[f64; 2]>,
    /// The edges swept, by their number in the roof's edge list.
    pub edges: Vec<usize>,
}

impl RoofEdge {
    /// The element of a record's field values; `None` when a field is missing or malformed.
    #[must_use]
    pub fn from_values(values: &[Option<Value>]) -> Option<Self> {
        let mut shared = vec![None; 3];
        shared[1] = values.get(roof_edge::PROFILE).cloned().flatten();
        shared[2] = values.get(roof_edge::EDGES).cloned().flatten();
        let parsed = FloorEdge::from_values(&shared)?;
        Some(Self {
            role: Role::of_field(values, roof_edge::ROLE),
            profile: parsed.profile,
            edges: parsed.edges,
        })
    }

    /// The field values of a record named `name`.
    #[must_use]
    pub fn values(&self, name: &str) -> Vec<Option<Value>> {
        vec![
            Some(Value::String(name.to_owned())),
            Some(Value::Enum(self.role.code())),
            Some(FloorEdge::profile_value(&self.profile)),
            Some(FloorEdge::edges_value(&self.edges)),
        ]
    }

    /// The default profile of `role` for a roof slab `thickness` thick: a board along the end of
    /// the slab, or a box channel hung on the board.
    #[must_use]
    pub fn default_profile(role: Role, thickness: f64) -> Vec<[f64; 2]> {
        let (t, top) = (FASCIA_THICKNESS, thickness);
        match role {
            Role::Fascia => vec![
                [0.0, top - FASCIA_HEIGHT],
                [t, top - FASCIA_HEIGHT],
                [t, top],
                [0.0, top],
            ],
            Role::Gutter => {
                let (w, h, s, z) = (0.12, 0.1, 0.01, top - 0.05);
                vec![
                    [t, z],
                    [t + w, z],
                    [t + w, z + h],
                    [t + w - s, z + h],
                    [t + w - s, z + s],
                    [t + s, z + s],
                    [t + s, z + h],
                    [t, z + h],
                ]
            }
        }
    }
}

/// What a roof edge element reads from a snapshot.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The authored data.
    pub element: RoofEdge,
    /// The roof that owns it.
    pub roof: ObjectId,
    /// The edge numbers that name no sloped edge of the roof.
    pub orphans: Vec<usize>,
    /// The chains of corner points the profile is swept along.
    pub chains: Vec<Chain>,
}

fn unit(a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy);
    (l > 1.0e-9).then(|| [dx / l, dy / l])
}

impl Derived {
    /// Reads the element `object`; `None` when it or its roof is unreadable.
    #[must_use]
    pub fn of(snapshot: &Snapshot, object: &Object) -> Option<Self> {
        let element = RoofEdge::from_values(&object.values)?;
        let host = spatial::get(snapshot, object.header.owner?)?;
        let roof = crate::roof::Derived::of(snapshot, &host).ok()?;
        let outline = &roof.surface.outline;
        let n = outline.len();
        let ccw = crate::floor::signed_area(outline) > 0.0;
        let sloped = |k: usize| roof.roof.edges.get(k).is_some_and(|e| e.slope.is_some());
        let mut on = vec![false; n];
        let mut orphans = Vec::new();
        for e in &element.edges {
            if *e < n && sloped(*e) {
                on[*e] = true;
            } else {
                orphans.push(*e);
            }
        }
        let all = n > 0 && on.iter().all(|s| *s);
        let starts: Vec<usize> = if all {
            vec![0]
        } else {
            (0..n).filter(|k| on[*k] && !on[(k + n - 1) % n]).collect()
        };
        let mut chains = Vec::new();
        for start in starts {
            let mut chain = Chain {
                points: Vec::new(),
                closed: all,
                normals: Vec::new(),
                top: None,
            };
            let mut k = start;
            loop {
                let (a, b) = (outline[k], outline[(k + 1) % n]);
                if let Some(d) = unit(a, b) {
                    let normal = if ccw { [d[1], -d[0]] } else { [-d[1], d[0]] };
                    // The eave of this edge is level: both ends take its height.
                    let z = roof
                        .surface
                        .eave_at([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0])?;
                    if chain.points.is_empty() {
                        chain.points.push([a[0], a[1], z]);
                    }
                    chain.points.push([b[0], b[1], z]);
                    chain.normals.push(normal);
                }
                k = (k + 1) % n;
                if k == start || !on[k] {
                    break;
                }
            }
            if !chain.normals.is_empty() {
                chains.push(chain);
            }
        }
        Some(Self {
            element,
            roof: host.id,
            orphans,
            chains,
        })
    }

    /// The solid: the profile swept along each chain.
    ///
    /// # Errors
    /// The solid kernel's errors.
    pub fn solid(&self) -> Result<Option<Polyhedron>> {
        crate::floor_edge::sweep(&self.chains, &self.element.profile)
    }

    /// The length swept along the plan, in meters.
    #[must_use]
    pub fn length(&self) -> f64 {
        self.chains
            .iter()
            .flat_map(|c| c.points.windows(2))
            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            .sum()
    }

    /// The volume of the swept solid, in cubic meters.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.solid()
            .ok()
            .flatten()
            .map_or(0.0, |s| s.volume().abs())
    }
}

/// The edge numbers of `object` that name no sloped edge of its roof, for the warnings register.
#[must_use]
pub fn orphans_of(snapshot: &Snapshot, object: &Object) -> Vec<usize> {
    let (Some(element), Some(host)) = (
        RoofEdge::from_values(&object.values),
        object.header.owner.and_then(|h| spatial::get(snapshot, h)),
    ) else {
        return Vec::new();
    };
    let Some(roof) = crate::roof::Roof::from_values(&host.values) else {
        return Vec::new();
    };
    let _ = types::ROOF;
    element
        .edges
        .into_iter()
        .filter(|e| roof.edges.get(*e).is_none_or(|edge| edge.slope.is_none()))
        .collect()
}
