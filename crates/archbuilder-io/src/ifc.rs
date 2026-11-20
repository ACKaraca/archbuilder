//! The document as an IFC4 file: the project, site, building and storeys, and on each storey its
//! walls, the doors and windows in them and its floors, mapped to the neutral description of
//! [`arc_io_ifc::write`] and written by it. Lengths are meters. The result is deterministic for a
//! document and options: every `GlobalId` comes from an `ObjectId`, and the time stamp is the
//! caller's. The mapping is described in `docs/architecture/domain-model.md`.

use arc_base::id::Identifier;
use arc_base::math::{Point2, Point3, Vector3};
use arc_base::{Cancel, Error, Limits, ObjectId, Result};
use arc_doc::{Object, Snapshot};
use arc_io_common::loss::{Argument, LossItem, LossReport, Reason};
use arc_io_ifc::write::{
    Body, Element, ElementKind, Placement, Quantity, QuantitySet, QuantityValue, Spatial, Storey,
    derived_id, role,
};
use arc_io_ifc::{WriteModel, WriteOptions, write};
use archbuilder_model::floor::{Derived, signed_area};
use archbuilder_model::insert::{self, Facing, Hand, Insert, Kind};
use archbuilder_model::types::{DOOR, FLOOR, WINDOW, text};
use archbuilder_model::wall::Wall;
use archbuilder_model::{join, spatial};

use crate::error::{codes, reasons};

/// What the caller decides about an export.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// The `name` of the file header: the file's name without a directory.
    pub file_name: String,
    /// The time stamp of the file header, an ISO 8601 date and time. The caller reads its own
    /// clock; this crate never does.
    pub timestamp: String,
    /// The originating system of the file header and of the loss report.
    pub application: String,
    /// The limits of the writer.
    pub limits: Limits,
    /// Cancels the export between elements.
    pub cancel: Cancel,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            timestamp: String::new(),
            application: String::new(),
            limits: Limits::DEFAULT,
            cancel: Cancel::new(),
        }
    }
}

/// A finished export.
#[derive(Debug, Clone)]
pub struct Export {
    /// The bytes of the IFC4 file.
    pub bytes: Vec<u8>,
    /// The walls, floors, doors and windows written; the openings that void walls are not counted.
    pub elements: usize,
    /// What was not exported or only approximately; the writer's own entries are included.
    pub report: LossReport,
}

/// The local frame of a wall or an insert in plan: an origin, the direction of its x axis and the
/// y axis to the left of it.
#[derive(Clone, Copy)]
struct Frame {
    origin: [f64; 2],
    u: [f64; 2],
}

impl Frame {
    /// The frame at `origin` along `direction`; `None` for a direction without length.
    fn new(origin: [f64; 2], direction: [f64; 2]) -> Option<Self> {
        let length = direction[0].hypot(direction[1]);
        (length > 0.0 && length.is_finite()).then(|| Self {
            origin,
            u: [direction[0] / length, direction[1] / length],
        })
    }

    /// The plan point `x` along the frame and `y` to the left of it.
    fn at(&self, x: f64, y: f64) -> [f64; 2] {
        [
            self.origin[0] + self.u[0] * x - self.u[1] * y,
            self.origin[1] + self.u[1] * x + self.u[0] * y,
        ]
    }

    /// The coordinates of a plan point in the frame.
    fn local(&self, p: [f64; 2]) -> [f64; 2] {
        let (dx, dy) = (p[0] - self.origin[0], p[1] - self.origin[1]);
        [
            dx * self.u[0] + dy * self.u[1],
            -dx * self.u[1] + dy * self.u[0],
        ]
    }

    /// The placement at height `z` above the storey.
    fn placement(&self, z: f64) -> Placement {
        Placement::new(
            Point3::new(self.origin[0], self.origin[1], z),
            Vector3::z(),
            Vector3::new(self.u[0], self.u[1], 0.0),
        )
    }
}

/// A loop as profile points, counterclockwise when `ccw` and clockwise otherwise.
fn profile(points: &[[f64; 2]], ccw: bool) -> Vec<Point2> {
    let mut out: Vec<Point2> = points.iter().map(|p| Point2::new(p[0], p[1])).collect();
    if (signed_area(points) > 0.0) != ccw {
        out.reverse();
    }
    out
}

struct Loss {
    reason: Reason,
    kind: &'static str,
    object: ObjectId,
    name: String,
}

/// The mapping in progress.
struct Mapper<'a> {
    snapshot: &'a Snapshot,
    cancel: &'a Cancel,
    elements: Vec<Element>,
    losses: Vec<Loss>,
    exported: usize,
}

fn name_of(object: &Object) -> String {
    text(&object.values, 0).unwrap_or_default().to_owned()
}

impl Mapper<'_> {
    fn lose(&mut self, reason: Reason, kind: &'static str, object: &Object) {
        self.losses.push(Loss {
            reason,
            kind,
            object: object.id,
            name: name_of(object),
        });
    }

    /// The walls, then the floors of a storey whose level is at `level`.
    fn storey(&mut self, storey: ObjectId, level: f64) -> Result<()> {
        for wall in spatial::walls(self.snapshot, storey) {
            self.cancel.check()?;
            self.wall(&wall, storey, level);
        }
        for floor in spatial::children(self.snapshot, FLOOR, storey) {
            self.cancel.check()?;
            self.floor(&floor, storey);
        }
        Ok(())
    }

    fn wall(&mut self, object: &Object, storey: ObjectId, level: f64) {
        let Some(wall) = Wall::of(self.snapshot, object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCWALL", object);
        };
        let direction = [wall.end[0] - wall.start[0], wall.end[1] - wall.start[1]];
        let Some(frame) = Frame::new(wall.start, direction) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCWALL", object);
        };
        let footprint =
            join::footprint_of(self.snapshot, object).unwrap_or_else(|| wall.plain_footprint());
        let mut element = Element::new(
            object.id.as_u128(),
            ElementKind::Wall,
            &name_of(object),
            storey.as_u128(),
        );
        element.placement = frame.placement(wall.base_offset);
        let gross = wall.solid_in(footprint, level).map(|s| s.volume());
        let mut quantities = vec![
            Quantity::new("Length", QuantityValue::Length(wall.length())),
            Quantity::new("Width", QuantityValue::Length(wall.thickness)),
            Quantity::new("Height", QuantityValue::Length(wall.height)),
        ];
        if let Ok(volume) = gross {
            let local = footprint.map(|p| frame.local(p));
            element.body = Some(Body::Extrusion {
                outer: profile(&local, true),
                holes: Vec::new(),
                direction: Vector3::z(),
                depth: wall.height,
            });
            quantities.push(Quantity::new("GrossVolume", QuantityValue::Volume(volume)));
            match insert::wall_solid(self.snapshot, object) {
                Some(Ok(net)) => {
                    quantities.push(Quantity::new(
                        "NetVolume",
                        QuantityValue::Volume(net.volume()),
                    ));
                }
                _ => self.lose(reasons::QUANTITY_NOT_EXPORTED, "IFCWALL", object),
            }
        } else {
            self.lose(reasons::BODY_NOT_EXPORTED, "IFCWALL", object);
        }
        element
            .quantity_sets
            .push(QuantitySet::new("Qto_WallBaseQuantities", quantities));
        self.elements.push(element);
        self.exported += 1;
        self.inserts(object, &wall, &frame, storey, level);
    }

    /// The doors and windows of a wall, by distance along it and then identifier.
    fn inserts(&mut self, host: &Object, wall: &Wall, frame: &Frame, storey: ObjectId, level: f64) {
        let mut found: Vec<(std::sync::Arc<Object>, Insert)> = Vec::new();
        for (key, label) in [(DOOR, "IFCDOOR"), (WINDOW, "IFCWINDOW")] {
            for object in spatial::children(self.snapshot, key, host.id) {
                match Insert::of(self.snapshot, &object) {
                    Some(item) => found.push((object, item)),
                    None => self.lose(reasons::NOT_EXPORTED, label, &object),
                }
            }
        }
        found.sort_by(|a, b| {
            a.1.distance
                .total_cmp(&b.1.distance)
                .then_with(|| a.0.id.cmp(&b.0.id))
        });
        for (object, item) in found {
            self.insert(&object, &item, (host, wall, frame), storey, level);
        }
    }

    /// An opening that voids the host wall and the door or window that fills it. The insert's
    /// frame has its local y axis toward the side a door swings to, so the hand of the leaf
    /// is the hand of the IFC operation type.
    fn insert(
        &mut self,
        object: &Object,
        item: &Insert,
        (host, wall, wall_frame): (&Object, &Wall, &Frame),
        storey: ObjectId,
        level: f64,
    ) {
        let door = item.kind == Kind::Door;
        let label = if door { "IFCDOOR" } else { "IFCWINDOW" };
        let along = if door && item.facing == Facing::Interior {
            -1.0
        } else {
            1.0
        };
        let center = wall_frame.at(item.distance, 0.0);
        let direction = [wall_frame.u[0] * along, wall_frame.u[1] * along];
        let Some(frame) = Frame::new(center, direction) else {
            return self.lose(reasons::NOT_EXPORTED, label, object);
        };
        let z = wall.base_offset + item.sill;
        let (low, high) = wall.extent();
        let half = item.width / 2.0;
        let corners = [(-half, low), (half, low), (half, high), (-half, high)]
            .map(|(x, y)| frame.local(wall_frame.at(item.distance + x, y)));
        let storey_id = storey.as_u128();
        let opening_id = derived_id(object.id.as_u128(), role::INSERT_OPENING, 0);
        let mut opening = Element::new(
            opening_id,
            ElementKind::Opening,
            &name_of(object),
            storey_id,
        );
        opening.placement = frame.placement(z);
        opening.body = Some(Body::Extrusion {
            outer: profile(&corners, true),
            holes: Vec::new(),
            direction: Vector3::z(),
            depth: item.height,
        });
        opening.predefined_type = Some("OPENING".to_owned());
        opening.host = Some(host.id.as_u128());
        let mut element = Element::new(
            object.id.as_u128(),
            if door {
                ElementKind::Door
            } else {
                ElementKind::Window
            },
            &name_of(object),
            storey_id,
        );
        element.placement = frame.placement(z);
        element.predefined_type = Some(if door { "DOOR" } else { "WINDOW" }.to_owned());
        element.operation_type = Some(operation(item).to_owned());
        element.overall_width = Some(item.width);
        element.overall_height = Some(item.height);
        element.host = Some(opening_id);
        match insert::pieces(wall, level, item).map(|p| brep(&p, &frame, level + z)) {
            Ok(Some(body)) => element.body = Some(body),
            _ => self.lose(reasons::BODY_NOT_EXPORTED, label, object),
        }
        if !door && item.operation == insert::Operation::Casement {
            self.lose(reasons::WINDOW_OPERATION, label, object);
        }
        self.elements.push(opening);
        self.elements.push(element);
        self.exported += 1;
    }

    fn floor(&mut self, object: &Object, storey: ObjectId) {
        let Some(derived) = Derived::of(self.snapshot, object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCSLAB", object);
        };
        let thickness = derived.floor.thickness;
        let mut element = Element::new(
            object.id.as_u128(),
            ElementKind::Slab,
            &name_of(object),
            storey.as_u128(),
        );
        // The slab hangs from its top: the profile lies in the top plane and the extrusion
        // runs down by the thickness.
        element.placement = Placement::new(
            Point3::new(0.0, 0.0, derived.floor.top_offset),
            Vector3::z(),
            Vector3::x(),
        );
        element.predefined_type = Some("FLOOR".to_owned());
        let loops = &derived.boundary.loops;
        if let Some((outer, holes)) = loops.split_first() {
            element.body = Some(Body::Extrusion {
                outer: profile(outer, true),
                holes: holes.iter().map(|h| profile(h, false)).collect(),
                direction: -Vector3::z(),
                depth: thickness,
            });
            let gross = signed_area(outer).abs();
            let net = derived.area();
            element.quantity_sets.push(QuantitySet::new(
                "Qto_SlabBaseQuantities",
                vec![
                    Quantity::new("Width", QuantityValue::Length(thickness)),
                    Quantity::new("GrossArea", QuantityValue::Area(gross)),
                    Quantity::new("NetArea", QuantityValue::Area(net)),
                    Quantity::new("GrossVolume", QuantityValue::Volume(gross * thickness)),
                    Quantity::new("NetVolume", QuantityValue::Volume(net * thickness)),
                ],
            ));
        } else {
            self.lose(reasons::BODY_NOT_EXPORTED, "IFCSLAB", object);
        }
        self.elements.push(element);
        self.exported += 1;
    }
}

/// The IFC4 operation type of a door (`IfcDoorTypeOperationEnum`) or partitioning of a window
/// (`IfcWindowTypePartitioningEnum`). The placement's y axis points to the swing side. The product
/// reads the hand facing the leaf from that side, IFC4 along the y axis: opposite viewpoints, so
/// the product's left (hinge at +x) is `SINGLE_SWING_RIGHT` (IFC left has its hinge at -x). Fixed and casement windows are both one panel; IFC4 has
/// no operation attribute on the window itself, so a casement's is reported as approximated.
fn operation(item: &Insert) -> &'static str {
    match (item.kind, item.hand) {
        (Kind::Door, Hand::Left) => "SINGLE_SWING_RIGHT",
        (Kind::Door, Hand::Right) => "SINGLE_SWING_LEFT",
        (Kind::Window, _) => "SINGLE_PANEL",
    }
}

/// The solids of an insert as one faceted B-rep in the insert's frame; `z0` is the elevation of
/// the frame's origin. `None` when a face has holes, which a B-rep of loops cannot hold.
fn brep(pieces: &[(insert::Role, arc_geom::Polyhedron)], frame: &Frame, z0: f64) -> Option<Body> {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for (_, solid) in pieces {
        let first = vertices.len();
        vertices.extend(solid.vertices().iter().map(|v| {
            let [x, y] = frame.local([v.x, v.y]);
            Point3::new(x, y, v.z - z0)
        }));
        for face in solid.faces() {
            if !face.holes.is_empty() {
                return None;
            }
            faces.push(
                face.outer
                    .iter()
                    .map(|&i| first + usize::try_from(i).unwrap_or(usize::MAX))
                    .collect(),
            );
        }
    }
    Some(Body::Brep { vertices, faces })
}

/// Writes the document of `snapshot` as an IFC4 file.
///
/// The first building of the document is exported. Walls, floors, doors and windows that cannot
/// be mapped are left out and named in the loss report; the writer's own refusals (limits, an
/// invalid number) are errors.
///
/// # Errors
///
/// `ab.io.no_structure` for a document without a project, a site or a building; the `io.ifc.*`
/// codes and `base.cancelled` of the writer.
pub fn export_ifc(snapshot: &Snapshot, options: &ExportOptions) -> Result<Export> {
    let (Some(project), Some(site), Some(building)) = (
        spatial::project(snapshot),
        spatial::site(snapshot),
        spatial::buildings(snapshot).into_iter().next(),
    ) else {
        return Err(Error::new(codes::NO_STRUCTURE));
    };
    let spatial_of = |o: &Object| Spatial::new(o.id.as_u128(), &name_of(o));
    let mut model = WriteModel::new(
        spatial_of(&project),
        spatial_of(&site),
        spatial_of(&building),
    );
    let mut mapper = Mapper {
        snapshot,
        cancel: &options.cancel,
        elements: Vec::new(),
        losses: Vec::new(),
        exported: 0,
    };
    for other in spatial::buildings(snapshot).into_iter().skip(1) {
        mapper.lose(reasons::NOT_EXPORTED, "IFCBUILDING", &other);
    }
    for entry in spatial::storeys(snapshot, building.id) {
        let elevation = entry.elevation();
        model.storeys.push(Storey::new(
            entry.storey.id.as_u128(),
            &name_of(&entry.storey),
            elevation,
        ));
        mapper.storey(entry.storey.id, elevation)?;
    }
    model.elements = std::mem::take(&mut mapper.elements);
    let write_options = WriteOptions {
        limits: options.limits,
        cancel: options.cancel.clone(),
        file_name: options.file_name.clone(),
        timestamp: options.timestamp.clone(),
        originating_system: options.application.clone(),
        ..WriteOptions::default()
    };
    let written = write(&model, &write_options)?;
    let mut report = written.report;
    report.header.file_name = Some(options.file_name.clone());
    report.header.application = Some(options.application.clone());
    report.header.time = Some(options.timestamp.clone());
    add_losses(&mut report, &mapper.losses);
    Ok(Export {
        bytes: written.bytes,
        elements: mapper.exported,
        report,
    })
}

/// Adds the mapper's losses to the writer's report, ordered like the report's own items.
fn add_losses(report: &mut LossReport, losses: &[Loss]) {
    for loss in losses {
        let summary = &mut report.summary;
        match loss.reason.outcome {
            arc_io_common::loss::Outcome::Approximated => summary.approximated += 1,
            _ => summary.dropped += 1,
        }
        let mut arguments = std::collections::BTreeMap::new();
        arguments.insert("count".to_owned(), Argument::Int(1));
        arguments.insert("name".to_owned(), Argument::Text(loss.name.clone()));
        report.items.push(LossItem {
            outcome: loss.reason.outcome,
            severity: loss.reason.severity,
            source_id: None,
            record_kind: Some(loss.kind.to_owned()),
            object_id: Some(loss.object.to_string()),
            reason: loss.reason.code.to_owned(),
            message_id: loss.reason.message_id.to_owned(),
            arguments,
        });
    }
    report.items.sort_by(|a, b| {
        (&a.reason, &a.record_kind, &a.object_id).cmp(&(&b.reason, &b.record_kind, &b.object_id))
    });
}
