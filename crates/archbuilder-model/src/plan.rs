//! The drawing of a plan view: the model's objects of one storey as the [`ViewSource`] of
//! `arc-view`, and the cached result.
//!
//! Walls give one part per layer (from the cleaned layer solids of [`crate::cleanup`]) whose join
//! key is the layer's material, so layers of one material join across walls and a seam shows only
//! where materials differ; floors give one part per layer of their type; roofs, ceilings and
//! stairs one body each; doors and windows their frame and leaf; every object adds its symbols
//! (swing arcs, glass lines, the walking line and arrow of a stair, outlines and crosses of
//! shafts and openings, the outline of a room). The result is built in two passes that are cached
//! separately: the geometry (walls to shafts) and the notes (rooms), so renaming a room builds
//! only the notes again.
//!
//! The text of a room tag or a stair label is not a symbol (a symbol is a curve); the application
//! draws it.
//!
//! Beside the storey's own elements a view draws the walls and roofs of the storeys below that
//! rise past its level (a double-height wall, a roof over two storeys). An element that the cut
//! crosses draws none of its own parts above the cut: the parts of a wall above a door head are
//! not repeated as overhead lines. A temporary display state of the application ([`Filter`])
//! hides elements through object overrides; it is part of the cache key, never of the document.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use arc_base::{Cancel, Color, Limits, ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot};
use arc_geom::{Curve2, Polyline2, PolylineVertex, Region, prism};
use arc_view::{
    ClassStyles, CutFill, DetailLevel, Geometry, GraphicOverride, LineStyle, Part, PlanRange,
    SourceObject, Symbol, ViewDef, ViewResult, ViewSource, VisibilityClass, build_view,
};

use crate::cleanup::{Deps, Store, cached};
use crate::element_type::{Function, Layers, TypeKind, type_of};
use crate::insert::{Insert, Kind, Role};
use crate::types::{
    CEILING, DOOR, ELEVATOR, FLOOR, LEVEL, MODEL_VIEW, RAILING, RAMP, ROOF, ROOM, SHAFT, STAIR,
    STOREY, VERTICAL_OPENING, WALL, WALL_OPENING, WINDOW,
};
use crate::view::{Category, Detail, PlanView};
use crate::wall::Wall;
use crate::{cleanup, spatial};

/// The sub-key of a symbol that stands for the area of its object: a click inside it picks the
/// object. The loops of an area are `AREA_KEY + k`.
pub const AREA_KEY: u32 = 0xFFFF_0000;
/// The sub-key of the first tread line of a stair arriving from below.
pub const TREAD_KEY: u32 = 1100;
const CHORD: f64 = 0.01;
/// The drawing category of the markers of sections and elevations, which no view hides.
pub const MARKER_CATEGORY: &str = "view_marker";
const MAX_OBJECTS: usize = 200_000;

/// A temporary display state laid over a view's own settings (the application's hide and
/// isolate): the elements it hides and, when set, the only elements it shows. It is not part of
/// the document and is applied as object overrides of the drawing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Filter {
    /// The elements not drawn.
    pub hidden: BTreeSet<ObjectId>,
    /// When set, the only elements drawn.
    pub only: Option<BTreeSet<ObjectId>>,
}

impl Filter {
    /// Whether the filter hides nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hidden.is_empty() && self.only.is_none()
    }

    /// Whether the filter shows `id`.
    #[must_use]
    pub fn shows(&self, id: ObjectId) -> bool {
        !self.hidden.contains(&id) && self.only.as_ref().is_none_or(|o| o.contains(&id))
    }

    /// The key a drawing with this filter is cached under: 0 for no filter.
    fn key(&self) -> u64 {
        if self.is_empty() {
            return 0;
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        h.finish() | 1
    }
}

/// What a view is drawn from: the two passes of a storey's drawing.
#[derive(Debug)]
pub struct Built {
    /// The storey.
    pub storey: ObjectId,
    /// The elevation of the storey's level.
    pub level: f64,
    /// The range: bottom, cut and top (absolute elevations).
    pub range: (f64, f64, Option<f64>),
    /// The scale denominator.
    pub scale: f64,
    /// The walls, floors, roofs, ceilings, stairs, doors, windows, shafts and openings.
    pub geometry: Arc<ViewResult>,
    /// The rooms.
    pub notes: Arc<ViewResult>,
}

impl Built {
    /// Both passes, geometry first (so the notes draw over it).
    #[must_use]
    pub fn results(&self) -> [&ViewResult; 2] {
        [&self.geometry, &self.notes]
    }

    /// The diagnostics of both passes: a failed build's reason, and the elements a pass left out
    /// (a section that does not close, a part too small to draw).
    #[must_use]
    pub fn diagnostics(&self) -> Vec<&arc_view::Diagnostic> {
        self.results()
            .iter()
            .flat_map(|r| r.diagnostics.iter())
            .collect()
    }

    /// The links and primitives of `source` in both passes, as `(pass, index)`.
    #[must_use]
    pub fn primitives_of(&self, source: ObjectId) -> Vec<(usize, usize)> {
        self.results()
            .iter()
            .enumerate()
            .flat_map(|(pass, r)| {
                r.primitives_of(source)
                    .into_iter()
                    .map(move |index| (pass, index))
            })
            .collect()
    }
}

// ----- styles -----------------------------------------------------------------------------------

fn gray(v: f32) -> Color {
    Color::new(v, v, v, 1.0)
}

/// The sheet weight in millimeters of a line of `base` millimeters at 1:100, for the scale
/// denominator `scale`: finer scales draw heavier lines, coarser ones lighter.
#[must_use]
pub fn weight(base: f64, scale: f64) -> f64 {
    base * (100.0 / scale.max(1.0)).powf(0.3).clamp(0.6, 1.6)
}

fn styles(scale: f64, tone: f32) -> ClassStyles {
    let line = |v: f32, w: f64| LineStyle::solid(gray(v)).with_lineweight(Some(weight(w, scale)));
    ClassStyles::default()
        .with(VisibilityClass::Cut, Some(line(0.95 * tone, 0.5)))
        .with(VisibilityClass::Beyond, Some(line(0.55 * tone, 0.18)))
        .with(VisibilityClass::Overhead, Some(line(0.6 * tone, 0.18)))
        .with(VisibilityClass::Visible, Some(line(0.8 * tone, 0.25)))
}

fn fill_of(function: Function, core: bool) -> CutFill {
    let v = if core {
        0.36
    } else {
        match function {
            Function::Finish => 0.64,
            Function::Thermal => 0.5,
            _ => 0.56,
        }
    };
    CutFill::solid(Color::new(v, v * 0.96, v * 0.9, 1.0))
}

fn join_key(material: &str) -> u64 {
    // FNV-1a: only equality matters.
    material
        .trim()
        .to_lowercase()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
}

fn curve(points: &[[f64; 2]], closed: bool) -> Option<Curve2> {
    let mut vertices: Vec<PolylineVertex> = Vec::with_capacity(points.len());
    for p in points {
        if vertices
            .last()
            .is_some_and(|v| (v.point.x - p[0]).hypot(v.point.y - p[1]) < 1.0e-9)
        {
            continue;
        }
        vertices.push(PolylineVertex::new(
            arc_base::math::Point2::new(p[0], p[1]),
            0.0,
        ));
    }
    if closed
        && vertices.len() > 1
        && let (Some(a), Some(b)) = (vertices.first(), vertices.last())
        && (a.point.x - b.point.x).hypot(a.point.y - b.point.y) < 1.0e-9
    {
        vertices.pop();
    }
    Polyline2::new(vertices, closed).ok().map(Curve2::Polyline)
}

// ----- the source -------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Geometry,
    Notes,
}

struct PlanSource<'a> {
    snapshot: &'a Snapshot,
    storey: ObjectId,
    level: f64,
    level_id: Option<ObjectId>,
    /// The lowest elevation drawn (the view depth).
    bottom: f64,
    cut: f64,
    /// The top of the overhead range.
    top: Option<f64>,
    scale: f64,
    tone: f32,
    pass: Pass,
    /// The rooms of the storey, derived once for the whole pass.
    rooms: std::cell::OnceCell<Vec<crate::room::Entry>>,
}

impl PlanSource<'_> {
    fn ids(&self) -> Vec<ObjectId> {
        let s = self.snapshot;
        let mut out: Vec<ObjectId> = Vec::new();
        if self.pass == Pass::Notes {
            out.extend(spatial::children(s, ROOM, self.storey).iter().map(|o| o.id));
            return out;
        }
        let own = |key| spatial::children(s, key, self.storey);
        let walls = own(WALL);
        out.extend(walls.iter().map(|w| w.id));
        for w in &walls {
            for key in [DOOR, WINDOW, WALL_OPENING] {
                out.extend(spatial::children(s, key, w.id).iter().map(|o| o.id));
            }
        }
        let hosts: Vec<_> = [FLOOR, ROOF, CEILING].into_iter().flat_map(own).collect();
        out.extend(hosts.iter().map(|o| o.id));
        for h in &hosts {
            out.extend(
                spatial::children(s, VERTICAL_OPENING, h.id)
                    .iter()
                    .map(|o| o.id),
            );
        }
        out.extend(own(STAIR).iter().map(|o| o.id));
        out.extend(own(RAMP).iter().map(|o| o.id));
        out.extend(own(RAILING).iter().map(|o| o.id));
        if let Some(level) = self.level_id {
            out.extend(
                spatial::objects(s, STAIR)
                    .iter()
                    .filter(|o| o.header.owner != Some(self.storey))
                    .filter(|o| {
                        crate::stair::Placed::of(s, o).is_some_and(|p| p.top_level == Some(level))
                    })
                    .map(|o| o.id),
            );
        }
        out.extend(self.rising_from_below());
        // The markers of the building's sections and elevations.
        if let Some(building) = spatial::get(s, self.storey).and_then(|o| o.header.owner) {
            out.extend(
                crate::model_view::markers(s, building)
                    .iter()
                    .filter(|v| {
                        crate::model_view::ModelView::from_values(&v.values)
                            .is_some_and(|m| m.is_drawing())
                    })
                    .map(|o| o.id),
            );
        }
        // Shafts of every storey whose span reaches this level.
        out.extend(
            spatial::objects(s, SHAFT)
                .iter()
                .filter(|o| {
                    o.header.owner == Some(self.storey)
                        || crate::opening::shaft_span(s, o)
                            .is_some_and(|sp| crate::opening::passes_level(sp.z, self.level))
                })
                .map(|o| o.id),
        );
        // Elevators of every storey whose shaft reaches this level.
        out.extend(
            spatial::objects(s, ELEVATOR)
                .iter()
                .filter(|o| {
                    o.header.owner == Some(self.storey)
                        || crate::elevator::span(s, o)
                            .is_some_and(|sp| crate::opening::passes_level(sp.z, self.level))
                })
                .map(|o| o.id),
        );
        out.truncate(MAX_OBJECTS);
        out
    }

    /// The walls (with their doors, windows and openings) and roofs of the storeys below that
    /// rise past this storey's level; the drawing classifies them against the range.
    fn rising_from_below(&self) -> Vec<ObjectId> {
        let s = self.snapshot;
        let Some(building) = spatial::get(s, self.storey).and_then(|o| o.header.owner) else {
            return Vec::new();
        };
        let reach = self.level + crate::types::HEIGHT_RANGE.0;
        let mut out = Vec::new();
        for below in spatial::storeys(s, building)
            .iter()
            .filter(|b| b.elevation() < self.level - crate::types::HEIGHT_RANGE.0)
        {
            for wall in spatial::children(s, WALL, below.storey.id) {
                // A wall attached to a roof follows it up; the drawing decides what it shows.
                let rises = crate::wall::attachment(&wall.values).is_some()
                    || Wall::of(s, &wall)
                        .is_some_and(|w| below.elevation() + w.base_offset + w.height > reach);
                if rises {
                    out.push(wall.id);
                    for key in [DOOR, WINDOW, WALL_OPENING] {
                        out.extend(spatial::children(s, key, wall.id).iter().map(|o| o.id));
                    }
                }
            }
            for roof in spatial::children(s, ROOF, below.storey.id) {
                if crate::roof::Derived::of(s, &roof).is_ok_and(|d| d.surface.top() > reach) {
                    out.push(roof.id);
                }
            }
        }
        out
    }

    /// A curtain wall: its mullions and panels as parts (cut, they show as mullion sections and
    /// thin panel strips), the baseline as a thin line and the swing of door panels.
    fn curtain_object(&self, curtain: &crate::curtain::Derived) -> Option<SourceObject> {
        use crate::curtain::Class;
        let mut out = SourceObject::new(Category::Wall.key(), styles(self.scale, self.tone));
        for (k, piece) in curtain.solids().ok()?.into_iter().enumerate() {
            let v = match piece.class {
                Class::Mullion | Class::Frame | Class::Handle => 0.35,
                Class::Glass => 0.85,
                Class::Solid | Class::Leaf => 0.6,
            };
            out = out.with_part(
                Part::new(u32::try_from(k).unwrap_or(0), Geometry::Solid(piece.solid))
                    .with_fill(Some(CutFill::solid(gray(v)))),
            );
        }
        let w = &curtain.wall;
        let mid = {
            let (low, high) = w.extent();
            (low + high) / 2.0
        };
        let l = w.length().max(1.0e-9);
        let n = [-(w.end[1] - w.start[1]) / l, (w.end[0] - w.start[0]) / l];
        let line = [
            [w.start[0] + n[0] * mid, w.start[1] + n[1] * mid],
            [w.end[0] + n[0] * mid, w.end[1] + n[1] * mid],
        ];
        if let Some(c) = curve(&line, false) {
            out = out.with_symbol(Symbol::new(c, VisibilityClass::Visible, 90));
        }
        for (k, (_, item)) in curtain.leaves().into_iter().enumerate() {
            for (j, poly) in crate::insert::symbol(w, curtain.level, &item)
                .unwrap_or_default()
                .iter()
                .enumerate()
            {
                let points: Vec<[f64; 2]> = poly.iter().map(|p| [p[0], p[1]]).collect();
                if let Some(c) = curve(&points, false) {
                    let key = 200 + u32::try_from(k * 8 + j).unwrap_or(0);
                    out = out.with_symbol(Symbol::new(c, VisibilityClass::Visible, key));
                }
            }
        }
        // The outline of the wall, so that a click between the mullions picks it.
        let fp = w.plain_footprint();
        if let Some(c) = curve(&fp, true) {
            out = out.with_symbol(Symbol::new(c, VisibilityClass::Visible, AREA_KEY));
        }
        Some(out)
    }

    fn wall_object(&self, object: &Object, detail: Detail) -> Option<SourceObject> {
        if let Some(curtain) = crate::curtain::Derived::of(self.snapshot, object) {
            return self.curtain_object(&curtain);
        }
        let pieces = cleanup::wall_pieces(self.snapshot, object)?.ok()?;
        let stacks: Vec<Vec<String>> = (0..crate::cleanup::stack_ranges_of(self.snapshot, object)
            .len()
            .max(1))
            .map(|k| cleanup::layer_materials_in(self.snapshot, object, k))
            .collect();
        let mut out = SourceObject::new(Category::Wall.key(), styles(self.scale, self.tone));
        for piece in pieces {
            let material = stacks
                .get(piece.segment)
                .and_then(|m| m.get(piece.layer))
                .map_or("", String::as_str);
            let core = piece.class == cleanup::PieceClass::Core;
            let function = match piece.class {
                cleanup::PieceClass::Finish => Function::Finish,
                _ => Function::Structure,
            };
            let (fill, join) = if detail == Detail::Coarse {
                (fill_of(Function::Structure, true), join_key("wall"))
            } else {
                (fill_of(function, core), join_key(material))
            };
            out = out.with_part(
                Part::new(piece.key, Geometry::Solid(piece.solid))
                    .with_fill(Some(fill))
                    .with_join(Some(join)),
            );
        }
        Some(out)
    }

    fn floor_object(&self, object: &Object) -> Option<SourceObject> {
        let clean = cleanup::clean_floor(self.snapshot, object)?;
        let (z0, z1) = clean.z;
        let layers = type_of(self.snapshot, TypeKind::Floor, &object.values)
            .and_then(|t| Layers::from_values(&t.values))
            .filter(|l| (l.total() - (z1 - z0)).abs() <= 1.0e-6 && !l.layers.is_empty());
        let mut out = SourceObject::new(Category::Floor.key(), styles(self.scale, self.tone));
        let tol = Tolerance::DEFAULT;
        let make = |region: &Region, from: f64, h: f64| {
            prism(region, from, h, CHORD, &tol, &Limits::DEFAULT).ok()
        };
        match layers {
            Some(layers) => {
                // Layers run from the top of the slab down.
                let mut top = z1;
                for (k, layer) in layers.layers.iter().enumerate() {
                    if layer.thickness <= 1.0e-9 {
                        continue;
                    }
                    let core = k >= layers.core.0 && k < layers.core.1;
                    if let Some(solid) = make(&clean.region, top - layer.thickness, layer.thickness)
                    {
                        out = out.with_part(
                            Part::new(u32::try_from(k).unwrap_or(0), Geometry::Solid(solid))
                                .with_fill(Some(fill_of(layer.function, core)))
                                .with_join(Some(join_key(&layer.material))),
                        );
                    }
                    top -= layer.thickness;
                }
            }
            None => {
                let solid = make(&clean.region, z0, z1 - z0)?;
                out = out.with_part(
                    Part::new(0, Geometry::Solid(solid))
                        .with_fill(Some(fill_of(Function::Structure, true)))
                        .with_join(Some(join_key("floor"))),
                );
            }
        }
        Some(out)
    }

    /// A flat ceiling gives a part per layer of its type, like a floor; a sloped one, or one whose
    /// type's layers do not add up to its thickness, one body.
    fn ceiling_object(&self, object: &Arc<Object>) -> Option<SourceObject> {
        let derived = crate::ceiling::derived(self.snapshot, object)?;
        let thickness = derived.ceiling.thickness();
        let layers = type_of(self.snapshot, TypeKind::Ceiling, &object.values)
            .and_then(|t| Layers::from_values(&t.values))
            .filter(|l| (l.total() - thickness).abs() <= 1.0e-6 && !l.layers.is_empty())
            .filter(|_| !derived.ceiling.sloped());
        let Some(layers) = layers else {
            return derived
                .solid()
                .ok()
                .map(|s| self.body(Category::Ceiling, s, 0.85));
        };
        let mut out = SourceObject::new(Category::Ceiling.key(), styles(self.scale, self.tone));
        // Layers run from the top down, as a floor's.
        let mut top = derived.plane.z + thickness;
        for (k, layer) in layers.layers.iter().enumerate() {
            if layer.thickness <= 1.0e-9 {
                continue;
            }
            let core = k >= layers.core.0 && k < layers.core.1;
            if let Ok(solid) = derived.boundary.solid(top, layer.thickness) {
                out = out.with_part(
                    Part::new(u32::try_from(k).unwrap_or(0), Geometry::Solid(solid))
                        .with_fill(Some(fill_of(layer.function, core)))
                        .with_join(Some(join_key(&layer.material))),
                );
            }
            top -= layer.thickness;
        }
        Some(out)
    }

    fn body(&self, category: Category, solid: arc_geom::Polyhedron, v: f32) -> SourceObject {
        SourceObject::new(category.key(), styles(self.scale, self.tone)).with_part(
            Part::new(0, Geometry::Solid(solid)).with_fill(Some(CutFill::solid(Color::new(
                v,
                v * 0.9,
                v * 0.85,
                1.0,
            )))),
        )
    }

    fn insert_object(&self, object: &Object, detail: Detail) -> Option<SourceObject> {
        let host = spatial::get(self.snapshot, object.header.owner?)?;
        let wall = Wall::of(self.snapshot, &host)?;
        let item = Insert::of(self.snapshot, object)?;
        // An arc wall's opening is drawn on its tangent frame.
        let (wall, item) = crate::insert::on_arc(&wall, &item);
        let level = host
            .header
            .owner
            .and_then(|s| spatial::get(self.snapshot, s))
            .map_or(0.0, |s| spatial::storey_elevation(self.snapshot, &s));
        let category = match item.kind {
            Kind::Door => Category::Door,
            Kind::Window => Category::Window,
            Kind::Opening => Category::Opening,
        };
        let mut out = SourceObject::new(category.key(), styles(self.scale, self.tone));
        if detail == Detail::Fine
            && let Ok(pieces) = crate::insert::pieces(&wall, level, &item)
        {
            for (k, (role, solid)) in pieces.into_iter().enumerate() {
                let v = match role {
                    Role::Frame | Role::Handle => 0.5,
                    Role::Panel => 0.7,
                    Role::Glass => 0.8,
                };
                out = out.with_part(
                    Part::new(u32::try_from(k).unwrap_or(0), Geometry::Solid(solid))
                        .with_fill(Some(CutFill::solid(gray(v)))),
                );
            }
        }
        let lines = crate::insert::symbol_at(&wall, level, &item, detail).unwrap_or_default();
        for (k, line) in lines.iter().enumerate() {
            let points: Vec<[f64; 2]> = line.iter().map(|p| [p[0], p[1]]).collect();
            if let Some(c) = curve(&points, false) {
                out = out.with_symbol(Symbol::new(
                    c,
                    VisibilityClass::Visible,
                    100 + u32::try_from(k).unwrap_or(0),
                ));
            }
        }
        // The opening's footprint, so that a click between the jambs picks the insert.
        let (x0, x1) = item.extent();
        let at = |x: f64, y: f64| {
            let d = [wall.end[0] - wall.start[0], wall.end[1] - wall.start[1]];
            let len = d[0].hypot(d[1]).max(1.0e-9);
            let (t, n) = ([d[0] / len, d[1] / len], [-d[1] / len, d[0] / len]);
            [
                wall.start[0] + t[0] * x + n[0] * y,
                wall.start[1] + t[1] * x + n[1] * y,
            ]
        };
        let h = wall.thickness / 2.0;
        if let Some(c) = curve(&[at(x0, -h), at(x1, -h), at(x1, h), at(x0, h)], true) {
            out = out.with_symbol(Symbol::new(c, VisibilityClass::Visible, AREA_KEY));
        }
        Some(out)
    }

    fn stair_object(&self, object: &Object) -> Option<SourceObject> {
        let placed = crate::stair::Placed::of(self.snapshot, object)?;
        let layout = placed.layout.ok()?;
        let mut out = SourceObject::new(Category::Stair.key(), styles(self.scale, self.tone));
        let arrives = object.header.owner != Some(self.storey);
        // A stair arriving from below lies under this view's cut: its treads are drawn as lines
        // beyond the cut (its solids would mostly lie below the view depth).
        if !arrives {
            for (k, solid) in layout.solids(placed.base).ok()?.into_iter().enumerate() {
                out = out.with_part(
                    Part::new(u32::try_from(k).unwrap_or(0), Geometry::Solid(solid))
                        .with_fill(Some(CutFill::solid(gray(0.55)))),
                );
            }
        }
        let rel = self.cut - placed.base;
        let plan = if arrives {
            layout.plan_above(rel)
        } else {
            layout.plan(rel)
        };
        let mut add = |points: &[[f64; 2]], closed: bool, key: u32, class: VisibilityClass| {
            if let Some(c) = curve(points, closed) {
                out = std::mem::replace(&mut out, SourceObject::new("", ClassStyles::default()))
                    .with_symbol(Symbol::new(c, class, key));
            }
        };
        add(&plan.walk, false, 1001, VisibilityClass::Visible);
        add(&plan.arrow, false, 1002, VisibilityClass::Visible);
        if arrives {
            for (k, (pair, _)) in plan.treads.iter().enumerate() {
                add(
                    pair,
                    false,
                    TREAD_KEY + u32::try_from(k).unwrap_or(0),
                    VisibilityClass::Beyond,
                );
            }
        }
        if let Some(cut) = &plan.cut {
            add(cut, false, 1003, VisibilityClass::Cut);
        }
        for (k, loop_) in plan.outline.iter().enumerate() {
            let key = if k == 0 {
                AREA_KEY
            } else {
                1010 + u32::try_from(k).unwrap_or(0)
            };
            add(loop_, true, key, VisibilityClass::Visible);
        }
        Some(out)
    }

    fn ramp_object(&self, object: &Object) -> Option<SourceObject> {
        let placed = crate::ramp::Placed::of(self.snapshot, object)?;
        let layout = placed.layout.ok()?;
        let mut out = SourceObject::new(Category::Ramp.key(), styles(self.scale, self.tone));
        out = out.with_part(
            Part::new(0, Geometry::Solid(layout.solid(placed.base).ok()?))
                .with_fill(Some(CutFill::solid(gray(0.55)))),
        );
        let plan = layout.plan();
        let mut add = |points: &[[f64; 2]], closed: bool, key: u32| {
            if let Some(c) = curve(points, closed) {
                out = std::mem::replace(&mut out, SourceObject::new("", ClassStyles::default()))
                    .with_symbol(Symbol::new(c, VisibilityClass::Visible, key));
            }
        };
        add(&plan.walk, false, 1001);
        add(&plan.arrow, false, 1002);
        for (k, tick) in plan.ticks.iter().enumerate() {
            add(tick, false, 1020 + u32::try_from(k).unwrap_or(0));
        }
        add(&plan.outline, true, AREA_KEY);
        Some(out)
    }

    /// The symbol of a railing: its rail line, with the extensions.
    fn railing_object(&self, object: &Object) -> Option<SourceObject> {
        let derived = crate::railing::Derived::of(self.snapshot, object).ok()?;
        let mut out = SourceObject::new(Category::Railing.key(), styles(self.scale, self.tone));
        if let Some(c) = curve(&derived.plan_line(), false) {
            out = out.with_symbol(Symbol::new(c, VisibilityClass::Visible, 1001));
        }
        Some(out)
    }

    fn opening_object(&self, object: &Object, key: &str) -> Option<SourceObject> {
        let mut out = SourceObject::new(Category::Opening.key(), styles(self.scale, self.tone));
        let (outline, _own, class) = if key == SHAFT {
            let span = crate::opening::shaft_span(self.snapshot, object)?;
            let own = object.header.owner == Some(self.storey);
            (
                span.outline,
                own,
                if own {
                    VisibilityClass::Visible
                } else {
                    VisibilityClass::Overhead
                },
            )
        } else {
            let host = spatial::get(self.snapshot, object.header.owner?)?;
            let _ = host;
            (
                crate::opening::outline_of(self.snapshot, object, crate::floor::VERTICAL_SLOTS)?,
                false,
                VisibilityClass::Overhead,
            )
        };
        if let Some(c) = curve(&outline, true) {
            out = out.with_symbol(Symbol::new(c, class, AREA_KEY));
        }
        // A cross corner to corner on the storeys a shaft cuts.
        if key == SHAFT && outline.len() >= 4 {
            let (lo, hi) = outline
                .iter()
                .fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| {
                    (
                        [lo[0].min(p[0]), lo[1].min(p[1])],
                        [hi[0].max(p[0]), hi[1].max(p[1])],
                    )
                });
            for (k, (a, b)) in [
                ([lo[0], lo[1]], [hi[0], hi[1]]),
                ([lo[0], hi[1]], [hi[0], lo[1]]),
            ]
            .into_iter()
            .enumerate()
            {
                if let Some(c) = curve(&[a, b], false) {
                    out =
                        out.with_symbol(Symbol::new(c, class, 10 + u32::try_from(k).unwrap_or(0)));
                }
            }
        }
        Some(out)
    }

    /// The plan symbol of an elevator: the shaft, the car with a cross corner to corner, and a
    /// mark across the landing door on every storey it stops at.
    fn elevator_object(&self, object: &Object) -> Option<SourceObject> {
        let d = crate::elevator::derived(self.snapshot, object)?;
        let own = object.header.owner == Some(self.storey);
        let class = if own {
            VisibilityClass::Visible
        } else {
            VisibilityClass::Overhead
        };
        let mut out = SourceObject::new(Category::Opening.key(), styles(self.scale, self.tone));
        let shaft = d.elevator.shaft_outline();
        if let Some(c) = curve(&shaft, true) {
            out = out.with_symbol(Symbol::new(c, class, AREA_KEY));
        }
        let car = d.elevator.car_outline();
        if let Some(c) = curve(&car, true) {
            out = out.with_symbol(Symbol::new(c, class, 20));
        }
        let mut lines = vec![[car[0], car[2]], [car[1], car[3]]];
        lines.push(d.elevator.door_line());
        for (k, l) in lines.into_iter().enumerate() {
            if let Some(c) = curve(&l, false) {
                out = out.with_symbol(Symbol::new(c, class, 30 + u32::try_from(k).unwrap_or(0)));
            }
        }
        Some(out)
    }

    fn room_object(&self, id: ObjectId) -> Option<SourceObject> {
        let entry = self
            .rooms
            .get_or_init(|| crate::room::of_storey(self.snapshot, self.storey))
            .iter()
            .find(|e| e.id == id)?;
        let mut out = SourceObject::new(
            Category::Room.key(),
            ClassStyles::default().with(
                VisibilityClass::Visible,
                Some(
                    LineStyle::solid(gray(0.45 * self.tone))
                        .with_lineweight(Some(weight(0.1, self.scale))),
                ),
            ),
        );
        for (k, l) in entry.footprint.loops().iter().enumerate() {
            if let Some(c) = curve(l, true) {
                out = out.with_symbol(Symbol::new(
                    c,
                    VisibilityClass::Visible,
                    AREA_KEY + u32::try_from(k).unwrap_or(0),
                ));
            }
        }
        Some(out)
    }
}

impl ViewSource for PlanSource<'_> {
    fn objects(&self, _: &ViewDef) -> Result<Vec<ObjectId>> {
        Ok(self.ids())
    }

    fn object(&self, id: ObjectId, def: &ViewDef) -> Result<Option<SourceObject>> {
        Ok(self
            .made(id, def)
            .map(|o| without_own_overhead(o, self.cut)))
    }
}

impl PlanSource<'_> {
    /// What `id` gives the drawing, before the parts of a cut element above the cut are dropped.
    fn made(&self, id: ObjectId, def: &ViewDef) -> Option<SourceObject> {
        let object = spatial::get(self.snapshot, id)?;
        let key = spatial::type_of(self.snapshot, id).unwrap_or_default();
        let detail = match def.detail {
            DetailLevel::Coarse => Detail::Coarse,
            DetailLevel::Medium => Detail::Medium,
            _ => Detail::Fine,
        };
        match key.as_str() {
            WALL => self.wall_object(&object, detail),
            FLOOR => self.floor_object(&object),
            ROOF => crate::roof::Derived::of(self.snapshot, &object)
                .ok()
                .and_then(|d| d.solid_cut().ok())
                .map(|(s, _)| self.body(Category::Roof, s, 0.62)),
            CEILING => self.ceiling_object(&object),
            DOOR | WINDOW | WALL_OPENING => self.insert_object(&object, detail),
            STAIR => self.stair_object(&object),
            RAMP => self.ramp_object(&object),
            RAILING => self.railing_object(&object),
            SHAFT | VERTICAL_OPENING => self.opening_object(&object, &key),
            ELEVATOR => self.elevator_object(&object),
            ROOM => self.room_object(id),
            MODEL_VIEW => self.marker_object(&object),
            _ => None,
        }
    }

    /// The marker of a section or elevation in plan: a section's line with a triangle at each end
    /// pointing the way it looks, an elevation's circle with a triangle pointing at the building.
    /// Its size is fixed on the sheet.
    fn marker_object(&self, object: &Object) -> Option<SourceObject> {
        use crate::model_view::{ModelView, ViewKind};
        let view = ModelView::from_values(&object.values)?;
        let (a, b) = view.line?;
        let look = view.looking();
        let size = 0.004 * self.scale;
        let mut out = SourceObject::new(MARKER_CATEGORY, styles(self.scale, self.tone));
        let triangle = |at: [f64; 2]| {
            let side = [look[1] * size * 0.6, -look[0] * size * 0.6];
            let tip = [at[0] + look[0] * size * 1.6, at[1] + look[1] * size * 1.6];
            vec![
                [at[0] + side[0], at[1] + side[1]],
                tip,
                [at[0] - side[0], at[1] - side[1]],
            ]
        };
        let mut symbols: Vec<(Vec<[f64; 2]>, bool)> = Vec::new();
        match view.kind {
            ViewKind::Section => {
                symbols.push((vec![a, b], false));
                symbols.push((triangle(a), true));
                symbols.push((triangle(b), true));
            }
            _ => {
                let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                let circle: Vec<[f64; 2]> = (0..24)
                    .map(|k| {
                        let t = f64::from(k) * std::f64::consts::TAU / 24.0;
                        [mid[0] + size * t.cos(), mid[1] + size * t.sin()]
                    })
                    .collect();
                symbols.push((circle, true));
                symbols.push((triangle(mid), true));
            }
        }
        for (k, (points, closed)) in symbols.into_iter().enumerate() {
            if let Some(c) = curve(&points, closed) {
                let key = if k == 0 {
                    AREA_KEY
                } else {
                    1 + u32::try_from(k).unwrap_or(0)
                };
                out = out.with_symbol(Symbol::new(c, VisibilityClass::Visible, key));
            }
        }
        Some(out)
    }
}

/// `object` without its parts above the cut when the cut crosses one of its parts: an element
/// that is cut shows its section, not overhead copies of its own upper bands (the wall above a
/// door head, the head of a door frame).
fn without_own_overhead(mut object: SourceObject, cut: f64) -> SourceObject {
    let span = |part: &Part| match &part.geometry {
        Geometry::Solid(solid) => {
            let b = solid.bounding_box();
            Some((b.min().z, b.max().z))
        }
        _ => None,
    };
    // The test of `arc-view`: a part is cut when it starts below the cut and reaches it.
    let cut_through = object
        .parts
        .iter()
        .filter_map(span)
        .any(|(lo, hi)| lo < cut && hi >= cut);
    if cut_through {
        object
            .parts
            .retain(|p| span(p).is_none_or(|(lo, _)| lo < cut));
    }
    object
}

// ----- building and caching ---------------------------------------------------------------------

static GEOMETRY: Store<ViewResult> = Mutex::new(Vec::new());
static NOTES: Store<ViewResult> = Mutex::new(Vec::new());

/// What a pass reads: every object of the kinds it draws or derives them from, and the levels and
/// storeys. The geometry reads walls, inserts, floors, roofs, ceilings, stairs, shafts, openings
/// and every element type; the notes read rooms, walls and wall types (which walls bound rooms).
/// A rename of a room, or an edit of anything else, leaves the other pass's inputs alone.
fn deps(snapshot: &Snapshot, view: &Arc<Object>, pass: Pass) -> Deps {
    let mut deps: Deps = vec![Arc::clone(view)];
    deps.extend(spatial::objects(snapshot, LEVEL));
    deps.extend(spatial::objects(snapshot, STOREY));
    let keys: &[&str] = match pass {
        // The room outlines follow the walls that bound rooms, by their types.
        Pass::Notes => &[ROOM, WALL, crate::types::WALL_TYPE],
        Pass::Geometry => &[
            WALL,
            DOOR,
            WINDOW,
            WALL_OPENING,
            FLOOR,
            ROOF,
            CEILING,
            STAIR,
            RAMP,
            RAILING,
            SHAFT,
            ELEVATOR,
            VERTICAL_OPENING,
            MODEL_VIEW,
        ],
    };
    for key in keys {
        deps.extend(spatial::objects(snapshot, key));
    }
    if pass == Pass::Geometry {
        for key in crate::types::TYPE_KEYS {
            deps.extend(spatial::objects(snapshot, key));
        }
    }
    deps
}

fn hide() -> GraphicOverride {
    let mut o = GraphicOverride::default();
    o.visible = Some(false);
    o
}

fn faint() -> GraphicOverride {
    let mut o = GraphicOverride::default();
    o.halftone = Some(true);
    o
}

fn definition(
    settings: &PlanView,
    range: (f64, f64, Option<f64>),
    halftone: bool,
    filtered: &[ObjectId],
) -> ViewDef {
    let (bottom, cut, top) = range;
    let mut def = ViewDef::plan(PlanRange::new(cut, bottom, top));
    def.scale = settings.scale;
    def.detail = match settings.detail {
        Detail::Coarse => DetailLevel::Coarse,
        Detail::Medium => DetailLevel::Medium,
        Detail::Fine => DetailLevel::Fine,
    };
    for category in Category::ALL {
        if !settings.shows(category) {
            def.overrides
                .by_category
                .insert(category.key().to_owned(), hide());
        } else if halftone {
            def.overrides
                .by_category
                .insert(category.key().to_owned(), faint());
        }
    }
    for id in settings.hidden.iter().chain(filtered) {
        def.overrides.by_object.insert(*id, hide());
    }
    def
}

/// `build_view`, with a failure as the result's one diagnostic, on the view `view`: the drawing is
/// empty and says why.
fn build_or_report(
    view: ObjectId,
    def: &ViewDef,
    source: &PlanSource<'_>,
    cancel: &Cancel,
) -> ViewResult {
    build_view(def, source, &Tolerance::DEFAULT, cancel).unwrap_or_else(|error| {
        let mut failed = ViewResult::default();
        failed.diagnostics.push(arc_view::Diagnostic {
            object: view,
            error,
        });
        failed
    })
}

fn pass_result(
    snapshot: &Snapshot,
    view: &Arc<Object>,
    (pass, source): (Pass, &PlanSource<'_>),
    (settings, filter): (&PlanView, &Filter),
    cancel: &Cancel,
) -> Option<Arc<ViewResult>> {
    let store = match pass {
        Pass::Geometry => &GEOMETRY,
        Pass::Notes => &NOTES,
    };
    let key = filter.key();
    // One filter per view is kept: the one the view is shown with.
    cleanup::keep_only(store, view.id, key);
    // The definition is the view record, which is among the dependencies, and the filter, which
    // is the key.
    let stored = cached(store, view.id, key, deps(snapshot, view, pass), || {
        let filtered: Vec<ObjectId> = if filter.is_empty() {
            Vec::new()
        } else {
            source
                .ids()
                .into_iter()
                .filter(|id| !filter.shows(*id))
                .collect()
        };
        let def = definition(
            settings,
            (source.bottom, source.cut, source.top),
            false,
            &filtered,
        );
        build_or_report(view.id, &def, source, cancel)
    });
    if cancel.is_cancelled() {
        // A cancelled build is no result: it is not kept.
        cleanup::forget(store, view.id, key);
        return None;
    }
    Some(stored)
}

/// The drawing of the plan view record `view` with the temporary `filter` laid over it: both
/// passes, rebuilt only when what they read or the filter changed. `None` when the record or its
/// storey cannot be read or the build was cancelled. A build that fails gives an empty pass whose
/// diagnostic says why ([`Built::diagnostics`]).
#[must_use]
pub fn build(
    snapshot: &Snapshot,
    view: &Arc<Object>,
    filter: &Filter,
    cancel: &Cancel,
) -> Option<Arc<Built>> {
    let settings = crate::view::settings_of(snapshot, view)?;
    let (storey, level, above) = crate::view::levels_of(snapshot, view)?;
    let range = settings.range(level, above);
    let level_id = spatial::get(snapshot, storey)
        .and_then(|s| spatial::level_of(snapshot, &s))
        .map(|l| l.id);
    let source = |pass| PlanSource {
        snapshot,
        storey,
        level,
        level_id,
        bottom: range.0,
        cut: range.1,
        top: range.2,
        scale: settings.scale,
        tone: 1.0,
        pass,
        rooms: std::cell::OnceCell::new(),
    };
    let geometry = pass_result(
        snapshot,
        view,
        (Pass::Geometry, &source(Pass::Geometry)),
        (&settings, filter),
        cancel,
    )?;
    let notes = pass_result(
        snapshot,
        view,
        (Pass::Notes, &source(Pass::Notes)),
        (&settings, filter),
        cancel,
    )?;
    Some(Arc::new(Built {
        storey,
        level,
        range,
        scale: settings.scale,
        geometry,
        notes,
    }))
}

static UNDER: Store<ViewResult> = Mutex::new(Vec::new());

/// The storey below the one `view` is of, drawn faint: its walls, floors and the rest through
/// its own default range. `None` when there is no storey below or the build was cancelled.
#[must_use]
pub fn underlay(
    snapshot: &Snapshot,
    view: &Arc<Object>,
    cancel: &Cancel,
) -> Option<Arc<ViewResult>> {
    let settings = crate::view::settings_of(snapshot, view)?;
    let (storey, level, _) = crate::view::levels_of(snapshot, view)?;
    let building = spatial::get(snapshot, storey)?.header.owner?;
    let below = spatial::storeys(snapshot, building)
        .into_iter()
        .rfind(|s| s.elevation() < level - crate::types::HEIGHT_RANGE.0)?;
    let below_level = below.elevation();
    let own = PlanView::new("underlay");
    let range = own.range(below_level, Some(level));
    let def = definition(&own, range, true, &[]);
    let source = PlanSource {
        snapshot,
        storey: below.storey.id,
        level: below_level,
        level_id: below.level.as_ref().map(|l| l.id),
        bottom: range.0,
        cut: range.1,
        top: range.2,
        scale: settings.scale,
        tone: 0.7,
        pass: Pass::Geometry,
        rooms: std::cell::OnceCell::new(),
    };
    let mut keep = deps(snapshot, view, Pass::Geometry);
    keep.extend(spatial::get(snapshot, below.storey.id));
    let stored = cached(&UNDER, view.id, 1, keep, || {
        build_or_report(view.id, &def, &source, cancel)
    });
    if cancel.is_cancelled() {
        cleanup::forget(&UNDER, view.id, 1);
        return None;
    }
    Some(stored)
}

// ----- sections and elevations ----------------------------------------------------------------

/// The storey `id` stands on: its owner, or its owner's owner (a door's wall, an opening's floor).
fn storey_of_element(snapshot: &Snapshot, id: ObjectId) -> Option<ObjectId> {
    let mut at = spatial::get(snapshot, id)?.header.owner?;
    for _ in 0..4 {
        if spatial::type_of(snapshot, at).as_deref() == Some(STOREY) {
            return Some(at);
        }
        at = spatial::get(snapshot, at)?.header.owner?;
    }
    None
}

/// The objects of a building in a vertical drawing: what the plan views of its storeys draw, but
/// with the solids only (the plan symbols are drawn in plan coordinates and have no place in a
/// vertical frame), every part of an element (none is dropped above a cut) and no rooms.
struct VerticalSource<'a> {
    snapshot: &'a Snapshot,
    building: ObjectId,
    scale: f64,
}

impl VerticalSource<'_> {
    fn storey_source(&self, storey: ObjectId) -> Option<PlanSource<'_>> {
        let object = spatial::get(self.snapshot, storey)?;
        let level = spatial::storey_elevation(self.snapshot, &object);
        Some(PlanSource {
            snapshot: self.snapshot,
            storey,
            level,
            level_id: spatial::level_of(self.snapshot, &object).map(|l| l.id),
            bottom: level - 1.0,
            cut: level + crate::view::DEFAULT_CUT,
            top: Some(level + crate::view::DEFAULT_TOP),
            scale: self.scale,
            tone: 1.0,
            pass: Pass::Geometry,
            rooms: std::cell::OnceCell::new(),
        })
    }

    fn ids(&self) -> Vec<ObjectId> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for entry in spatial::storeys(self.snapshot, self.building) {
            let Some(source) = self.storey_source(entry.storey.id) else {
                continue;
            };
            for id in source.ids() {
                if spatial::type_of(self.snapshot, id).as_deref() != Some(MODEL_VIEW)
                    && seen.insert(id)
                {
                    out.push(id);
                }
            }
        }
        out.truncate(MAX_OBJECTS);
        out
    }
}

impl ViewSource for VerticalSource<'_> {
    fn objects(&self, _: &ViewDef) -> Result<Vec<ObjectId>> {
        Ok(self.ids())
    }

    fn object(&self, id: ObjectId, def: &ViewDef) -> Result<Option<SourceObject>> {
        let Some(source) = storey_of_element(self.snapshot, id).and_then(|s| self.storey_source(s))
        else {
            return Ok(None);
        };
        Ok(source.made(id, def).and_then(|mut o| {
            o.symbols.clear();
            (!o.parts.is_empty()).then_some(o)
        }))
    }
}

static VERTICAL: Store<ViewResult> = Mutex::new(Vec::new());

/// The definition of the section or elevation `settings`: its frame, the cut at the line, the far
/// clip behind it, nothing in front, the crop to the line and the vertical range, and the
/// visibility of the view.
fn vertical_definition(
    settings: &crate::model_view::ModelView,
    filtered: &[ObjectId],
) -> Result<ViewDef> {
    let mut def = ViewDef::plan(PlanRange::new(0.0, -settings.depth, None));
    def.direction = settings.direction()?;
    def.scale = settings.scale;
    def.detail = match settings.detail {
        Detail::Coarse => DetailLevel::Coarse,
        Detail::Medium => DetailLevel::Medium,
        Detail::Fine => DetailLevel::Fine,
    };
    if settings.crop {
        // Without a vertical range the crop is the line's length only.
        let far = VERTICAL_REACH;
        def.crop = Some(arc_view::crop_rectangle(
            arc_base::math::Point2::new(0.0, settings.bottom.unwrap_or(-far)),
            arc_base::math::Point2::new(settings.width(), settings.top.unwrap_or(far)),
            &Tolerance::DEFAULT,
        )?);
    }
    for category in Category::ALL {
        if !settings.shows(category) {
            def.overrides
                .by_category
                .insert(category.key().to_owned(), hide());
        }
    }
    for id in settings.hidden.iter().chain(filtered) {
        def.overrides.by_object.insert(*id, hide());
    }
    Ok(def)
}

/// How far up and down an uncropped vertical range reaches, in meters.
const VERTICAL_REACH: f64 = 10_000.0;

/// The drawing of the section or elevation record `view` with the temporary `filter`, in view
/// coordinates (x along the line from its start, y the elevation): rebuilt only when what it reads
/// or the filter changed. `None` for a record that is not a section or elevation of a building, or
/// a cancelled build; a build that fails gives an empty drawing whose diagnostic says why.
#[must_use]
pub fn build_vertical(
    snapshot: &Snapshot,
    view: &Arc<Object>,
    filter: &Filter,
    cancel: &Cancel,
) -> Option<Arc<ViewResult>> {
    let settings = crate::model_view::ModelView::from_values(&view.values)
        .filter(crate::model_view::ModelView::is_drawing)?;
    let building = view.header.owner?;
    let source = VerticalSource {
        snapshot,
        building,
        scale: settings.scale,
    };
    let key = filter.key();
    cleanup::keep_only(&VERTICAL, view.id, key);
    let stored = cached(
        &VERTICAL,
        view.id,
        key,
        deps(snapshot, view, Pass::Geometry),
        || {
            let filtered: Vec<ObjectId> = if filter.is_empty() {
                Vec::new()
            } else {
                source
                    .ids()
                    .into_iter()
                    .filter(|id| !filter.shows(*id))
                    .collect()
            };
            match vertical_definition(&settings, &filtered) {
                Ok(def) => build_view(&def, &source, &Tolerance::DEFAULT, cancel)
                    .unwrap_or_else(|error| failed(view.id, error)),
                Err(error) => failed(view.id, error),
            }
        },
    );
    if cancel.is_cancelled() {
        cleanup::forget(&VERTICAL, view.id, key);
        return None;
    }
    Some(stored)
}

fn failed(view: ObjectId, error: arc_base::Error) -> ViewResult {
    let mut failed = ViewResult::default();
    failed.diagnostics.push(arc_view::Diagnostic {
        object: view,
        error,
    });
    failed
}

// ----- picking ----------------------------------------------------------------------------------

fn distance(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
}

/// The points of a curve, arcs flattened.
#[must_use]
pub fn flatten(c: &Curve2) -> Vec<[f64; 2]> {
    use arc_geom::CurveFlatten;
    c.flatten(CHORD, 0.2, 4096)
        .map(|v| v.iter().map(|p| [p.x, p.y]).collect())
        .unwrap_or_default()
}

fn inside(polygon: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut in_ = false;
    for (a, b) in polygon.iter().zip(polygon.iter().cycle().skip(1)) {
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            in_ = !in_;
        }
    }
    in_
}

/// What a drawing has under a point: the objects with a line or fill of their own there, and the
/// objects only a joined fill names (a joined fill is one region of several objects, so it cannot
/// say which of them lies under the point).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hits {
    /// Objects whose own primitive lies under the point.
    pub direct: Vec<ObjectId>,
    /// Objects a joined fill under the point was made from.
    pub joined: Vec<ObjectId>,
}

impl Hits {
    /// Orders `stack` (the objects under a point in picking order, from the model) by what the
    /// drawing shows: only objects the drawing has under the point stay, in the order of the
    /// stack; direct hits the stack lacks come after.
    #[must_use]
    pub fn filter(&self, stack: Vec<ObjectId>) -> Vec<ObjectId> {
        let mut out: Vec<ObjectId> = stack
            .into_iter()
            .filter(|id| self.direct.contains(id) || self.joined.contains(id))
            .collect();
        let extra: Vec<ObjectId> = self
            .direct
            .iter()
            .filter(|id| !out.contains(id))
            .copied()
            .collect();
        out.extend(extra);
        out
    }
}

/// The objects whose drawing lies under `at`: a line within `aperture` meters, a fill or an area
/// symbol containing the point.
#[must_use]
pub fn hits(built: &Built, at: [f64; 2], aperture: f64) -> Hits {
    use arc_draft::display::Shape;
    let mut out = Hits::default();
    let point = arc_base::math::Point2::new(at[0], at[1]);
    for result in built.results() {
        for (p, link) in result.list.primitives.iter().zip(&result.links) {
            let hit = match &p.shape {
                Shape::Stroke { curve, .. } => {
                    let pts = flatten(curve);
                    if link.sub_key >= AREA_KEY {
                        inside(&pts, at)
                            || pts
                                .iter()
                                .zip(pts.iter().cycle().skip(1))
                                .any(|(a, b)| distance(*a, *b, at) <= aperture)
                    } else {
                        pts.windows(2).any(|w| match (w.first(), w.get(1)) {
                            (Some(a), Some(b)) => distance(*a, *b, at) <= aperture,
                            _ => false,
                        })
                    }
                }
                Shape::Region(region) => region
                    .classify(point, &Tolerance::DEFAULT)
                    .is_ok_and(|c| c != arc_geom::Containment::Outside),
                _ => false,
            };
            if hit {
                if !out.direct.contains(&link.source) {
                    out.direct.push(link.source);
                }
                for (id, _) in &link.joined {
                    if !out.joined.contains(id) {
                        out.joined.push(*id);
                    }
                }
            }
        }
    }
    out
}

/// The drawing of `view` with `filter` when it is already built from what the snapshot holds
/// (nothing is built): what an interaction may use without waiting.
#[must_use]
pub fn peek(snapshot: &Snapshot, view: &Arc<Object>, filter: &Filter) -> Option<Arc<Built>> {
    let settings = crate::view::settings_of(snapshot, view)?;
    let (storey, level, above) = crate::view::levels_of(snapshot, view)?;
    let key = filter.key();
    let geometry = cleanup::lookup(
        &GEOMETRY,
        view.id,
        key,
        &deps(snapshot, view, Pass::Geometry),
    )?;
    let notes = cleanup::lookup(&NOTES, view.id, key, &deps(snapshot, view, Pass::Notes))?;
    Some(Arc::new(Built {
        storey,
        level,
        range: settings.range(level, above),
        scale: settings.scale,
        geometry,
        notes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_base::id::Identifier;

    #[test]
    fn a_build_that_fails_says_why_instead_of_drawing_nothing() {
        let mut registry = arc_doc::TypeRegistry::new();
        crate::register_types(&mut registry).unwrap_or_default();
        let snapshot = Snapshot::empty(Arc::new(registry));
        let view = ObjectId::from_u128(7);
        let source = PlanSource {
            snapshot: &snapshot,
            storey: ObjectId::from_u128(8),
            level: 0.0,
            level_id: None,
            bottom: -0.01,
            cut: 1.2,
            top: Some(3.0),
            scale: 100.0,
            tone: 1.0,
            pass: Pass::Geometry,
            rooms: std::cell::OnceCell::new(),
        };
        let mut def = ViewDef::plan(PlanRange::new(1.2, -0.01, Some(3.0)));
        def.scale = f64::NAN;
        let result = build_or_report(view, &def, &source, &Cancel::new());
        assert!(result.list.primitives.is_empty());
        let [diagnostic] = result.diagnostics.as_slice() else {
            panic!("{:?}", result.diagnostics);
        };
        assert_eq!(diagnostic.object, view);
        assert_eq!(diagnostic.error.code(), "view.definition.invalid");
    }
}
