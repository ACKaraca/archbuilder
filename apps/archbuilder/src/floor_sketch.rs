//! The boundary sketch of a floor: the outline and the openings as editable loops, headless. The
//! engine keeps a [`Sketch`] of the floor (its vertices, arcs and the walls its edges follow),
//! reads pointer input in model coordinates and answers with what to draw and, on Finish, with
//! the parameters of `archbuilder.floor.edit_boundary`, so everything the person did is one
//! command and one undo step. Escape drops the sketch and the floor is untouched.
//!
//! - **Delete**: a click on a vertex removes it (a loop keeps three).
//! - **Select** (the start): a press on a vertex's square drags the vertex, one on the diamond at
//!   the middle of an edge drags the edge; a click selects. Delete removes the selected vertex.
//! - **Add**: a click on an edge adds a vertex there. **Arc**: a click on an edge picks it, the
//!   next click is a point of the arc, which makes a three-point arc (the edge becomes straight
//!   again when the point lies on its line). **Wall**: a click on a wall replaces the selected
//!   edge (or the nearest one) by an edge that follows that wall.
//! - An edge that follows a wall stays linked until an edit touches it; the engine counts the
//!   edges an edit freed so the person can be told.

use arc_base::ObjectId;
use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::{OverlayColor, ProductOverlay};
use archbuilder_model::floor::{self, Boundary, Face, Floor, Refusal, flatten_edge};
use archbuilder_model::sketch::Sketch;
use archbuilder_model::wall::Wall;
use serde_json::json;

type P = [f64; 2];

/// How far from a handle a press still takes it, in pixels.
const HIT_PIXELS: f64 = 9.0;
/// Half the side of a handle, in pixels.
const HANDLE_HALF: f64 = 4.5;
/// The chord of the lines drawn for arcs, in meters.
const DRAW_CHORD: f64 = 0.01;

/// What a click does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Select, and drag with a press.
    Select,
    /// Add a vertex on an edge.
    Add,
    /// Make an edge a three-point arc.
    Arc,
    /// Replace an edge by a wall.
    Wall,
    /// Delete the vertex that is clicked.
    Delete,
}

/// What is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sel {
    /// A vertex, by index in the sketch.
    Vertex(usize),
    /// The edge that starts at a vertex.
    Edge(usize),
}

#[derive(Debug, Clone, Copy)]
enum Drag {
    Vertex(usize),
    Edge(usize, P),
}

/// The sketch of one floor.
#[derive(Debug, Clone)]
pub struct FloorSketch {
    /// The floor.
    pub floor: ObjectId,
    base: Floor,
    /// The sketch being edited.
    pub sketch: Sketch,
    /// What a click does.
    pub mode: Mode,
    /// What is selected.
    pub selected: Option<Sel>,
    arc_edge: Option<usize>,
    drag: Option<Drag>,
    /// The elevation the sketch is drawn at, in meters.
    pub z: f64,
    /// The edges that followed a wall and were freed since the last time this was taken.
    freed: usize,
    /// Whether anything changed since the start.
    pub changed: bool,
    /// What the last click could not do, to be said, taken once.
    hint: Option<Hint>,
}

/// Why a click did nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hint {
    /// A wall was clicked with no edge selected.
    PickEdgeFirst,
    /// A vertex cannot be deleted: its loop keeps three.
    LoopTooSmall,
}

fn dist(a: P, b: P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// The nearest point of the segment `a`-`b` to `p`.
fn nearest_on(a: P, b: P, p: P) -> P {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    [a[0] + d[0] * t, a[1] + d[1] * t]
}

impl FloorSketch {
    /// The sketch of the floor `id`, from its authored data and the walls it follows; `None` when
    /// the record is not a readable floor.
    #[must_use]
    pub fn begin(snapshot: &Snapshot, id: ObjectId) -> Option<Self> {
        let object = archbuilder_model::spatial::get(snapshot, id)?;
        let base = Floor::from_values(&object.values)?;
        let boundary = base.boundary_in(snapshot)?;
        let derived = floor::Derived::of(snapshot, &object)?;
        Some(Self {
            floor: id,
            sketch: Sketch::of(&base, &boundary),
            base,
            mode: Mode::Select,
            selected: None,
            arc_edge: None,
            drag: None,
            z: derived.top,
            freed: 0,
            changed: false,
            hint: None,
        })
    }

    /// What the last click could not do, once.
    pub fn take_hint(&mut self) -> Option<Hint> {
        self.hint.take()
    }

    /// The edges freed since the last call.
    pub fn take_freed(&mut self) -> usize {
        std::mem::take(&mut self.freed)
    }

    /// Whether a drag is in progress.
    #[must_use]
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The edge picked for an arc, waiting for its point.
    #[must_use]
    pub fn arc_pending(&self) -> bool {
        self.arc_edge.is_some()
    }

    fn at(&self, k: usize) -> Option<(P, P, f64)> {
        let v = self.sketch.vertices.get(k)?;
        let r = self.loop_range(k)?;
        let next = self
            .sketch
            .vertices
            .get(if k + 1 == r.end { r.start } else { k + 1 })?;
        Some((v.point, next.point, v.bulge))
    }

    fn loop_range(&self, k: usize) -> Option<std::ops::Range<usize>> {
        let l = self.sketch.vertices.get(k)?.loop_index;
        let start = self
            .sketch
            .vertices
            .iter()
            .position(|v| v.loop_index == l)?;
        let end = self
            .sketch
            .vertices
            .iter()
            .rposition(|v| v.loop_index == l)?
            + 1;
        Some(start..end)
    }

    /// The point in the middle of the edge that starts at `k` (on the arc for an arc).
    fn middle(&self, k: usize) -> Option<P> {
        let (a, b, bulge) = self.at(k)?;
        let flat = flatten_edge(a, b, bulge, DRAW_CHORD);
        if flat.len() > 1 {
            flat.get(flat.len() / 2).copied()
        } else {
            Some([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0])
        }
    }

    fn vertex_at(&self, p: P, reach: f64) -> Option<usize> {
        (0..self.sketch.vertices.len())
            .filter_map(|k| Some((k, dist(self.sketch.vertices.get(k)?.point, p))))
            .filter(|(_, d)| *d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }

    fn middle_at(&self, p: P, reach: f64) -> Option<usize> {
        (0..self.sketch.vertices.len())
            .filter_map(|k| Some((k, dist(self.middle(k)?, p))))
            .filter(|(_, d)| *d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }

    /// The edge nearest to `p` within `reach` and the nearest point of it.
    fn edge_at(&self, p: P, reach: f64) -> Option<(usize, P)> {
        (0..self.sketch.vertices.len())
            .filter_map(|k| {
                let (a, b, bulge) = self.at(k)?;
                let mut pts = flatten_edge(a, b, bulge, DRAW_CHORD);
                pts.push(b);
                pts.windows(2)
                    .map(|w| nearest_on(w[0], w[1], p))
                    .map(|q| (k, q, dist(q, p)))
                    .min_by(|x, y| x.2.total_cmp(&y.2))
            })
            .filter(|(_, _, d)| *d <= reach)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(k, q, _)| (k, q))
    }

    /// A press at `p`: takes a handle for a drag. Returns whether it did.
    pub fn press(&mut self, p: P, ppu: f64) -> bool {
        if self.mode != Mode::Select {
            return false;
        }
        let reach = HIT_PIXELS / ppu.max(1.0e-9);
        if let Some(k) = self.vertex_at(p, reach) {
            self.selected = Some(Sel::Vertex(k));
            self.drag = Some(Drag::Vertex(k));
            return true;
        }
        if let Some(k) = self.middle_at(p, reach) {
            self.selected = Some(Sel::Edge(k));
            self.drag = Some(Drag::Edge(k, p));
            return true;
        }
        false
    }

    /// The pointer at `p` while a drag runs.
    pub fn moved(&mut self, p: P) -> bool {
        let freed = match self.drag {
            Some(Drag::Vertex(k)) => self.sketch.move_vertex(k, p).freed,
            Some(Drag::Edge(k, from)) => {
                let by = [p[0] - from[0], p[1] - from[1]];
                self.drag = Some(Drag::Edge(k, p));
                self.sketch.move_edge(k, by).freed
            }
            None => return false,
        };
        self.freed = self.freed.max(freed);
        self.changed = true;
        true
    }

    /// The button went up: the drag ends.
    pub fn release(&mut self) -> bool {
        self.drag.take().is_some()
    }

    /// A click at `p`: what it does depends on the mode. `wall_at` finds the wall under a point
    /// with its data.
    pub fn click(&mut self, p: P, ppu: f64, wall_at: &dyn Fn(P) -> Option<(ObjectId, Wall)>) {
        let reach = HIT_PIXELS / ppu.max(1.0e-9);
        match self.mode {
            Mode::Select => {
                self.selected = self
                    .vertex_at(p, reach)
                    .map(Sel::Vertex)
                    .or_else(|| self.middle_at(p, reach).map(Sel::Edge))
                    .or_else(|| self.edge_at(p, reach).map(|(k, _)| Sel::Edge(k)));
            }
            Mode::Add => {
                if let Some((k, q)) = self.edge_at(p, reach)
                    && self.vertex_at(q, reach).is_none()
                    && let Some(new) = self.sketch.insert_vertex(k, q)
                {
                    self.selected = Some(Sel::Vertex(new));
                    self.changed = true;
                    // The next press drags the new vertex.
                    self.mode = Mode::Select;
                }
            }
            Mode::Delete => {
                if let Some(k) = self.vertex_at(p, reach) {
                    self.selected = Some(Sel::Vertex(k));
                    if !self.delete_selected() {
                        self.hint = Some(Hint::LoopTooSmall);
                    }
                }
            }
            Mode::Arc => match self.arc_edge {
                None => {
                    self.arc_edge = self.edge_at(p, reach.max(0.05)).map(|(k, _)| k);
                    self.selected = self.arc_edge.map(Sel::Edge);
                }
                Some(k) => {
                    if let Some(c) = self.sketch.set_arc_through(k, p) {
                        self.freed += c.freed;
                        self.changed = true;
                    }
                    self.arc_edge = None;
                }
            },
            Mode::Wall => {
                // A handle or a click on an edge selects it; a wall then replaces the edge.
                if let Some(k) = self
                    .middle_at(p, reach)
                    .or_else(|| self.vertex_at(p, reach))
                {
                    self.selected = Some(Sel::Edge(k));
                    return;
                }
                let wall = wall_at(p);
                let edge = match self.selected {
                    Some(Sel::Edge(k) | Sel::Vertex(k)) => Some(k),
                    None => None,
                };
                match (wall, edge) {
                    (Some((id, wall)), Some(k)) => {
                        let Some(mid) = self.middle(k) else { return };
                        // The face (and offset 0) nearest to where the edge is now.
                        let signed = wall_side(&wall, mid);
                        let face = [
                            Face::Interior,
                            Face::Exterior,
                            Face::CoreInterior,
                            Face::CoreExterior,
                        ]
                        .into_iter()
                        .min_by(|a, b| {
                            (a.across(&wall) - signed)
                                .abs()
                                .total_cmp(&(b.across(&wall) - signed).abs())
                        })
                        .unwrap_or(Face::Interior);
                        if self.sketch.follow_wall(k, id, face, 0.0) {
                            self.changed = true;
                        }
                    }
                    (wall, _) => match self.edge_at(p, reach) {
                        Some((k, _)) => self.selected = Some(Sel::Edge(k)),
                        None if wall.is_some() => self.hint = Some(Hint::PickEdgeFirst),
                        None => {}
                    },
                }
            }
        }
    }

    /// Forgets the edge picked for an arc.
    pub fn reset_pending(&mut self) {
        self.arc_edge = None;
    }

    /// Takes the points of the vertices of edges that follow walls from the walls `wall_of`
    /// finds, so the sketch shows where the floor would be.
    pub fn refresh(&mut self, wall_of: &dyn Fn(ObjectId) -> Option<Wall>) {
        if let Some(b) = self.sketch.boundary(&self.base, wall_of) {
            let (mut current, mut k) = (0_u32, 0_usize);
            for v in &mut self.sketch.vertices {
                if v.loop_index != current {
                    current = v.loop_index;
                    k = 0;
                }
                if let Some(p) = b.loops.get(v.loop_index as usize).and_then(|l| l.get(k)) {
                    v.point = *p;
                }
                k += 1;
            }
        }
    }

    /// Deletes the selected vertex; `false` when nothing is selected or the loop would be left with
    /// fewer than three vertices.
    pub fn delete_selected(&mut self) -> bool {
        let Some(Sel::Vertex(k)) = self.selected else {
            return false;
        };
        match self.sketch.delete_vertex(k) {
            Some(c) => {
                self.freed += c.freed;
                self.selected = None;
                self.changed = true;
                true
            }
            None => false,
        }
    }

    /// Makes the selected edge straight again.
    pub fn straighten_selected(&mut self) -> bool {
        let Some(Sel::Edge(k) | Sel::Vertex(k)) = self.selected else {
            return false;
        };
        match self.sketch.vertices.get_mut(k) {
            Some(v) if v.bulge != 0.0 => {
                v.bulge = 0.0;
                self.changed = true;
                true
            }
            _ => false,
        }
    }

    /// The boundary of the sketch as it would be applied, or what is wrong with it.
    ///
    /// # Errors
    /// The [`Refusal`] that names what is wrong.
    pub fn check(&self, wall_of: &dyn Fn(ObjectId) -> Option<Wall>) -> Result<Boundary, Refusal> {
        self.sketch.validate(&self.base, wall_of)
    }

    /// The parameters of `archbuilder.floor.edit_boundary` for the sketch.
    #[must_use]
    pub fn params(&self) -> serde_json::Value {
        let vertices: Vec<serde_json::Value> = self
            .sketch
            .vertices
            .iter()
            .map(|v| {
                let mut o = json!({"loop_index": v.loop_index, "point": v.point});
                if v.bulge != 0.0 {
                    o["bulge"] = json!(v.bulge);
                }
                if let Some(a) = v.edge {
                    o["wall"] = json!(a.wall.to_string());
                    o["face"] = json!(a.face.name());
                    o["offset"] = json!(a.offset);
                }
                o
            })
            .collect();
        json!({"floor": self.floor.to_string(), "vertices": vertices})
    }

    /// What to draw: the loops (the outline and the openings, arcs as their chords), the
    /// handles, the selection and, when the sketch is invalid, the outline in the warning color.
    #[must_use]
    pub fn overlays(&self, ppu: f64, valid: bool) -> Vec<ProductOverlay> {
        let z = self.z;
        let p3 = |p: P| Point3::new(p[0], p[1], z + 0.01);
        let half = HANDLE_HALF / ppu.max(1.0e-9);
        let mut out = Vec::new();
        // A translucent fill of the sketch, arcs as their chords.
        let mut rings: Vec<Vec<Point3>> = Vec::new();
        let mut current: Option<u32> = None;
        for (k, v) in self.sketch.vertices.iter().enumerate() {
            if current != Some(v.loop_index) {
                current = Some(v.loop_index);
                rings.push(Vec::new());
            }
            if let (Some(ring), Some((a, b, bulge))) = (rings.last_mut(), self.at(k)) {
                ring.extend(flatten_edge(a, b, bulge, DRAW_CHORD).into_iter().map(p3));
            }
        }
        if valid && !rings.is_empty() {
            out.push(ProductOverlay::Styled {
                color: OverlayColor::Rgba([110, 160, 230, 70]),
                item: Box::new(ProductOverlay::Fill { rings }),
            });
        }
        let outline = if valid {
            OverlayColor::Accent
        } else {
            OverlayColor::Warning
        };
        for k in 0..self.sketch.vertices.len() {
            let Some((a, b, bulge)) = self.at(k) else {
                continue;
            };
            let mut pts: Vec<Point3> = flatten_edge(a, b, bulge, DRAW_CHORD)
                .into_iter()
                .map(p3)
                .collect();
            pts.push(p3(b));
            let linked = self
                .sketch
                .vertices
                .get(k)
                .is_some_and(|v| v.edge.is_some());
            let selected = self.selected == Some(Sel::Edge(k));
            let color = if selected {
                OverlayColor::Rgba([255, 170, 0, 255])
            } else if linked {
                OverlayColor::Rgba([60, 190, 120, 255])
            } else {
                outline
            };
            out.push(ProductOverlay::Styled {
                color,
                item: Box::new(ProductOverlay::Path {
                    points: pts,
                    dashed: linked,
                }),
            });
            if let Some(m) = self.middle(k) {
                out.push(ProductOverlay::Styled {
                    color,
                    item: Box::new(ProductOverlay::Path {
                        points: vec![
                            p3([m[0] - half, m[1]]),
                            p3([m[0], m[1] + half]),
                            p3([m[0] + half, m[1]]),
                            p3([m[0], m[1] - half]),
                            p3([m[0] - half, m[1]]),
                        ],
                        dashed: false,
                    }),
                });
            }
        }
        for (k, v) in self.sketch.vertices.iter().enumerate() {
            let c = v.point;
            let color = if self.selected == Some(Sel::Vertex(k)) {
                OverlayColor::Rgba([255, 170, 0, 255])
            } else {
                outline
            };
            out.push(ProductOverlay::Styled {
                color,
                item: Box::new(ProductOverlay::Path {
                    points: vec![
                        p3([c[0] - half, c[1] - half]),
                        p3([c[0] + half, c[1] - half]),
                        p3([c[0] + half, c[1] + half]),
                        p3([c[0] - half, c[1] + half]),
                        p3([c[0] - half, c[1] - half]),
                    ],
                    dashed: false,
                }),
            });
        }
        out
    }
}

/// The signed distance of `p` from the baseline of `wall`, toward the left of its direction.
fn wall_side(wall: &Wall, p: P) -> f64 {
    let length = wall.length().max(f64::MIN_POSITIVE);
    let u = [
        (wall.end[0] - wall.start[0]) / length,
        (wall.end[1] - wall.start[1]) / length,
    ];
    u[0] * (p[1] - wall.start[1]) - u[1] * (p[0] - wall.start[0])
}
