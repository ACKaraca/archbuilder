//! Mesh export: the triangles of the elements of a document as OBJ with its MTL, binary STL or
//! glTF 2.0 (JSON or GLB), written by `arc-io-mesh`. Nothing is stored; every solid is derived
//! from the records the way the viewport derives it, so the file shows what the model shows.
//!
//! Each element is one object (OBJ), one node (glTF) or a part of the one solid (STL binary);
//! its name is the element's name and its identifier, and a glTF node also carries them as
//! `extras`. A mesh cannot carry parameters, types, layers, quantities or relations, nor the
//! elements without a body; the loss report names each of those.
//!
//! No function in this crate panics; a solid that cannot be derived or meshed is reported and the
//! export goes on with the rest.

use std::collections::{BTreeMap, BTreeSet};

use arc_base::math::Point3;
use arc_base::{Cancel, Error, Limits, ObjectId, Result, Tolerance};
use arc_doc::{Object, Snapshot};
use arc_geom::Polyhedron;
use arc_io_common::loss::{Argument, LossItem, LossReport, Reason};
use arc_io_mesh::{
    AlphaMode, Encoding, Material, MeshGroup, MeshPart, MeshScene, Node, Preserved, Transform,
    WriteOptions, gltf, obj, stl,
};
use arc_mesh::TriMesh;
use archbuilder_model::cleanup::{self, PieceClass};
use archbuilder_model::insert::{self, Insert, Role};
use archbuilder_model::types::{
    CEILING, DOOR, DORMER, ELEVATOR, FLOOR, FLOOR_EDGE, RAILING, RAMP, REFERENCE_PLANE, ROOF,
    ROOF_EDGE, ROOF_EXTRUSION, ROOM, SELECTION_SET, STAIR, WALL, WINDOW, WORK_PLANE,
};
use archbuilder_model::wall::Wall;
use archbuilder_model::{spatial, types};

use crate::error::reasons;

/// The format of a mesh file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshFormat {
    /// Wavefront OBJ, with an MTL material library beside it.
    Obj,
    /// Binary STL: one solid for all elements, without names or colors.
    Stl,
    /// glTF 2.0 JSON with its buffer embedded.
    Gltf,
    /// glTF 2.0 in one binary container.
    Glb,
}

impl MeshFormat {
    /// The format a file extension names, ignoring case.
    #[must_use]
    pub fn of_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "obj" => Some(Self::Obj),
            "stl" => Some(Self::Stl),
            "gltf" => Some(Self::Gltf),
            "glb" => Some(Self::Glb),
            _ => None,
        }
    }

    /// The name of the format in a loss report.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Obj => "obj",
            Self::Stl => "stl",
            Self::Gltf | Self::Glb => "gltf",
        }
    }

    const fn is_gltf(self) -> bool {
        matches!(self, Self::Gltf | Self::Glb)
    }
}

/// The unit coordinates are written in. glTF is always in meters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Unit {
    /// Meters, the unit of the model.
    #[default]
    Meter,
    /// Centimeters.
    Centimeter,
    /// Millimeters.
    Millimeter,
    /// International feet.
    Foot,
    /// International inches.
    Inch,
}

impl Unit {
    /// How many of this unit make a meter.
    #[must_use]
    pub const fn per_meter(self) -> f64 {
        match self {
            Self::Meter => 1.0,
            Self::Centimeter => 100.0,
            Self::Millimeter => 1000.0,
            Self::Foot => 1.0 / 0.3048,
            Self::Inch => 1.0 / 0.0254,
        }
    }
}

/// What the caller decides about a mesh export.
#[derive(Debug, Clone)]
pub struct MeshOptions {
    /// The format.
    pub format: MeshFormat,
    /// The unit of the coordinates.
    pub unit: Unit,
    /// The only elements to write; `None` writes the whole model.
    pub only: Option<BTreeSet<ObjectId>>,
    /// Elements to leave out: the ones a caller's view does not show.
    pub hidden: BTreeSet<ObjectId>,
    /// One object for the whole model, split only by material, instead of one per element.
    pub merge: bool,
    /// Whether materials (colors) are written.
    pub materials: bool,
    /// The name of the MTL file the OBJ names in its `mtllib` statement.
    pub mtl_name: String,
    /// The originating system of the loss report.
    pub application: String,
    /// The time stamp of the loss report, an ISO 8601 date and time.
    pub timestamp: String,
    /// The file name of the loss report.
    pub file_name: String,
    /// The limits of the meshes.
    pub limits: Limits,
    /// Cancels the export between elements.
    pub cancel: Cancel,
}

impl Default for MeshOptions {
    fn default() -> Self {
        Self {
            format: MeshFormat::Obj,
            unit: Unit::Meter,
            only: None,
            hidden: BTreeSet::new(),
            merge: false,
            materials: true,
            mtl_name: String::new(),
            application: String::new(),
            timestamp: String::new(),
            file_name: String::new(),
            limits: Limits::DEFAULT,
            cancel: Cancel::new(),
        }
    }
}

/// A finished mesh export.
#[derive(Debug, Clone)]
pub struct MeshExport {
    /// The bytes of the file.
    pub bytes: Vec<u8>,
    /// The bytes of the MTL library of an OBJ file with materials.
    pub mtl: Option<Vec<u8>>,
    /// The elements written.
    pub elements: usize,
    /// The triangles written.
    pub triangles: usize,
    /// The bounding box of the written positions, in the unit of the file.
    pub bounds: Option<[[f64; 3]; 2]>,
    /// The loss report: the writer's items and what a mesh cannot carry.
    pub report: LossReport,
}

/// The materials: a stable name, the color as linear RGB and the opacity. The names are
/// identifiers, not messages, so a file reads the same in every language.
const MATERIALS: [(&str, [f32; 3], f32); 24] = [
    ("wall.core", [0.62, 0.58, 0.50], 1.0),
    ("wall.finish", [0.80, 0.77, 0.70], 1.0),
    ("wall.layer", [0.72, 0.66, 0.46], 1.0),
    ("wall", [0.62, 0.58, 0.50], 1.0),
    ("curtain.glass", [0.45, 0.68, 0.80], 0.35),
    ("curtain.panel", [0.70, 0.70, 0.68], 1.0),
    ("curtain.frame", [0.28, 0.30, 0.33], 1.0),
    ("door.leaf", [0.45, 0.27, 0.12], 1.0),
    ("door.handle", [0.85, 0.70, 0.20], 1.0),
    ("window.glass", [0.45, 0.72, 0.92], 0.35),
    ("insert.frame", [0.80, 0.78, 0.72], 1.0),
    ("floor", [0.60, 0.70, 0.80], 1.0),
    ("floor_edge", [0.62, 0.62, 0.60], 1.0),
    ("roof", [0.62, 0.28, 0.20], 1.0),
    ("roof.fascia", [0.80, 0.78, 0.72], 1.0),
    ("roof.gutter", [0.45, 0.48, 0.52], 1.0),
    ("dormer.wall", [0.78, 0.74, 0.68], 1.0),
    ("dormer.roof", [0.55, 0.30, 0.25], 1.0),
    ("ceiling", [0.86, 0.82, 0.72], 1.0),
    ("stair", [0.72, 0.70, 0.66], 1.0),
    ("railing", [0.25, 0.27, 0.30], 1.0),
    ("ramp", [0.72, 0.70, 0.66], 1.0),
    ("elevator.car", [0.55, 0.58, 0.62], 1.0),
    ("generic", [0.70, 0.70, 0.70], 1.0),
];

/// The solids of one element, each with the material it is drawn with.
type Bodies = Vec<(&'static str, Polyhedron)>;

/// The kinds that have a body, in the order they are written.
const BODY_KINDS: [&str; 14] = [
    WALL,
    DOOR,
    WINDOW,
    FLOOR,
    FLOOR_EDGE,
    ROOF,
    ROOF_EXTRUSION,
    ROOF_EDGE,
    DORMER,
    CEILING,
    STAIR,
    RAMP,
    RAILING,
    ELEVATOR,
];

/// The kinds that are not geometry, with what the report says of each: rooms are volumes of air,
/// the others are drawing aids and lists.
const BODYLESS: [&str; 5] = [
    ROOM,
    types::GRID,
    REFERENCE_PLANE,
    WORK_PLANE,
    SELECTION_SET,
];

/// An element with its meshes.
struct Entry {
    id: Option<ObjectId>,
    key: &'static str,
    label: String,
    parts: Vec<(&'static str, TriMesh)>,
}

/// A loss of the export, before it is a report item.
struct Loss {
    reason: Reason,
    kind: String,
    object: Option<ObjectId>,
    name: String,
    count: i64,
}

fn name_of(object: &Object) -> String {
    types::text(&object.values, 0)
        .unwrap_or_default()
        .to_owned()
}

/// The name of an element in the file: its name and its identifier. The characters an OBJ name
/// cannot hold (comment, continuation, line breaks) become underscores.
fn label(name: &str, id: ObjectId) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if matches!(c, '#' | '\\') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("{} [{id}]", clean.trim())
}

fn curtain_material(class: archbuilder_model::curtain::Class) -> &'static str {
    use archbuilder_model::curtain::Class;
    match class {
        Class::Glass => "curtain.glass",
        Class::Solid => "curtain.panel",
        Class::Mullion | Class::Frame => "curtain.frame",
        Class::Leaf => "door.leaf",
        Class::Handle => "door.handle",
    }
}

fn insert_material(role: Role, window: bool) -> &'static str {
    match role {
        Role::Frame => "insert.frame",
        Role::Panel => "door.leaf",
        Role::Glass if window => "window.glass",
        Role::Glass => "window.glass",
        Role::Handle => "door.handle",
    }
}

fn storey_elevation(snapshot: &Snapshot, object: &Object) -> f64 {
    object
        .header
        .owner
        .and_then(|id| spatial::get(snapshot, id))
        .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s))
}

/// The solids of `object`, which is of the kind `key`. `Ok(None)` for a record that is not
/// readable (nothing to draw, as in the viewport).
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    reason = "one arm for each kind with a body"
)]
fn bodies(snapshot: &Snapshot, key: &str, object: &Object) -> Result<Option<Bodies>> {
    let mut out: Bodies = Vec::new();
    match key {
        WALL => {
            let Some(wall) = Wall::of(snapshot, object) else {
                return Ok(None);
            };
            let _ = wall;
            if let Some(curtain) = archbuilder_model::curtain::Derived::of(snapshot, object) {
                for piece in curtain.solids()? {
                    out.push((curtain_material(piece.class), piece.solid));
                }
                return Ok(Some(out));
            }
            let pieces = cleanup::wall_pieces(snapshot, object)
                .and_then(Result::ok)
                .filter(|p| !p.is_empty());
            match pieces {
                Some(pieces) => {
                    for piece in pieces {
                        let material = match piece.class {
                            PieceClass::Core => "wall.core",
                            PieceClass::Finish => "wall.finish",
                            PieceClass::Other => "wall.layer",
                        };
                        out.push((material, piece.solid));
                    }
                }
                None => match insert::wall_solid(snapshot, object) {
                    Some(solid) => out.push(("wall", solid?)),
                    None => return Ok(None),
                },
            }
        }
        DOOR | WINDOW => {
            let Some(host) = object
                .header
                .owner
                .and_then(|id| spatial::get(snapshot, id))
            else {
                return Ok(None);
            };
            let (Some(item), Some(wall)) =
                (Insert::of(snapshot, object), Wall::of(snapshot, &host))
            else {
                return Ok(None);
            };
            let elevation = storey_elevation(snapshot, &host);
            for (role, solid) in insert::pieces(&wall, elevation, &item)? {
                out.push((insert_material(role, key == WINDOW), solid));
            }
        }
        FLOOR => {
            let Some(floor) = archbuilder_model::floor::Derived::of(snapshot, object) else {
                return Ok(None);
            };
            let solid = match cleanup::floor_solid(snapshot, object) {
                Some(Ok(solid)) => solid,
                _ => floor.solid()?,
            };
            out.push(("floor", solid));
        }
        FLOOR_EDGE => {
            let Some(derived) = archbuilder_model::floor_edge::Derived::of(snapshot, object) else {
                return Ok(None);
            };
            if let Some(solid) = derived.solid()? {
                out.push(("floor_edge", solid));
            }
        }
        ROOF => {
            let Ok(derived) = archbuilder_model::roof::Derived::of(snapshot, object) else {
                return Ok(None);
            };
            out.push(("roof", derived.solid_cut()?.0));
        }
        ROOF_EXTRUSION => {
            let Ok(derived) = archbuilder_model::roof_extrusion::Derived::of(snapshot, object)
            else {
                return Ok(None);
            };
            out.push(("roof", derived.solid()?));
        }
        ROOF_EDGE => {
            let Some(derived) = archbuilder_model::roof_edge::Derived::of(snapshot, object) else {
                return Ok(None);
            };
            if let Some(solid) = derived.solid()? {
                let material = match derived.element.role {
                    archbuilder_model::roof_edge::Role::Fascia => "roof.fascia",
                    archbuilder_model::roof_edge::Role::Gutter => "roof.gutter",
                };
                out.push((material, solid));
            }
        }
        DORMER => {
            let Ok(derived) = archbuilder_model::dormer::Derived::of(snapshot, object) else {
                return Ok(None);
            };
            for (kind, solid) in derived.parts {
                let material = if kind == archbuilder_model::dormer::PartKind::Roof {
                    "dormer.roof"
                } else {
                    "dormer.wall"
                };
                out.push((material, solid));
            }
        }
        CEILING => {
            let Some(shared) = spatial::get(snapshot, object.id) else {
                return Ok(None);
            };
            let Some(derived) = archbuilder_model::ceiling::derived(snapshot, &shared) else {
                return Ok(None);
            };
            out.push(("ceiling", derived.solid()?));
        }
        STAIR => {
            let Some(placed) = archbuilder_model::stair::Placed::of(snapshot, object) else {
                return Ok(None);
            };
            let Ok(layout) = &placed.layout else {
                return Ok(None);
            };
            out.extend(
                layout
                    .solids(placed.base)?
                    .into_iter()
                    .map(|s| ("stair", s)),
            );
            out.extend(
                layout
                    .railing_solids(placed.base)?
                    .into_iter()
                    .map(|s| ("railing", s)),
            );
        }
        RAMP => {
            let Some(placed) = archbuilder_model::ramp::Placed::of(snapshot, object) else {
                return Ok(None);
            };
            let Ok(layout) = &placed.layout else {
                return Ok(None);
            };
            out.push(("ramp", layout.solid(placed.base)?));
        }
        RAILING => {
            let Ok(derived) = archbuilder_model::railing::Derived::of(snapshot, object) else {
                return Ok(None);
            };
            out.extend(derived.solids()?.into_iter().map(|s| ("railing", s)));
        }
        ELEVATOR => {
            let Some(derived) = archbuilder_model::elevator::derived(snapshot, object) else {
                return Ok(None);
            };
            let region =
                archbuilder_model::floor::Boundary::polygons(vec![derived.elevator.car_outline()])
                    .region()?;
            let solid = arc_geom::prism(
                &region,
                derived.base,
                ELEVATOR_CAR_HEIGHT,
                0.01,
                &Tolerance::DEFAULT,
                &Limits::DEFAULT,
            )?;
            out.push(("elevator.car", solid));
        }
        _ => return Ok(None),
    }
    Ok(Some(out))
}

/// The height of the car of an elevator, as the viewport draws it, in meters.
const ELEVATOR_CAR_HEIGHT: f64 = 2.4;

/// The triangles of `solid` with every coordinate multiplied by `scale`.
fn mesh_of(solid: &Polyhedron, scale: f64, limits: &Limits) -> Result<TriMesh> {
    let mesh = TriMesh::from_polyhedron(solid, &Tolerance::DEFAULT, limits)?;
    let positions = mesh
        .positions()
        .iter()
        .map(|p| Point3::new(p.x * scale, p.y * scale, p.z * scale))
        .collect();
    TriMesh::new(positions, mesh.triangles().to_vec(), limits)
}

/// One mesh of `meshes`, which share a material.
fn join(meshes: &[&TriMesh], limits: &Limits) -> Result<TriMesh> {
    let mut positions = Vec::new();
    let mut triangles = Vec::new();
    for mesh in meshes {
        let base = u32::try_from(positions.len()).map_err(|_| {
            Error::new(arc_io_mesh::codes::LIMIT_EXCEEDED).with_arg("limit", "array_length")
        })?;
        positions.extend_from_slice(mesh.positions());
        triangles.extend(
            mesh.triangles()
                .iter()
                .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
        );
    }
    TriMesh::new(positions, triangles, limits)
}

/// The sRGB encoding of a linear color component, for the MTL library, whose colors are meant for
/// display as they are.
fn srgb(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

fn material_of(name: &str, gltf_file: bool) -> Material {
    let (_, color, alpha) = MATERIALS
        .iter()
        .find(|(n, _, _)| *n == name)
        .copied()
        .unwrap_or(("generic", [0.7, 0.7, 0.7], 1.0));
    if gltf_file {
        Material {
            name: name.to_owned(),
            base_color_factor: Some([color[0], color[1], color[2], alpha]),
            metallic_factor: Some(0.0),
            roughness_factor: Some(0.8),
            alpha_mode: Some(if alpha < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            }),
            double_sided: true,
            ..Material::default()
        }
    } else {
        let encoded = color.map(srgb);
        Material {
            name: name.to_owned(),
            diffuse: Some(encoded),
            ambient: Some(encoded.map(|c| c * 0.4)),
            specular: Some([0.1; 3]),
            opacity: Some(alpha),
            ..Material::default()
        }
    }
}

/// The element identity as the `extras` of a glTF node, in canonical form.
fn extras(entry: &Entry, name: &str) -> Preserved {
    let value = serde_json::json!({
        "archbuilder": { "id": entry.id.map(|i| i.to_string()), "kind": entry.key, "name": name }
    });
    Preserved {
        extensions: None,
        extras: serde_json::to_string(&value).ok(),
    }
}

/// Makes the scene of a collected set of elements.
fn scene_of(entries: &[Entry], options: &MeshOptions, project: &str) -> Result<MeshScene> {
    let gltf_file = options.format.is_gltf();
    let mut scene = MeshScene::default();
    let used: BTreeSet<&str> = entries
        .iter()
        .flat_map(|e| e.parts.iter().map(|(m, _)| *m))
        .collect();
    if options.materials {
        scene.materials = used.iter().map(|m| material_of(m, gltf_file)).collect();
    }
    let material = |name: &str| options.materials.then(|| name.to_owned());
    // The element's parts, or the parts of everything when merged: one per material.
    let merged;
    let entries: &[Entry] = if options.merge {
        let mut by_material: BTreeMap<&'static str, Vec<&TriMesh>> = BTreeMap::new();
        for entry in entries {
            for (m, mesh) in &entry.parts {
                by_material.entry(m).or_default().push(mesh);
            }
        }
        let mut parts = Vec::new();
        for (m, meshes) in by_material {
            parts.push((m, join(&meshes, &options.limits)?));
        }
        merged = [Entry {
            id: None,
            key: "archbuilder.project",
            label: project.to_owned(),
            parts,
        }];
        &merged
    } else {
        entries
    };
    let mut children = Vec::new();
    for entry in entries {
        let mut primitives = Vec::new();
        for (m, mesh) in &entry.parts {
            primitives.push(scene.meshes.len());
            scene.meshes.push(MeshPart {
                name: entry.label.clone(),
                mesh: mesh.clone(),
                material: material(m),
                preserved: Preserved::default(),
            });
        }
        if gltf_file && !primitives.is_empty() {
            scene.mesh_groups.push(MeshGroup {
                name: entry.label.clone(),
                primitives,
                preserved: Preserved::default(),
            });
            children.push(scene.nodes.len() + 1);
            scene.nodes.push(Node {
                name: entry.label.clone(),
                mesh: Some(scene.mesh_groups.len() - 1),
                preserved: if options.merge {
                    Preserved::default()
                } else {
                    extras(entry, &entry.label)
                },
                ..Node::default()
            });
        }
    }
    if gltf_file {
        // The model is Z up and glTF is Y up: one root node turns the model, so positions stay
        // the model's own. Its children are the elements.
        let root = Node {
            name: project.to_owned(),
            transform: Transform::Trs {
                translation: [0.0; 3],
                rotation: [
                    -std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                    0.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                ],
                scale: [1.0; 3],
            },
            children: Vec::new(),
            ..Node::default()
        };
        // Node 0 is the root; the element nodes were numbered from 1 above.
        let mut nodes = vec![root];
        nodes.append(&mut scene.nodes);
        if let Some(first) = nodes.first_mut() {
            first.children = children;
        }
        scene.nodes = nodes;
        scene.roots = vec![0];
    }
    Ok(scene)
}

/// The element count, triangle count and bounds of a scene.
fn measure(scene: &MeshScene) -> (usize, [[f64; 3]; 2]) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut triangles = 0;
    for part in &scene.meshes {
        triangles += part.mesh.triangle_count();
        for p in part.mesh.positions() {
            for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
                lo[k] = lo[k].min(v);
                hi[k] = hi[k].max(v);
            }
        }
    }
    (triangles, [lo, hi])
}

/// Writes the elements of `snapshot` as a mesh file.
///
/// # Errors
/// `ab.io.mesh_unit_unsupported` for glTF in another unit than meters, `ab.io.mesh_empty` when
/// no element has a body to write, and the `io.mesh.*` codes and `base.cancelled` of the writers.
#[allow(clippy::too_many_lines, reason = "collect, report, write: one pass")]
pub fn export_mesh(snapshot: &Snapshot, options: &MeshOptions) -> Result<MeshExport> {
    if options.format.is_gltf() && options.unit != Unit::Meter {
        return Err(Error::new(crate::error::codes::MESH_UNIT_UNSUPPORTED));
    }
    let scale = options.unit.per_meter();
    let wanted = |id: ObjectId| {
        !options.hidden.contains(&id) && options.only.as_ref().is_none_or(|only| only.contains(&id))
    };
    let mut losses: Vec<Loss> = Vec::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut inserts = 0_i64;
    for key in BODY_KINDS {
        for object in spatial::objects(snapshot, key) {
            options.cancel.check()?;
            if !wanted(object.id) {
                continue;
            }
            let name = name_of(&object);
            let derived = bodies(snapshot, key, &object).and_then(|found| {
                found
                    .map(|parts| {
                        parts
                            .into_iter()
                            .map(|(m, solid)| Ok((m, mesh_of(&solid, scale, &options.limits)?)))
                            .collect::<Result<Vec<_>>>()
                    })
                    .transpose()
            });
            match derived {
                Ok(Some(parts)) => {
                    let parts: Vec<_> = parts
                        .into_iter()
                        .filter(|(_, m)| m.triangle_count() > 0)
                        .collect();
                    if parts.is_empty() {
                        continue;
                    }
                    if key == DOOR || key == WINDOW {
                        inserts += 1;
                    }
                    entries.push(Entry {
                        id: Some(object.id),
                        key: static_key(key),
                        label: label(&name, object.id),
                        parts,
                    });
                }
                Ok(None) => {}
                Err(_) => losses.push(Loss {
                    reason: reasons::MESH_BODY_FAILED,
                    kind: key.to_owned(),
                    object: Some(object.id),
                    name,
                    count: 1,
                }),
            }
        }
    }
    for key in BODYLESS {
        let count = spatial::objects(snapshot, key)
            .iter()
            .filter(|o| wanted(o.id))
            .count();
        if count > 0 {
            losses.push(Loss {
                reason: reasons::MESH_OBJECT_NOT_EXPORTED,
                kind: key.to_owned(),
                object: None,
                name: String::new(),
                count: i64::try_from(count).unwrap_or(i64::MAX),
            });
        }
    }
    if entries.is_empty() {
        return Err(Error::new(crate::error::codes::MESH_EMPTY));
    }
    let count = i64::try_from(entries.len()).unwrap_or(i64::MAX);
    losses.push(Loss {
        reason: reasons::MESH_PARAMETERS_NOT_CARRIED,
        kind: "archbuilder".to_owned(),
        object: None,
        name: String::new(),
        count,
    });
    losses.push(Loss {
        reason: reasons::MESH_RELATIONS_NOT_CARRIED,
        kind: "archbuilder".to_owned(),
        object: None,
        name: String::new(),
        count: inserts.max(1),
    });
    losses.push(Loss {
        reason: reasons::MESH_CURVES_FACETED,
        kind: "archbuilder".to_owned(),
        object: None,
        name: String::new(),
        count,
    });
    if !options.format.is_gltf() {
        losses.push(Loss {
            reason: reasons::MESH_UNIT_NOT_RECORDED,
            kind: options.format.name().to_owned(),
            object: None,
            name: String::new(),
            count: 1,
        });
    }
    let project = spatial::project(snapshot)
        .map(|p| name_of(&p))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "ArchBuilder".to_owned());
    let scene = scene_of(&entries, options, &project)?;
    let (triangles, bounds) = measure(&scene);
    let mut write_options = WriteOptions {
        cancel: options.cancel.clone(),
        mtl_name: (options.format == MeshFormat::Obj && options.materials)
            .then(|| options.mtl_name.clone())
            .filter(|n| !n.is_empty()),
        ..WriteOptions::default()
    };
    let written = match options.format {
        MeshFormat::Obj => obj::write(&scene, &mut write_options)?,
        MeshFormat::Stl => {
            write_options.encoding = Encoding::BinaryLittleEndian;
            stl::write(&scene, &mut write_options)?
        }
        MeshFormat::Gltf => gltf::write(&scene, &mut write_options)?,
        MeshFormat::Glb => {
            write_options.encoding = Encoding::BinaryLittleEndian;
            gltf::write(&scene, &mut write_options)?
        }
    };
    let mut report = written.report;
    report.header.file_name = Some(options.file_name.clone());
    report.header.application = Some(options.application.clone());
    report.header.time = Some(options.timestamp.clone());
    add_losses(&mut report, &losses);
    Ok(MeshExport {
        bytes: written.bytes,
        mtl: written.mtl,
        elements: if options.merge { 1 } else { entries.len() },
        triangles,
        bounds: (triangles > 0).then_some(bounds),
        report,
    })
}

/// The key of a body kind as a static string, for the entry.
fn static_key(key: &str) -> &'static str {
    BODY_KINDS.into_iter().find(|k| *k == key).unwrap_or("")
}

/// Adds `losses` to the writer's report, ordered like the report's own items.
fn add_losses(report: &mut LossReport, losses: &[Loss]) {
    for loss in losses {
        let count = u64::try_from(loss.count.max(1)).unwrap_or(1);
        match loss.reason.outcome {
            arc_io_common::loss::Outcome::Approximated => report.summary.approximated += count,
            _ => report.summary.dropped += count,
        }
        let mut arguments = BTreeMap::new();
        arguments.insert("count".to_owned(), Argument::Int(loss.count));
        arguments.insert("name".to_owned(), Argument::Text(loss.name.clone()));
        report.items.push(LossItem {
            outcome: loss.reason.outcome,
            severity: loss.reason.severity,
            source_id: None,
            record_kind: Some(loss.kind.clone()),
            object_id: loss.object.map(|o| o.to_string()),
            reason: loss.reason.code.to_owned(),
            message_id: loss.reason.message_id.to_owned(),
            arguments,
        });
    }
    report.items.sort_by(|a, b| {
        (&a.reason, &a.record_kind, &a.object_id).cmp(&(&b.reason, &b.record_kind, &b.object_id))
    });
}
