//! Direct manipulation: dragging the selection, the handles of a selected wall, door, window or
//! group, and the temporary dimensions that can be typed over. The engine is headless: it reads
//! the document and pointer input in the shape the controller has it, keeps the state of one
//! gesture and answers with the commands to run and the overlays to draw. The controller
//! ([`crate::interaction`]) feeds it and runs what it returns, so one gesture is one command and
//! one undo step.
//!
//! # Rules
//!
//! - A press is claimed when it lands on a handle, a temporary dimension's value, or an object
//!   (a door or window, else a wall, else a selected floor); otherwise the viewport's rubber band
//!   starts as before. Shift on the press leaves it to the viewport (add to the selection).
//! - A press and release closer than [`DRAG_PIXELS`] is a click: the viewport's click selects.
//! - **Move.** Dragging a selected object moves the whole selection; dragging an unselected one
//!   selects it first and moves it. The displacement snaps like the Move tool (end points, wall
//!   bodies, tracking lines, directions from the press point, the grid); the dragged walls' own
//!   ends do not attract. Releasing with Ctrl held copies instead of moving. A door or window
//!   dragged alone slides along its wall and stops at the clear span and the neighboring
//!   openings.
//! - **End handles** of a selected wall move one end; the end snaps like a wall end of the wall
//!   tool.
//! - **Top handle** of a selected wall (views that are not from above): the height, in steps of
//!   [`HEIGHT_STEP`], shown as a dimension while dragging.
//! - **Rotation handle** of a selection with walls or floors: the selection turns about the
//!   center of its bounds in steps of 15 degrees (free with Shift).
//! - **Flip handles** of a selected door or window flip its hand and its facing on a click.
//! - **Temporary dimensions.** A selected wall shows the distance between its center line and the
//!   center lines of the nearest parallel walls on either side; a selected door or window shows
//!   the gaps between its edges and the ends of the wall's clear span. A click on a value opens
//!   an input; the typed value moves the object so that the dimension has it.

use std::f64::consts::PI;

use arc_base::ObjectId;
use arc_base::math::Point3;
use arc_doc::Snapshot;
use arc_ui::egui::CursorIcon;
use arc_ui::{DynamicEvent, DynamicField, OverlayColor, ProductOverlay, SnapKind};
use archbuilder_commands::insert::{INSERT_FLIP_FACING, INSERT_FLIP_HAND};
use archbuilder_commands::modify::{MODIFY_COPY, MODIFY_MOVE, MODIFY_ROTATE};
use archbuilder_commands::structure::PROPERTY_SET;
use archbuilder_commands::wall::WALL_MOVE_END;
use archbuilder_model::floor::Floor;
use archbuilder_model::insert::{self, Insert};
use archbuilder_model::spatial;
use archbuilder_model::transform::Xform;
use archbuilder_model::types::{self, DOOR, FLOOR, ROOM, WALL, WINDOW};
use archbuilder_model::units::{LengthUnit, length_text};
use archbuilder_model::wall::Wall;
use serde_json::json;

use crate::snap;
use crate::wall_tool::storey_for;

/// A message of the application, named by identifier.
pub struct Message {
    /// The identifier in the catalogs.
    pub message_id: &'static str,
}

/// The title of the dimension input.
pub const EDIT_PROMPT: Message = Message {
    message_id: "ab-dim-edit-prompt",
};

/// The label of the dimension input's field.
const EDIT_LABEL: Message = Message {
    message_id: "ab-dim-edit-label",
};

/// A pointer travel in pixels beyond which a press is a drag.
pub const DRAG_PIXELS: f64 = 4.0;
/// How far from a handle's center a press still takes it, in pixels.
pub const HIT_PIXELS: f64 = 7.0;
/// Half the size of a handle on the screen, in pixels.
const HANDLE_HALF: f64 = 4.5;
/// How far the rotation handle stands from the center of the selection, in pixels.
const ROTATE_ARM: f64 = 34.0;
/// The steps of a dragged wall height, in meters.
pub const HEIGHT_STEP: f64 = 0.05;
/// The angular step of a dragged rotation, in degrees.
pub const ROTATE_STEP: f64 = 15.0;
/// Walls closer to parallel than this (the sine of the angle) get a dimension.
const PARALLEL: f64 = 0.008;
/// The least overlap, in meters, along a wall for a parallel wall to get a dimension.
const MIN_OVERLAP: f64 = 0.05;
/// The most walls looked at for dimensions and picks.
const MAX_WALLS: usize = 20_000;

/// A view ray: the origin and the direction.
pub type Ray = ([f64; 3], [f64; 3]);

type V2 = [f64; 2];
type V3 = [f64; 3];

fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot3(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn dist2(a: V2, b: V2) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn p3(p: V2, z: f64) -> Point3 {
    Point3::new(p[0], p[1], z)
}

/// What the engine needs to know of the camera.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    /// The pixels that span one model unit on the plane through the camera target.
    pub ppu: f64,
    /// The screen's right direction.
    pub right: V3,
    /// The screen's up direction.
    pub up: V3,
    /// The viewing direction.
    pub forward: V3,
    /// The eye of a perspective camera; `None` for an orthographic one.
    pub eye: Option<V3>,
    /// The distance from the eye to the camera target, for a perspective camera.
    pub target_depth: f64,
    /// Whether the camera looks (nearly) straight down.
    pub top: bool,
}

impl Frame {
    /// The model length of one pixel at `p`.
    fn px(&self, p: V3) -> f64 {
        let ppu = match self.eye {
            Some(eye) => {
                let depth = dot3(sub3(p, eye), self.forward).max(1.0e-6);
                self.ppu * self.target_depth / depth
            }
            None => self.ppu,
        };
        1.0 / ppu.max(1.0e-9)
    }

    /// The screen offset in pixels (x right, y down) from `p` to where `ray` passes it.
    fn offset(&self, ray: &Ray, p: V3) -> V2 {
        let (o, d) = *ray;
        let along = dot3(d, self.forward);
        if along.abs() < 1.0e-12 {
            return [f64::INFINITY, f64::INFINITY];
        }
        let t = dot3(sub3(p, o), self.forward) / along;
        let m = [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
        let w = sub3(m, p);
        let k = 1.0 / self.px(p);
        [dot3(w, self.right) * k, -dot3(w, self.up) * k]
    }

    /// `p` moved by `x` pixels to the right and `y` pixels up on the screen.
    fn shifted(&self, p: V3, x: f64, y: f64) -> Point3 {
        let k = self.px(p);
        Point3::new(
            p[0] + (self.right[0] * x + self.up[0] * y) * k,
            p[1] + (self.right[1] * x + self.up[1] * y) * k,
            p[2] + (self.right[2] * x + self.up[2] * y) * k,
        )
    }
}

/// The kind of a handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    /// One end of a wall's baseline: 0 the start, 1 the end.
    End {
        /// The wall.
        wall: ObjectId,
        /// The end.
        end: usize,
    },
    /// The top of a wall.
    Top {
        /// The wall.
        wall: ObjectId,
    },
    /// The rotation of the selection.
    Rotate,
    /// The hand of a door or window.
    FlipHand(ObjectId),
    /// The facing of a door or window.
    FlipFacing(ObjectId),
}

/// A handle: its kind and its place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Handle {
    /// The kind.
    pub kind: HandleKind,
    /// The model point.
    pub at: V3,
}

/// A temporary dimension: the distance between a point that stays and a point of the object that
/// moves, which a typed value changes by moving the object along the line between them.
#[derive(Debug, Clone, PartialEq)]
pub struct Dim {
    /// The point that stays.
    pub fixed: V2,
    /// The point of the object.
    pub moving: V2,
    /// The elevation the dimension is drawn at.
    pub z: f64,
    /// The object that moves.
    pub target: ObjectId,
    /// Where the line of the dimension is drawn, as a distance from the fixed point toward the
    /// moving point's side of the object (the offset of the drawn line from the measured points).
    shift: V2,
    /// Where along the line the value is written, from the fixed point (0) to the moving one (1).
    label_at: f64,
}

impl Dim {
    /// The distance in meters.
    #[must_use]
    pub fn value(&self) -> f64 {
        dist2(self.fixed, self.moving)
    }

    fn direction(&self) -> V2 {
        let v = self.value();
        if v > 1.0e-9 {
            [
                (self.moving[0] - self.fixed[0]) / v,
                (self.moving[1] - self.fixed[1]) / v,
            ]
        } else {
            [1.0, 0.0]
        }
    }

    fn a(&self) -> V2 {
        [self.fixed[0] + self.shift[0], self.fixed[1] + self.shift[1]]
    }

    fn b(&self) -> V2 {
        [
            self.moving[0] + self.shift[0],
            self.moving[1] + self.shift[1],
        ]
    }

    fn anchor(&self) -> V3 {
        let (a, b) = (self.a(), self.b());
        let t = self.label_at;
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, self.z]
    }
}

/// A command to run.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    /// The command identifier.
    pub name: &'static str,
    /// Its parameters.
    pub params: serde_json::Value,
}

/// What a finished gesture asks for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Done {
    /// The commands to run, in order.
    pub commands: Vec<Command>,
}

/// What the engine reads with each pointer event.
pub struct Input<'a> {
    /// The document.
    pub snapshot: &'a Snapshot,
    /// The tab's selection.
    pub selection: &'a [ObjectId],
    /// The camera.
    pub frame: Frame,
    /// The view ray through the pointer.
    pub ray: Option<Ray>,
    /// The pixel.
    pub pixel: V2,
    /// The point of the work plane under the pointer.
    pub at: Option<V2>,
    /// Whether Shift is held.
    pub shift: bool,
    /// Whether Ctrl is held.
    pub ctrl: bool,
    /// Whether snapping is off for this event (Alt held).
    pub free: bool,
    /// The project's length unit.
    pub unit: LengthUnit,
    /// The decimal separator of the interface language.
    pub separator: char,
}

/// What the pointer is over.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Handle(HandleKind),
    Dim(usize),
    Body(ObjectId),
}

#[derive(Debug, Clone)]
enum What {
    Move {
        objects: Vec<ObjectId>,
        pick: Option<ObjectId>,
    },
    End {
        wall: ObjectId,
        end: usize,
    },
    Top {
        wall: ObjectId,
    },
    Rotate {
        objects: Vec<ObjectId>,
        center: V2,
    },
    Click(Hit),
}

/// What a drag in progress would do on release.
#[derive(Debug, Clone, Default)]
struct Out {
    delta: V2,
    to: V2,
    height: f64,
    angle: f64,
    copy: bool,
    ghost: Vec<ProductOverlay>,
    guides: Vec<[V2; 2]>,
    snap: Option<(SnapKind, V2)>,
    label: Option<(Point3, String)>,
    valid: bool,
}

#[derive(Debug, Clone)]
struct Drag {
    what: What,
    start_px: V2,
    start_at: V2,
    z: f64,
    live: bool,
    out: Out,
}

/// The input of a temporary dimension being edited.
#[derive(Debug, Clone)]
struct Edit {
    invalid: bool,
    dim: Dim,
    text: String,
    unit: LengthUnit,
    at: V2,
}

/// The state of direct manipulation of one tab.
#[derive(Debug, Default)]
pub struct Direct {
    drag: Option<Drag>,
    hover: Option<Hit>,
    swallow: bool,
    edit: Option<Edit>,
    select: Option<ObjectId>,
}

// -- Reading the document -------------------------------------------------------------------

fn kind_of(snapshot: &Snapshot, id: ObjectId) -> Option<String> {
    spatial::type_of(snapshot, id)
}

fn is_insert(snapshot: &Snapshot, id: ObjectId) -> bool {
    matches!(kind_of(snapshot, id).as_deref(), Some(DOOR | WINDOW))
}

/// A wall with the elevation of its base.
struct W {
    wall: Wall,
    base: f64,
}

fn wall_of(snapshot: &Snapshot, id: ObjectId) -> Option<W> {
    let object = spatial::get(snapshot, id)?;
    let wall = Wall::of(snapshot, &object)?;
    let level = object
        .header
        .owner
        .and_then(|s| spatial::get(snapshot, s))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    Some(W {
        wall,
        base: level + wall.base_offset,
    })
}

/// The unit direction along a wall and its left normal; `None` for a wall with no length.
fn axes(wall: &Wall) -> Option<(V2, V2)> {
    let length = wall.length();
    (length > 0.0).then(|| {
        let u = [
            (wall.end[0] - wall.start[0]) / length,
            (wall.end[1] - wall.start[1]) / length,
        ];
        (u, [-u[1], u[0]])
    })
}

/// The point of a wall at `along` from its start and `across` to the left.
fn at_wall(wall: &Wall, along: f64, across: f64) -> V2 {
    let (u, n) = axes(wall).unwrap_or(([1.0, 0.0], [0.0, 1.0]));
    [
        wall.start[0] + u[0] * along + n[0] * across,
        wall.start[1] + u[1] * along + n[1] * across,
    ]
}

/// Where the middle of the wall's thickness lies, as a distance to the left of the baseline.
fn middle(wall: &Wall) -> f64 {
    let (low, high) = wall.extent();
    (low + high) / 2.0
}

/// The kinds a move works on.
fn movable(snapshot: &Snapshot, ids: &[ObjectId]) -> Vec<ObjectId> {
    ids.iter()
        .copied()
        .filter(|id| {
            matches!(
                kind_of(snapshot, *id).as_deref(),
                Some(WALL | FLOOR | ROOM | DOOR | WINDOW)
            )
        })
        .take(archbuilder_commands::modify::MAX_OBJECTS)
        .collect()
}

/// The points of an object, for the center of a turn.
fn points_of(snapshot: &Snapshot, id: ObjectId) -> Vec<V2> {
    let Some(object) = spatial::get(snapshot, id) else {
        return Vec::new();
    };
    if let Some(wall) = Wall::from_values(&object.values) {
        return vec![wall.start, wall.end];
    }
    if let Some(boundary) = Floor::from_values(&object.values).and_then(|f| f.boundary_in(snapshot))
    {
        return boundary.loops.iter().flatten().copied().collect();
    }
    archbuilder_model::room::Room::from_values(&object.values)
        .map(|r| vec![r.point])
        .unwrap_or_default()
}

/// The objects a turn works on and the center of their bounds; `None` unless a wall or floor is
/// among them.
fn rotation_of(snapshot: &Snapshot, selection: &[ObjectId]) -> Option<(Vec<ObjectId>, V2, f64)> {
    let objects: Vec<ObjectId> = selection
        .iter()
        .copied()
        .filter(|id| matches!(kind_of(snapshot, *id).as_deref(), Some(WALL | FLOOR | ROOM)))
        .take(archbuilder_commands::modify::MAX_OBJECTS)
        .collect();
    if !objects
        .iter()
        .any(|id| matches!(kind_of(snapshot, *id).as_deref(), Some(WALL | FLOOR)))
    {
        return None;
    }
    let mut min = [f64::INFINITY; 2];
    let mut max = [f64::NEG_INFINITY; 2];
    for p in objects.iter().flat_map(|id| points_of(snapshot, *id)) {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    let center = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
    if !center.iter().all(|v| v.is_finite()) {
        return None;
    }
    let (_, z) = storey_for(snapshot, &objects)?;
    Some((objects, center, z))
}

/// The handles of the selection.
#[must_use]
pub fn handles(snapshot: &Snapshot, selection: &[ObjectId], frame: &Frame) -> Vec<Handle> {
    let mut out = Vec::new();
    if let [id] = selection {
        match kind_of(snapshot, *id).as_deref() {
            Some(WALL) => {
                if let Some(w) = wall_of(snapshot, *id) {
                    for (end, p) in [w.wall.start, w.wall.end].into_iter().enumerate() {
                        out.push(Handle {
                            kind: HandleKind::End { wall: *id, end },
                            at: [p[0], p[1], w.base],
                        });
                    }
                    if !frame.top {
                        let m = at_wall(&w.wall, w.wall.length() / 2.0, middle(&w.wall));
                        out.push(Handle {
                            kind: HandleKind::Top { wall: *id },
                            at: [m[0], m[1], w.base + w.wall.height],
                        });
                    }
                }
            }
            Some(DOOR | WINDOW) => {
                let parts = spatial::get(snapshot, *id).and_then(|o| {
                    let item = Insert::of(snapshot, &o)?;
                    let host = o.header.owner?;
                    Some((item, wall_of(snapshot, host)?))
                });
                if let Some((item, w)) = parts {
                    let (low, high) = w.wall.extent();
                    let z = w.base + item.sill + item.height / 2.0;
                    let side = if item.facing == insert::Facing::Exterior {
                        high + 0.25
                    } else {
                        low - 0.25
                    };
                    let hand = at_wall(&w.wall, item.distance + item.width / 2.0 + 0.2, 0.0);
                    let facing = at_wall(&w.wall, item.distance, side);
                    out.push(Handle {
                        kind: HandleKind::FlipHand(*id),
                        at: [hand[0], hand[1], z],
                    });
                    out.push(Handle {
                        kind: HandleKind::FlipFacing(*id),
                        at: [facing[0], facing[1], z],
                    });
                }
            }
            _ => {}
        }
    }
    if let Some((_, center, z)) = rotation_of(snapshot, selection) {
        let up = [frame.up[0], frame.up[1]];
        let n = up[0].hypot(up[1]);
        let up = if n > 1.0e-6 {
            [up[0] / n, up[1] / n]
        } else {
            [0.0, 1.0]
        };
        let arm = ROTATE_ARM * frame.px([center[0], center[1], z]);
        out.push(Handle {
            kind: HandleKind::Rotate,
            at: [center[0] + up[0] * arm, center[1] + up[1] * arm, z],
        });
    }
    out
}

/// The temporary dimensions of the selection: of a single wall or a single door or window.
#[must_use]
pub fn dims(snapshot: &Snapshot, selection: &[ObjectId]) -> Vec<Dim> {
    let [id] = selection else {
        return Vec::new();
    };
    match kind_of(snapshot, *id).as_deref() {
        Some(WALL) => wall_dims(snapshot, *id),
        Some(DOOR | WINDOW) => insert_dims(snapshot, *id),
        _ => Vec::new(),
    }
}

fn wall_dims(snapshot: &Snapshot, id: ObjectId) -> Vec<Dim> {
    let (Some(w), Some(owner)) = (
        wall_of(snapshot, id),
        spatial::get(snapshot, id).and_then(|o| o.header.owner),
    ) else {
        return Vec::new();
    };
    let Some((u, n)) = axes(&w.wall) else {
        return Vec::new();
    };
    let length = w.wall.length();
    let center = |p: V2| (p[0] - w.wall.start[0]) * u[0] + (p[1] - w.wall.start[1]) * u[1];
    let across = |p: V2| (p[0] - w.wall.start[0]) * n[0] + (p[1] - w.wall.start[1]) * n[1];
    let mid = middle(&w.wall);
    // The nearest parallel wall on the left (positive) and on the right (negative).
    let mut best: [Option<(f64, f64, f64)>; 2] = [None, None];
    for other in spatial::walls(snapshot, owner).iter().take(MAX_WALLS) {
        if other.id == id {
            continue;
        }
        let Some(v) = Wall::from_values(&other.values) else {
            continue;
        };
        let Some((vu, _)) = axes(&v) else {
            continue;
        };
        if (u[0] * vu[1] - u[1] * vu[0]).abs() > PARALLEL {
            continue;
        }
        let (s, e) = (center(v.start), center(v.end));
        let (from, to) = (s.min(e).max(0.0), s.max(e).min(length));
        if to - from < MIN_OVERLAP {
            continue;
        }
        let q = across(at_wall(&v, 0.0, middle(&v))) - mid;
        if q.abs() < 1.0e-4 {
            continue;
        }
        let side = usize::from(q < 0.0);
        if best[side].is_none_or(|(d, _, _)| q.abs() < d) {
            best[side] = Some((q.abs(), (from + to) / 2.0, q));
        }
    }
    best.into_iter()
        .flatten()
        .map(|(_, along, q)| {
            let moving = at_wall(&w.wall, along, mid);
            let fixed = at_wall(&w.wall, along, mid + q);
            Dim {
                fixed,
                moving,
                z: w.base,
                target: id,
                shift: [0.0, 0.0],
                // Off the middle of the room, where its tag usually stands.
                label_at: 0.25,
            }
        })
        .collect()
}

fn insert_dims(snapshot: &Snapshot, id: ObjectId) -> Vec<Dim> {
    let Some(object) = spatial::get(snapshot, id) else {
        return Vec::new();
    };
    let Some(item) = Insert::of(snapshot, &object) else {
        return Vec::new();
    };
    let Some(host) = object.header.owner.and_then(|h| spatial::get(snapshot, h)) else {
        return Vec::new();
    };
    let (Some(w), Some((low, high))) = (
        wall_of(snapshot, host.id),
        insert::clear_span_of(snapshot, &host),
    ) else {
        return Vec::new();
    };
    let (from, to) = item.extent();
    let (_, face) = w.wall.extent();
    let across = face + 0.3;
    let mut out = Vec::new();
    for (fixed, moving) in [(low, from), (high, to)] {
        if (moving - fixed).abs() > 1.0e-6 {
            out.push(Dim {
                fixed: at_wall(&w.wall, fixed, middle(&w.wall)),
                moving: at_wall(&w.wall, moving, middle(&w.wall)),
                z: w.base,
                target: id,
                label_at: 0.5,
                shift: {
                    let o = at_wall(&w.wall, 0.0, across);
                    let c = at_wall(&w.wall, 0.0, middle(&w.wall));
                    [o[0] - c[0], o[1] - c[1]]
                },
            });
        }
    }
    out
}

/// The object a press at the pointer takes: a door or window, else the nearest wall, else a
/// selected floor.
pub fn body_at(i: &Input<'_>) -> Option<ObjectId> {
    let ray = i.ray?;
    if let Some(id) = insert::pick(i.snapshot, ray) {
        return Some(id);
    }
    let (o, d) = ray;
    let aperture =
        i.at.map_or(0.0, |a| HIT_PIXELS * i.frame.px([a[0], a[1], o[2]]));
    let mut best: Option<(f64, ObjectId)> = None;
    for object in spatial::objects(i.snapshot, WALL).iter().take(MAX_WALLS) {
        let Some(w) = wall_of(i.snapshot, object.id) else {
            continue;
        };
        let Some((u, n)) = axes(&w.wall) else {
            continue;
        };
        let rel = sub3(o, [w.wall.start[0], w.wall.start[1], 0.0]);
        let local_o = [
            dot3(rel, [u[0], u[1], 0.0]),
            dot3(rel, [n[0], n[1], 0.0]),
            o[2],
        ];
        let local_d = [dot3(d, [u[0], u[1], 0.0]), dot3(d, [n[0], n[1], 0.0]), d[2]];
        let (low, high) = w.wall.extent();
        let a = aperture.min(0.5);
        let min = [-a, low - a, w.base];
        let max = [w.wall.length() + a, high + a, w.base + w.wall.height];
        if let Some(t) = insert::enters(local_o, local_d, min, max)
            && best.is_none_or(|(b, _)| t < b)
        {
            best = Some((t, object.id));
        }
    }
    if let Some((_, id)) = best {
        return Some(id);
    }
    let at = i.at?;
    archbuilder_model::pick::stack(i.snapshot, &[], at, aperture, Some(ray))
        .into_iter()
        .find(|id| kind_of(i.snapshot, *id).as_deref() == Some(FLOOR) && i.selection.contains(id))
}

/// The baselines of the walls of the storey `owner` is on, without `exclude`.
fn baselines(snapshot: &Snapshot, objects: &[ObjectId], exclude: &[ObjectId]) -> Vec<[V2; 2]> {
    let Some((storey, _)) = storey_for(snapshot, objects) else {
        return Vec::new();
    };
    spatial::walls(snapshot, storey)
        .iter()
        .take(MAX_WALLS)
        .filter(|o| !exclude.contains(&o.id))
        .filter_map(|o| Wall::from_values(&o.values))
        .map(|w| [w.start, w.end])
        .collect()
}

fn snap_kind(kind: snap::Kind) -> Option<SnapKind> {
    match kind {
        snap::Kind::Endpoint => Some(SnapKind::Endpoint),
        snap::Kind::Intersection => Some(SnapKind::Intersection),
        snap::Kind::Nearest => Some(SnapKind::Nearest),
        _ => None,
    }
}

/// The outlines of what a motion would leave, dashed: the walls as they would stand (neighbors
/// that keep a join included) and, for a copy, the copies; floors' outlines.
fn ghost(
    snapshot: &Snapshot,
    objects: &[ObjectId],
    xf: &Xform,
    z: f64,
    copy: bool,
) -> Vec<ProductOverlay> {
    let closed = |points: &[V2]| ProductOverlay::Path {
        points: points
            .iter()
            .chain(points.first())
            .map(|p| p3(*p, z))
            .collect(),
        dashed: true,
    };
    let mut out = Vec::new();
    if !copy {
        for wall in archbuilder_commands::modify::preview(snapshot, objects, xf) {
            out.push(closed(&wall.plain_footprint()));
        }
    }
    for id in objects.iter().take(2000) {
        let Some(object) = spatial::get(snapshot, *id) else {
            continue;
        };
        if let Some(wall) = Wall::from_values(&object.values) {
            if copy {
                let moved: Vec<V2> = wall
                    .plain_footprint()
                    .iter()
                    .map(|p| xf.point(*p))
                    .collect();
                out.push(closed(&moved));
            }
        } else if let Some(boundary) =
            Floor::from_values(&object.values).and_then(|f| f.boundary_in(snapshot))
        {
            for ring in &boundary.loops {
                let moved: Vec<V2> = ring.iter().map(|p| xf.point(*p)).collect();
                out.push(closed(&moved));
            }
        }
    }
    out
}

/// A door or window sliding along its wall: the range its center may take.
struct Slide {
    item: Insert,
    wall: Wall,
    lo: f64,
    hi: f64,
    level: f64,
}

fn slide_of(snapshot: &Snapshot, id: ObjectId) -> Option<Slide> {
    let object = spatial::get(snapshot, id)?;
    let item = Insert::of(snapshot, &object)?;
    let host = spatial::get(snapshot, object.header.owner?)?;
    let wall = Wall::of(snapshot, &host)?;
    let (low, high) = insert::clear_span_of(snapshot, &host)?;
    let level = host
        .header
        .owner
        .and_then(|s| spatial::get(snapshot, s))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    let eps = 1.0e-6;
    let half = item.width / 2.0;
    let mut lo = low + insert::SIDE_CLEARANCE + half + eps;
    let mut hi = high - insert::SIDE_CLEARANCE - half - eps;
    for (other, theirs) in insert::hosted(snapshot, host.id) {
        if other.id == id {
            continue;
        }
        let (from, to) = theirs.extent();
        if theirs.distance <= item.distance {
            lo = lo.max(to + insert::MIN_GAP + half + eps);
        } else {
            hi = hi.min(from - insert::MIN_GAP - half - eps);
        }
    }
    Some(Slide {
        item,
        wall,
        lo,
        hi,
        level,
    })
}

impl Slide {
    /// The distance after a displacement, within the range (the old one when the range is empty).
    fn distance_after(&self, delta: V2) -> f64 {
        let Some((u, _)) = axes(&self.wall) else {
            return self.item.distance;
        };
        let wanted = self.item.distance + delta[0] * u[0] + delta[1] * u[1];
        if self.lo > self.hi {
            return self.item.distance;
        }
        // Whole centimeters when they stay in range, else the edge of the range itself.
        let rounded = (wanted * 100.0).round() / 100.0;
        if (self.lo..=self.hi).contains(&rounded) {
            rounded
        } else {
            wanted.clamp(self.lo, self.hi)
        }
    }

    /// The displacement along the baseline that gives `distance`.
    fn displacement_to(&self, distance: f64) -> V2 {
        let Some((u, _)) = axes(&self.wall) else {
            return [0.0, 0.0];
        };
        let d = distance - self.item.distance;
        [u[0] * d, u[1] * d]
    }

    fn outline(&self, distance: f64) -> Vec<ProductOverlay> {
        let moved = Insert {
            distance,
            ..self.item
        };
        insert::outline(&self.wall, self.level, &moved)
            .unwrap_or_default()
            .into_iter()
            .map(|line| ProductOverlay::Path {
                points: line.iter().map(|p| Point3::new(p[0], p[1], p[2])).collect(),
                dashed: true,
            })
            .collect()
    }
}

fn ids_of(objects: &[ObjectId]) -> Vec<String> {
    objects.iter().map(ToString::to_string).collect()
}

/// Reads a length typed in a field: a number with an optional unit suffix, in the project's unit
/// without one; the decimal separator is a point or a comma.
#[must_use]
pub fn parse_length(text: &str, unit: LengthUnit) -> Option<f64> {
    let text = text.trim().replace(',', ".");
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
        .unwrap_or(text.len());
    let (number, suffix) = text.split_at(end);
    let value: f64 = number.parse().ok().filter(|v: &f64| v.is_finite())?;
    let factor = match suffix.trim().to_ascii_lowercase().as_str() {
        "" => match unit {
            LengthUnit::Meter => 1.0,
            LengthUnit::Millimeter => 0.001,
            LengthUnit::Centimeter => 0.01,
            LengthUnit::Foot => 0.3048,
            LengthUnit::Inch => 0.0254,
        },
        "m" => 1.0,
        "mm" => 0.001,
        "cm" => 0.01,
        "ft" | "'" => 0.3048,
        "in" | "\"" => 0.0254,
        _ => return None,
    };
    Some(value * factor)
}

impl Direct {
    /// A new engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a gesture or an input is in progress.
    #[must_use]
    pub fn active(&self) -> bool {
        self.drag.is_some() || self.edit.is_some()
    }

    /// Whether a drag moved beyond the click distance.
    #[must_use]
    pub fn dragging(&self) -> bool {
        self.drag.as_ref().is_some_and(|d| d.live)
    }

    /// The object to select now: the unselected object a drag took, once it moves. Taken once.
    pub fn take_select(&mut self) -> Option<ObjectId> {
        self.select.take()
    }

    /// Whether the click that follows a claimed press is the engine's: the controller leaves it
    /// alone. Taken once.
    pub fn take_swallow(&mut self) -> bool {
        std::mem::take(&mut self.swallow)
    }

    /// Ends a gesture or an input (Escape). Whether there was one.
    pub fn cancel(&mut self) -> bool {
        self.select = None;
        let had = self.drag.is_some() || self.edit.is_some();
        self.drag = None;
        self.edit = None;
        had
    }

    /// The mouse pointer to show.
    #[must_use]
    pub fn icon(&self, selection: &[ObjectId]) -> Option<CursorIcon> {
        if let Some(d) = self.drag.as_ref().filter(|d| d.live) {
            return Some(match d.what {
                What::Top { .. } => CursorIcon::ResizeVertical,
                What::Move { .. } => CursorIcon::Grabbing,
                _ => CursorIcon::Crosshair,
            });
        }
        match self.hover? {
            Hit::Handle(HandleKind::Top { .. }) => Some(CursorIcon::ResizeVertical),
            Hit::Handle(HandleKind::FlipHand(_) | HandleKind::FlipFacing(_)) | Hit::Dim(_) => {
                Some(CursorIcon::PointingHand)
            }
            Hit::Handle(_) => Some(CursorIcon::Grab),
            Hit::Body(id) if selection.contains(&id) => Some(CursorIcon::Move),
            Hit::Body(_) => None,
        }
    }

    fn hit(&self, i: &Input<'_>) -> Option<Hit> {
        let ray = i.ray?;
        for h in handles(i.snapshot, i.selection, &i.frame) {
            let [x, y] = i.frame.offset(&ray, h.at);
            if x.hypot(y) <= HIT_PIXELS {
                return Some(Hit::Handle(h.kind));
            }
        }
        for (k, d) in dims(i.snapshot, i.selection).iter().enumerate() {
            let [x, y] = i.frame.offset(&ray, d.anchor());
            let text = length_text(d.value(), i.unit, i.separator);
            // The label's plate sits up and to the right of its anchor.
            #[allow(clippy::cast_precision_loss, reason = "a few characters")]
            let width = 7.0 * text.chars().count() as f64;
            if (4.0..=14.0 + width).contains(&x) && (-28.0..=-2.0).contains(&y) {
                return Some(Hit::Dim(k));
            }
        }
        body_at(i).map(Hit::Body)
    }

    /// The hover under the pointer, without a button. Whether the pointer is over a handle or a
    /// dimension, which the controller then takes the move for.
    pub fn hover(&mut self, i: &Input<'_>) -> bool {
        self.hover = self.hit(i);
        matches!(self.hover, Some(Hit::Handle(_) | Hit::Dim(_)))
    }

    /// A press of the primary button. Whether the engine claims it.
    pub fn press(&mut self, i: &Input<'_>) -> bool {
        self.edit = None;
        self.swallow = false;
        if i.shift {
            return false;
        }
        let Some(at) = i.at else {
            return false;
        };
        let Some(hit) = self.hit(i) else {
            return false;
        };
        let z = storey_for(i.snapshot, i.selection).map_or(0.0, |(_, z)| z);
        let what = match hit {
            Hit::Handle(HandleKind::End { wall, end }) => What::End { wall, end },
            Hit::Handle(HandleKind::Top { wall }) => What::Top { wall },
            Hit::Handle(HandleKind::Rotate) => {
                let Some((objects, center, _)) = rotation_of(i.snapshot, i.selection) else {
                    return false;
                };
                What::Rotate { objects, center }
            }
            Hit::Handle(kind) => What::Click(Hit::Handle(kind)),
            Hit::Dim(k) => What::Click(Hit::Dim(k)),
            Hit::Body(id) if i.selection.contains(&id) => {
                let objects = movable(i.snapshot, i.selection);
                if objects.is_empty() {
                    return false;
                }
                What::Move {
                    objects,
                    pick: None,
                }
            }
            Hit::Body(id) => {
                if movable(i.snapshot, &[id]).is_empty() {
                    return false;
                }
                What::Move {
                    objects: vec![id],
                    pick: Some(id),
                }
            }
        };
        self.drag = Some(Drag {
            what,
            start_px: i.pixel,
            start_at: at,
            z,
            live: false,
            out: Out::default(),
        });
        true
    }

    /// A move of the pointer. Whether the engine takes it.
    pub fn moved(&mut self, i: &Input<'_>) -> bool {
        let Some(mut drag) = self.drag.take() else {
            return self.hover(i);
        };
        if !drag.live && dist2(i.pixel, drag.start_px) > DRAG_PIXELS {
            drag.live = true;
            if let What::Move { pick, .. } = &drag.what {
                self.select = *pick;
            }
        }
        if drag.live {
            Self::update(&mut drag, i);
        }
        self.drag = Some(drag);
        true
    }

    #[allow(clippy::too_many_lines, reason = "one arm per kind of drag")]
    fn update(drag: &mut Drag, i: &Input<'_>) {
        let Some(at) = i.at else {
            return;
        };
        let ppu = Some(i.frame.ppu);
        let snapshot = i.snapshot;
        let mut out = Out::default();
        match &drag.what {
            What::Move { objects, .. } => {
                let single = match objects.as_slice() {
                    [one] if is_insert(snapshot, *one) => {
                        slide_of(snapshot, *one).map(|s| (*one, s))
                    }
                    _ => None,
                };
                if let Some((_, slide)) = single {
                    let raw = [at[0] - drag.start_at[0], at[1] - drag.start_at[1]];
                    let distance = slide.distance_after(raw);
                    out.delta = slide.displacement_to(distance);
                    out.ghost = slide.outline(distance);
                    out.valid = (distance - slide.item.distance).abs() > 1.0e-9;
                } else {
                    let walls: Vec<ObjectId> = objects
                        .iter()
                        .copied()
                        .filter(|id| kind_of(snapshot, *id).as_deref() == Some(WALL))
                        .collect();
                    let lines = baselines(snapshot, objects, &walls);
                    let mut ends = vec![[0.0, 0.0]];
                    ends.extend(lines.iter().flatten().copied());
                    let at_of = |pointer: V2, from: Option<V2>| {
                        snap::snap(&snap::Input {
                            pointer,
                            pixels_per_unit: ppu,
                            ends: &ends,
                            walls: &lines,
                            from,
                        })
                    };
                    let base = at_of(drag.start_at, None)
                        .filter(|_| !i.free)
                        .map_or(drag.start_at, |h| h.point);
                    let hit = at_of(at, Some(base)).filter(|_| !i.free);
                    let to = hit.as_ref().map_or(at, |h| h.point);
                    out.delta = [to[0] - base[0], to[1] - base[1]];
                    out.guides = hit.as_ref().map_or_else(Vec::new, |h| h.guides.clone());
                    out.snap = hit
                        .as_ref()
                        .and_then(|h| snap_kind(h.kind).map(|k| (k, h.point)));
                    out.valid = dist2(to, base) > 1.0e-9;
                    out.copy = i.ctrl;
                    let xf = Xform::Translate(out.delta);
                    out.ghost = ghost(snapshot, objects, &xf, drag.z, i.ctrl);
                    out.ghost.push(ProductOverlay::Line {
                        from: p3(base, drag.z),
                        to: p3(to, drag.z),
                    });
                }
                out.copy = i.ctrl;
                let shown = length_text(out.delta[0].hypot(out.delta[1]), i.unit, i.separator);
                out.label = Some((p3(at, drag.z), shown));
            }
            What::End { wall, end } => {
                let Some(w) = wall_of(snapshot, *wall) else {
                    return;
                };
                let lines = baselines(snapshot, &[*wall], &[*wall]);
                let mut ends = vec![[0.0, 0.0]];
                ends.extend(lines.iter().flatten().copied());
                let fixed = if *end == 0 { w.wall.end } else { w.wall.start };
                let hit = snap::snap(&snap::Input {
                    pointer: at,
                    pixels_per_unit: ppu,
                    ends: &ends,
                    walls: &lines,
                    from: Some(fixed),
                })
                .filter(|_| !i.free);
                let to = hit.as_ref().map_or(at, |h| h.point);
                out.to = to;
                out.guides = hit.as_ref().map_or_else(Vec::new, |h| h.guides.clone());
                out.snap = hit
                    .as_ref()
                    .and_then(|h| snap_kind(h.kind).map(|k| (k, h.point)));
                let moved = if *end == 0 {
                    Wall {
                        start: to,
                        ..w.wall
                    }
                } else {
                    Wall { end: to, ..w.wall }
                };
                out.valid = dist2(to, fixed) > types::MIN_WALL_LENGTH;
                let (dx, dy) = (to[0] - fixed[0], to[1] - fixed[1]);
                let degrees = dy.atan2(dx).to_degrees();
                out.label = Some((
                    p3(to, drag.z),
                    format!(
                        "{}  {degrees:.1}\u{b0}",
                        length_text(dist2(to, fixed), i.unit, i.separator)
                    ),
                ));
                let _ = moved;
                out.ghost = archbuilder_commands::modify::preview_end(snapshot, *wall, *end, to)
                    .iter()
                    .map(|wall| {
                        let f = wall.plain_footprint();
                        ProductOverlay::Path {
                            points: f.iter().chain(f.first()).map(|p| p3(*p, w.base)).collect(),
                            dashed: true,
                        }
                    })
                    .collect();
            }
            What::Top { wall } => {
                let (Some(w), Some((o, d))) = (wall_of(snapshot, *wall), i.ray) else {
                    return;
                };
                let m = at_wall(&w.wall, w.wall.length() / 2.0, middle(&w.wall));
                let w0 = sub3([m[0], m[1], 0.0], o);
                let (b, c, e, dd) = (d[2], dot3(d, d), dot3(d, w0), w0[2]);
                let denom = c - b * b;
                if denom.abs() < 1.0e-12 {
                    return;
                }
                let z = (b * e - c * dd) / denom;
                let step = if i.free { 0.001 } else { HEIGHT_STEP };
                let height = (((z - w.base) / step).round() * step)
                    .clamp(types::HEIGHT_RANGE.0, types::HEIGHT_RANGE.1);
                out.height = height;
                out.valid = (height - w.wall.height).abs() > 1.0e-9;
                out.label = Some((
                    Point3::new(m[0], m[1], w.base + height),
                    length_text(height, i.unit, i.separator),
                ));
                let top = w.base + height;
                // The wall as a ghost box at the new height: the footprint at the base and the top,
                // and the four edges between them.
                let f = w.wall.plain_footprint();
                for z in [w.base, top] {
                    out.ghost.push(ProductOverlay::Path {
                        points: f.iter().chain(f.first()).map(|p| p3(*p, z)).collect(),
                        dashed: true,
                    });
                }
                for p in f {
                    out.ghost.push(ProductOverlay::Line {
                        from: p3(p, w.base),
                        to: p3(p, top),
                    });
                }
            }
            What::Rotate { objects, center } => {
                let first = (drag.start_at[1] - center[1]).atan2(drag.start_at[0] - center[0]);
                let now = (at[1] - center[1]).atan2(at[0] - center[0]);
                let mut degrees = (now - first).to_degrees();
                degrees = (degrees + 180.0).rem_euclid(360.0) - 180.0;
                if !i.shift && !i.free {
                    degrees = (degrees / ROTATE_STEP).round() * ROTATE_STEP;
                }
                out.angle = degrees.to_radians();
                out.valid = degrees.abs() > 1.0e-9;
                let xf = Xform::Rotate {
                    base: *center,
                    angle: out.angle,
                };
                out.ghost = ghost(snapshot, objects, &xf, drag.z, false);
                out.ghost.push(ProductOverlay::Line {
                    from: p3(*center, drag.z),
                    to: p3(at, drag.z),
                });
                out.label = Some((p3(at, drag.z), format!("{degrees:.0}\u{b0}")));
            }
            What::Click(_) => {}
        }
        drag.out = out;
    }

    /// A release of the primary button: the commands the gesture ends in, if any.
    pub fn release(&mut self, i: &Input<'_>) -> Option<Done> {
        let drag = self.drag.take()?;
        if !drag.live {
            // A click on a handle or a value acts; a click on an object is the viewport's.
            return match drag.what {
                What::Click(Hit::Handle(HandleKind::FlipHand(id))) => {
                    self.swallow = true;
                    Some(Done {
                        commands: vec![Command {
                            name: INSERT_FLIP_HAND,
                            params: json!({"inserts": [id.to_string()]}),
                        }],
                    })
                }
                What::Click(Hit::Handle(HandleKind::FlipFacing(id))) => {
                    self.swallow = true;
                    Some(Done {
                        commands: vec![Command {
                            name: INSERT_FLIP_FACING,
                            params: json!({"inserts": [id.to_string()]}),
                        }],
                    })
                }
                What::Click(Hit::Dim(k)) => {
                    self.swallow = true;
                    if let Some(dim) = dims(i.snapshot, i.selection).into_iter().nth(k) {
                        let text = length_text(dim.value(), i.unit, i.separator);
                        self.edit = Some(Edit {
                            invalid: false,
                            at: i.at.unwrap_or(dim.moving),
                            dim,
                            text,
                            unit: i.unit,
                        });
                    }
                    None
                }
                What::Move { .. } => None,
                _ => {
                    self.swallow = true;
                    None
                }
            };
        }
        self.swallow = true;
        let out = drag.out;
        if !out.valid {
            return None;
        }
        let command = match drag.what {
            What::Move { objects, .. } => Command {
                name: if i.ctrl { MODIFY_COPY } else { MODIFY_MOVE },
                params: json!({"objects": ids_of(&objects), "displacement": out.delta}),
            },
            What::End { wall, end } => Command {
                name: WALL_MOVE_END,
                params: json!({
                    "wall": wall.to_string(),
                    "end": if end == 0 { "start" } else { "end" },
                    "to": out.to,
                }),
            },
            What::Top { wall } => Command {
                name: PROPERTY_SET,
                params: json!({
                    "objects": [wall.to_string()], "field": "height", "value": out.height
                }),
            },
            What::Rotate { objects, center } => Command {
                name: MODIFY_ROTATE,
                params: json!({
                    "objects": ids_of(&objects), "base": center, "angle": out.angle
                }),
            },
            What::Click(_) => return None,
        };
        Some(Done {
            commands: vec![command],
        })
    }

    /// An edit of the dimension input. A line committed with Enter gives the move that makes the
    /// dimension the typed value.
    pub fn edited(&mut self, event: &DynamicEvent) -> Option<Done> {
        let edit = self.edit.as_mut()?;
        match event {
            DynamicEvent::Edited { text, .. } => {
                edit.text.clone_from(text);
                None
            }
            DynamicEvent::Enter { .. } => {
                let Some(value) = parse_length(&edit.text, edit.unit).filter(|v| *v >= 0.0) else {
                    // The input stays open; the controller says the value is not a length.
                    edit.invalid = true;
                    return None;
                };
                let edit = self.edit.take()?;
                let delta = value - edit.dim.value();
                if delta.abs() < 1.0e-9 {
                    return None;
                }
                let dir = edit.dim.direction();
                Some(Done {
                    commands: vec![Command {
                        name: MODIFY_MOVE,
                        params: json!({
                            "objects": [edit.dim.target.to_string()],
                            "displacement": [dir[0] * delta, dir[1] * delta],
                        }),
                    }],
                })
            }
            DynamicEvent::Cancel { .. } => {
                self.edit = None;
                None
            }
            DynamicEvent::Tab { .. } => None,
        }
    }

    /// Whether the last entry of the dimension input was not a length; taken once.
    pub fn take_invalid(&mut self) -> bool {
        self.edit
            .as_mut()
            .is_some_and(|e| std::mem::take(&mut e.invalid))
    }

    /// The input panel of a dimension being edited: its position and its field.
    #[must_use]
    pub fn input(&self) -> Option<(V2, DynamicField)> {
        self.edit.as_ref().map(|e| {
            (
                e.at,
                DynamicField {
                    label: EDIT_LABEL.message_id.to_owned(),
                    text: e.text.clone(),
                    locked: true,
                    focused: true,
                    select_all: true,
                    escape_cancels: true,
                },
            )
        })
    }

    /// The overlays: the preview of a drag, else the handles and the temporary dimensions of the
    /// selection.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "dimensions, then handles, as one list"
    )]
    pub fn overlays(
        &self,
        snapshot: &Snapshot,
        selection: &[ObjectId],
        frame: &Frame,
        unit: LengthUnit,
        separator: char,
    ) -> Vec<ProductOverlay> {
        let mut out = Vec::new();
        if let Some(d) = self.drag.as_ref().filter(|d| d.live) {
            out.extend(d.out.ghost.iter().cloned());
            for [a, b] in &d.out.guides {
                out.push(
                    ProductOverlay::Line {
                        from: p3(*a, d.z),
                        to: p3(*b, d.z),
                    }
                    .colored(OverlayColor::Muted),
                );
            }
            if let Some((kind, at)) = d.out.snap {
                out.push(ProductOverlay::Snap {
                    kind,
                    at: p3(at, d.z),
                });
            }
            if let Some((at, text)) = &d.out.label {
                out.push(ProductOverlay::Label {
                    at: *at,
                    text: text.clone(),
                });
            }
            return out;
        }
        for dim in dims(snapshot, selection) {
            let (a, b) = (dim.a(), dim.b());
            let z = dim.z;
            let dir = dim.direction();
            let side = [-dir[1], dir[0]];
            let tick = 6.0 * frame.px(dim.anchor());
            out.push(
                ProductOverlay::Path {
                    points: vec![p3(a, z), p3(b, z)],
                    dashed: false,
                }
                .colored(OverlayColor::Muted),
            );
            for end in [a, b] {
                out.push(
                    ProductOverlay::Line {
                        from: p3([end[0] + side[0] * tick, end[1] + side[1] * tick], z),
                        to: p3([end[0] - side[0] * tick, end[1] - side[1] * tick], z),
                    }
                    .colored(OverlayColor::Muted),
                );
            }
            for (from, to) in [(dim.fixed, a), (dim.moving, b)] {
                if dist2(from, to) > 1.0e-9 {
                    out.push(
                        ProductOverlay::Line {
                            from: p3(from, z),
                            to: p3(to, z),
                        }
                        .colored(OverlayColor::Muted),
                    );
                }
            }
            out.push(
                ProductOverlay::Label {
                    at: Point3::new(dim.anchor()[0], dim.anchor()[1], z),
                    text: length_text(dim.value(), unit, separator),
                }
                .colored(OverlayColor::Accent),
            );
        }
        for h in handles(snapshot, selection, frame) {
            let hovered = self.hover == Some(Hit::Handle(h.kind));
            let color = if hovered {
                OverlayColor::Warning
            } else {
                OverlayColor::Accent
            };
            let ring = shape(frame, &h);
            if let HandleKind::Rotate = h.kind
                && let Some((_, center, z)) = rotation_of(snapshot, selection)
            {
                out.push(
                    ProductOverlay::Line {
                        from: p3(center, z),
                        to: Point3::new(h.at[0], h.at[1], h.at[2]),
                    }
                    .colored(OverlayColor::Muted),
                );
            }
            out.push(
                ProductOverlay::Fill {
                    rings: vec![ring.clone()],
                }
                .colored(color),
            );
            let mut closed = ring.clone();
            closed.extend(ring.first().copied());
            out.push(
                ProductOverlay::Path {
                    points: closed,
                    dashed: false,
                }
                .colored(color),
            );
        }
        out
    }
}

/// The outline of a handle on the screen.
fn shape(frame: &Frame, h: &Handle) -> Vec<Point3> {
    let r = HANDLE_HALF;
    let offsets: Vec<[f64; 2]> = match h.kind {
        HandleKind::End { .. } => vec![[-r, -r], [r, -r], [r, r], [-r, r]],
        HandleKind::Top { .. } => vec![
            [0.0, -r - 2.0],
            [r + 2.0, 0.0],
            [0.0, r + 2.0],
            [-r - 2.0, 0.0],
        ],
        HandleKind::Rotate => (0..10)
            .map(|k| {
                let a = f64::from(k) * PI / 5.0;
                [(r + 1.0) * a.cos(), (r + 1.0) * a.sin()]
            })
            .collect(),
        HandleKind::FlipHand(_) => vec![[-r, -r], [r + 1.0, 0.0], [-r, r]],
        HandleKind::FlipFacing(_) => vec![[-r, -r], [r, -r], [0.0, r + 1.0]],
    };
    offsets
        .into_iter()
        .map(|[x, y]| frame.shifted(h.at, x, y))
        .collect()
}
