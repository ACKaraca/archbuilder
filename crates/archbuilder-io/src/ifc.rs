//! The document as an IFC4 file: the project, site, building and storeys, and on each storey its
//! walls, the doors and windows in them, its floors, its roofs and its enclosed rooms (spaces), mapped to the neutral description of
//! [`arc_io_ifc::write`] and written by it. Lengths are meters. The result is deterministic for a
//! document and options: every `GlobalId` comes from an `ObjectId`, and the time stamp is the
//! caller's. The mapping is described in `docs/architecture/domain-model.md`.

use arc_base::id::Identifier;
use arc_base::math::{Point2, Point3, Vector3};
use arc_base::{Cancel, Error, Limits, ObjectId, Result};
use arc_doc::{Object, Snapshot};
use arc_io_common::loss::{Argument, LossItem, LossReport, Reason};
use arc_io_ifc::write::{
    Body, Element, ElementKind, ElementType, LayerDirection, LayerSetUsage, MaterialLayer,
    Placement, Property, PropertySet, PropertyValue, Quantity, QuantitySet, QuantityValue, Spatial,
    Storey, derived_id, role,
};
use arc_io_ifc::{WriteModel, WriteOptions, write};
use archbuilder_model::element_type::{
    self as et, Exposure, Function, InsertType, Layers, TypeKind, WallType,
};
use archbuilder_model::floor::{Derived, signed_area};
use archbuilder_model::insert::{self, Facing, Hand, Insert, Kind};
use archbuilder_model::room::State;
use archbuilder_model::types::{
    CEILING, DOOR, FLOOR, ROOF, SHAFT, VERTICAL_OPENING, WALL_OPENING, WINDOW, text,
};
use archbuilder_model::wall::Wall;
use archbuilder_model::{cleanup, join, spatial};

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
    /// The walls, floors, roofs, rooms, doors and windows written; the openings that void walls are not counted.
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
    count: i64,
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
        self.lose_n(reason, kind, object, 1);
    }

    fn lose_n(&mut self, reason: Reason, kind: &'static str, object: &Object, count: usize) {
        self.losses.push(Loss {
            reason,
            kind,
            object: object.id,
            name: name_of(object),
            count: i64::try_from(count).unwrap_or(i64::MAX),
        });
    }

    /// A curtain wall: the writer of the foundation has no `IfcCurtainWall`, `IfcPlate` or
    /// `IfcMember`, so the wall, its panels and its mullions are named in the loss report, never
    /// written as a wall or a proxy.
    fn curtain_wall(&mut self, object: &Object, derived: &archbuilder_model::curtain::Derived) {
        let (panels, mullions) = derived.counts();
        self.lose(reasons::CURTAIN_NOT_WRITTEN, "IFCCURTAINWALL", object);
        self.lose_n(reasons::CURTAIN_NOT_WRITTEN, "IFCPLATE", object, panels);
        self.lose_n(reasons::CURTAIN_NOT_WRITTEN, "IFCMEMBER", object, mullions);
    }

    /// The walls, then the floors of a storey whose level is at `level`.
    fn storey(&mut self, storey: ObjectId, level: f64) -> Result<()> {
        for wall in spatial::walls(self.snapshot, storey) {
            self.cancel.check()?;
            self.wall(&wall, storey, level);
        }
        for floor in spatial::children(self.snapshot, FLOOR, storey) {
            self.cancel.check()?;
            self.floor(&floor, storey, level);
        }
        for roof in spatial::children(self.snapshot, ROOF, storey) {
            self.cancel.check()?;
            self.roof(&roof, storey, level);
        }
        for ceiling in spatial::children(self.snapshot, CEILING, storey) {
            self.cancel.check()?;
            self.ceiling(&ceiling, storey, level);
        }
        for shaft in spatial::children(self.snapshot, SHAFT, storey) {
            self.cancel.check()?;
            self.shaft(&shaft, storey, level);
        }
        for entry in archbuilder_model::room::of_storey(self.snapshot, storey) {
            self.cancel.check()?;
            self.room(&entry, storey, level);
        }
        // No stair entities in the writer: each stair is named in the loss report, never written
        // as a proxy. Its shaft opening is already a hole in the floors above.
        for stair in spatial::children(self.snapshot, archbuilder_model::types::STAIR, storey) {
            self.lose(reasons::STAIR_NOT_WRITTEN, "IFCSTAIR", &stair);
        }
        Ok(())
    }

    /// An enclosed room as an `IfcSpace`: the number is the `Name`, the room's name the `LongName`,
    /// the region (with its courtyards as voids) extruded by the room's height, with the derived
    /// quantities: `NetFloorArea` at the wall faces, `GrossFloorArea` to the wall center lines,
    /// `NetPerimeter` the outer loop and every courtyard loop. A room that is not
    /// enclosed has no region and one that shares its region with another room would be a second
    /// space on the same boundary; both are named in the loss report.
    fn room(&mut self, entry: &archbuilder_model::room::Entry, storey: ObjectId, level: f64) {
        let Some(object) = spatial::get(self.snapshot, entry.id) else {
            return;
        };
        let reason = match entry.state() {
            State::Enclosed => None,
            State::Unenclosed => Some(reasons::ROOM_UNENCLOSED),
            State::Redundant => Some(reasons::ROOM_REDUNDANT),
        };
        if let Some(reason) = reason {
            return self.lose(reason, "IFCSPACE", &object);
        }
        // The room's solid: under the lowest ceiling, roof or slab above it, else its upper limit.
        let shape = archbuilder_model::room::shape(self.snapshot, entry);
        let height = shape
            .as_ref()
            .map_or_else(|| (entry.top - entry.base).max(0.0), |s| s.height);
        // The shafts that pass through the room are out of its area, its body and its volume.
        let (Some(area), Some(gross), Some(perimeter), Some(volume)) = (
            archbuilder_model::room::net_area(self.snapshot, entry),
            entry.footprint.gross_area(),
            entry.perimeter(),
            shape.as_ref().map(|s| s.volume).or_else(|| entry.volume()),
        ) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCSPACE", &object);
        };
        let (name, long_name) = match &entry.room.number {
            Some(number) => (number.clone(), Some(entry.room.name.clone())),
            None => (entry.room.name.clone(), None),
        };
        let mut element = Element::new(
            entry.id.as_u128(),
            ElementKind::Space,
            &name,
            storey.as_u128(),
        );
        element.long_name = long_name;
        element.predefined_type = Some("INTERNAL".to_owned());
        element.placement = Placement::new(
            Point3::new(0.0, 0.0, entry.base - level),
            Vector3::z(),
            Vector3::x(),
        );
        let flat = shape.as_ref().and_then(|s| s.flat_top());
        // A sloped top, or a shaft through the room, needs the room's own solid; a failure is
        // reported, never replaced by a flat extrusion at some other height.
        let own = shape
            .as_ref()
            .filter(|s| flat.is_none() || entry.area().is_some_and(|a| s.area < a - 1.0e-6));
        element.body = match own.map(|s| s.solid()) {
            Some(Ok(solid)) => brep(
                [&solid],
                &Frame {
                    origin: [0.0, 0.0],
                    u: [1.0, 0.0],
                },
                entry.base,
            ),
            Some(Err(_)) => {
                self.lose(reasons::BODY_NOT_EXPORTED, "IFCSPACE", &object);
                None
            }
            None => Some(Body::Extrusion {
                outer: profile(&entry.footprint.polygon, true),
                holes: entry
                    .footprint
                    .holes
                    .iter()
                    .map(|h| profile(h, false))
                    .collect(),
                direction: Vector3::z(),
                depth: flat.map_or(height, |z| (z - entry.base).max(0.0)),
            }),
        };
        element.quantity_sets.push(QuantitySet::new(
            "Qto_SpaceBaseQuantities",
            vec![
                Quantity::new("NetFloorArea", QuantityValue::Area(area)),
                Quantity::new("GrossFloorArea", QuantityValue::Area(gross)),
                Quantity::new("NetPerimeter", QuantityValue::Length(perimeter)),
                Quantity::new("Height", QuantityValue::Length(height)),
                Quantity::new("NetVolume", QuantityValue::Volume(volume)),
            ],
        ));
        if let Some(number) = &entry.room.number {
            element.property_sets.push(PropertySet::new(
                "Pset_SpaceCommon",
                vec![Property::new(
                    "Reference",
                    PropertyValue::Label(number.clone()),
                )],
            ));
        }
        self.elements.push(element);
        self.exported += 1;
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one wall: body, quantities and inserts"
    )]
    fn wall(&mut self, object: &Object, storey: ObjectId, level: f64) {
        if let Some(derived) = archbuilder_model::curtain::Derived::of(self.snapshot, object) {
            return self.curtain_wall(object, &derived);
        }
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
        if let Some(found) = et::type_of(self.snapshot, TypeKind::Wall, &object.values) {
            element.element_type = Some(found.id.as_u128());
            // The layers run from the exterior face, on the left of the baseline: the local y
            // axis of the placement, toward its negative side.
            let (_, high) = wall.extent();
            element.layer_usage = Some(LayerSetUsage::new(LayerDirection::Axis2, false, high));
            if let Some(t) = WallType::from_values(&found.values) {
                element.property_sets.push(PropertySet::new(
                    "Pset_WallCommon",
                    vec![
                        Property::new(
                            "IsExternal",
                            PropertyValue::Boolean(t.exposure == Exposure::Exterior),
                        ),
                        Property::new("LoadBearing", PropertyValue::Boolean(t.load_bearing)),
                    ],
                ));
            }
        }
        let gross = wall.solid_in(footprint, level).map(|s| s.volume());
        let mut quantities = vec![
            Quantity::new("Length", QuantityValue::Length(wall.length())),
            Quantity::new("Width", QuantityValue::Length(wall.thickness)),
            Quantity::new("Height", QuantityValue::Length(wall.height)),
        ];
        // A wall whose top follows a roof has its clipped solid as a B-rep.
        let clipped = insert::attached_roof(self.snapshot, object).and_then(|(_, surface)| {
            archbuilder_model::wall::under_roof(&wall, footprint, level, &surface).ok()
        });
        let gross = match &clipped {
            Some(solid) => {
                let top = solid.bounding_box().max().z - (level + wall.base_offset);
                for q in &mut quantities {
                    if q.name == "Height" {
                        q.value = QuantityValue::Length(top);
                    }
                }
                Ok(solid.volume())
            }
            None => gross,
        };
        if let Ok(volume) = gross {
            let local = footprint.map(|p| frame.local(p));
            // A wall of several layers is its cleaned layers: they differ from the plain prism at
            // joins, wrapped ends and openings and where slabs meet it.
            // The body is the layers of the wall, cleaned at joins, openings, slabs and roofs.
            let solids = cleanup::wall_pieces_whole(self.snapshot, object)
                .and_then(Result::ok)
                .filter(|p| !p.is_empty());
            let pieces = solids
                .as_ref()
                .and_then(|p| brep(p.iter().map(|x| &x.solid), &frame, level + wall.base_offset));
            // The net volume is that of the layers with the openings cut: what the body is once
            // its openings void it.
            let has_pieces = pieces.is_some();
            let body_volume = has_pieces.then(|| {
                cleanup::wall_pieces(self.snapshot, object)
                    .and_then(Result::ok)
                    .map(|p| p.iter().map(|x| x.solid.volume()).sum::<f64>())
            });
            element.body = match &clipped {
                _ if has_pieces => pieces,
                Some(solid) => brep([solid], &frame, level + wall.base_offset),
                None => Some(Body::Extrusion {
                    outer: profile(&local, true),
                    holes: Vec::new(),
                    direction: Vector3::z(),
                    depth: wall.height,
                }),
            };
            // The gross volume is that of the body written: the layers without the openings.
            let gross_body = has_pieces.then(|| {
                solids
                    .iter()
                    .flatten()
                    .map(|p| p.solid.volume())
                    .sum::<f64>()
            });
            quantities.push(Quantity::new(
                "GrossVolume",
                QuantityValue::Volume(gross_body.unwrap_or(volume)),
            ));
            match (body_volume, insert::wall_solid(self.snapshot, object)) {
                (Some(Some(net)), _) => {
                    quantities.push(Quantity::new("NetVolume", QuantityValue::Volume(net)));
                }
                (None, Some(Ok(net))) => {
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
        for (key, label) in [
            (DOOR, "IFCDOOR"),
            (WINDOW, "IFCWINDOW"),
            (WALL_OPENING, "IFCOPENINGELEMENT"),
        ] {
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
        // The openings that curtain walls inside this wall cut; each is named after its wall.
        for (id, item) in archbuilder_model::curtain::embedded(self.snapshot, host) {
            if let Some(curtain) = spatial::get(self.snapshot, id) {
                self.wall_opening(&curtain, &item, (host, wall, frame), storey);
            }
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
        if item.kind == Kind::Opening {
            return self.wall_opening(object, item, (host, wall, wall_frame), storey);
        }
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
        let kind = if door {
            TypeKind::Door
        } else {
            TypeKind::Window
        };
        element.element_type =
            et::type_of(self.snapshot, kind, &object.values).map(|t| t.id.as_u128());
        element.predefined_type = Some(if door { "DOOR" } else { "WINDOW" }.to_owned());
        element.operation_type = Some(operation(item).to_owned());
        element.overall_width = Some(item.width);
        element.overall_height = Some(item.height);
        element.host = Some(opening_id);
        match insert::pieces(wall, level, item)
            .map(|p| brep(p.iter().map(|(_, s)| s), &frame, level + z))
        {
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

    /// A bare wall opening: an `IfcOpeningElement` that voids the wall. Its profile lies in the
    /// plane of the wall (x along the baseline, y up) and is extruded across the wall's
    /// thickness; an arched top is written as chords.
    fn wall_opening(
        &mut self,
        object: &Object,
        item: &Insert,
        (host, wall, wall_frame): (&Object, &Wall, &Frame),
        storey: ObjectId,
    ) {
        let (low, high) = wall.extent();
        let half = item.width / 2.0;
        let spring = if item.rise > 0.0 && item.rise < item.height {
            item.top() - item.rise
        } else {
            item.top()
        };
        // The profile in the wall's plane, relative to the sill's lower left corner.
        let mut points = vec![
            [0.0, 0.0],
            [item.width, 0.0],
            [item.width, spring - item.sill],
        ];
        if spring < item.top() {
            let bulge = 2.0 * item.rise / item.width;
            points.extend(
                archbuilder_model::floor::flatten_edge(
                    [item.width, spring - item.sill],
                    [0.0, spring - item.sill],
                    bulge,
                    archbuilder_model::floor::FLAT_CHORD,
                )
                .into_iter()
                .skip(1)
                .take_while(|p| p[0] > 1.0e-9),
            );
            self.lose(reasons::ARC_FACETED, "IFCOPENINGELEMENT", object);
        } else {
            points.push([0.0, item.height]);
        }
        points.push([0.0, spring - item.sill]);
        points.dedup_by(|a, b| (a[0] - b[0]).abs() < 1.0e-9 && (a[1] - b[1]).abs() < 1.0e-9);
        let origin = wall_frame.at(item.distance - half, high);
        let u = wall_frame.u;
        // Local z runs across the wall from its left face to its right face, local y up.
        let mut opening = Element::new(
            derived_id(object.id.as_u128(), role::INSERT_OPENING, 0),
            ElementKind::Opening,
            &name_of(object),
            storey.as_u128(),
        );
        opening.placement = Placement::new(
            Point3::new(origin[0], origin[1], wall.base_offset + item.sill),
            Vector3::new(u[1], -u[0], 0.0),
            Vector3::new(u[0], u[1], 0.0),
        );
        opening.body = Some(Body::Extrusion {
            outer: profile(&points, true),
            holes: Vec::new(),
            direction: Vector3::z(),
            depth: high - low,
        });
        opening.predefined_type = Some("OPENING".to_owned());
        opening.host = Some(host.id.as_u128());
        self.elements.push(opening);
        self.exported += 1;
    }

    /// A ceiling as an `IfcCovering` with the predefined type `CEILING`: a prism over its
    /// boundary (a B-rep when it slopes), the layers of its type on the `IfcCoveringType`, and
    /// the area, width and volume of its body. The shafts and vertical openings that pass
    /// through it are `IfcOpeningElement`s that void it.
    fn ceiling(&mut self, object: &std::sync::Arc<Object>, storey: ObjectId, level: f64) {
        let Some(derived) = archbuilder_model::ceiling::derived(self.snapshot, object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCCOVERING", object);
        };
        let mut element = Element::new(
            object.id.as_u128(),
            ElementKind::Covering,
            &name_of(object),
            storey.as_u128(),
        );
        element.predefined_type = Some("CEILING".to_owned());
        element.element_type =
            et::type_of(self.snapshot, TypeKind::Ceiling, &object.values).map(|t| t.id.as_u128());
        let thickness = derived.ceiling.thickness();
        let origin = Frame {
            origin: [0.0, 0.0],
            u: [1.0, 0.0],
        };
        if derived.boundary.has_arcs() {
            self.lose(reasons::ARC_FACETED, "IFCCOVERING", object);
        }
        let body = if derived.ceiling.sloped() {
            derived
                .solid()
                .ok()
                .and_then(|s| brep([&s], &origin, level))
        } else {
            element.placement = Placement::new(
                Point3::new(0.0, 0.0, derived.plane.z - level),
                Vector3::z(),
                Vector3::x(),
            );
            let flat = derived
                .boundary
                .faceted(archbuilder_model::floor::FLAT_CHORD)
                .loops;
            flat.split_first().map(|(outer, holes)| Body::Extrusion {
                outer: profile(outer, true),
                holes: holes.iter().map(|h| profile(h, false)).collect(),
                direction: Vector3::z(),
                depth: thickness,
            })
        };
        match body {
            Some(body) => {
                element.body = Some(body);
                // Both areas are of the faceted boundary the body is made of.
                let net = derived
                    .boundary
                    .faceted(archbuilder_model::floor::FLAT_CHORD)
                    .area();
                let gross = archbuilder_model::floor::signed_area(
                    derived
                        .raw
                        .faceted(archbuilder_model::floor::FLAT_CHORD)
                        .loops
                        .first()
                        .map_or(&[][..], Vec::as_slice),
                )
                .abs();
                element.quantity_sets.push(QuantitySet::new(
                    "Qto_CoveringBaseQuantities",
                    vec![
                        Quantity::new("Width", QuantityValue::Length(thickness)),
                        Quantity::new("GrossArea", QuantityValue::Area(gross)),
                        Quantity::new("NetArea", QuantityValue::Area(net)),
                    ],
                ));
                element.property_sets.push(PropertySet::new(
                    "Pset_CoveringCommon",
                    vec![Property::new("IsExternal", PropertyValue::Boolean(false))],
                ));
            }
            None => self.lose(reasons::BODY_NOT_EXPORTED, "IFCCOVERING", object),
        }
        self.elements.push(element);
        self.exported += 1;
        self.cuts_of(object, storey, level, derived.z_range());
    }

    /// The vertical openings hosted by `host`, as `IfcOpeningElement`s that void it.
    fn cuts_of(&mut self, host: &Object, storey: ObjectId, level: f64, z: (f64, f64)) {
        for cut in spatial::children(self.snapshot, VERTICAL_OPENING, host.id) {
            let Some(outline) = archbuilder_model::opening::outline_of(
                self.snapshot,
                &cut,
                archbuilder_model::floor::VERTICAL_SLOTS,
            ) else {
                self.lose(reasons::NOT_EXPORTED, "IFCOPENINGELEMENT", &cut);
                continue;
            };
            self.opening_element(&cut, host, storey, level, z, &outline, 0);
        }
    }

    /// One `IfcOpeningElement` named after `object`, voiding `host`, over the plan `outline`
    /// through the elevations `z` (a little beyond them, so the void is clear of the faces).
    #[allow(
        clippy::too_many_arguments,
        reason = "the pieces of one opening element"
    )]
    fn opening_element(
        &mut self,
        object: &Object,
        host: &Object,
        storey: ObjectId,
        level: f64,
        z: (f64, f64),
        outline: &[[f64; 2]],
        index: u32,
    ) {
        const BEYOND: f64 = 0.05;
        let mut opening = Element::new(
            derived_id(object.id.as_u128(), role::INSERT_OPENING, index),
            ElementKind::Opening,
            &name_of(object),
            storey.as_u128(),
        );
        opening.placement = Placement::new(
            Point3::new(0.0, 0.0, z.0 - BEYOND - level),
            Vector3::z(),
            Vector3::x(),
        );
        opening.body = Some(Body::Extrusion {
            outer: profile(outline, true),
            holes: Vec::new(),
            direction: Vector3::z(),
            depth: z.1 - z.0 + 2.0 * BEYOND,
        });
        opening.predefined_type = Some("OPENING".to_owned());
        opening.host = Some(host.id.as_u128());
        self.elements.push(opening);
        self.exported += 1;
    }

    /// A shaft: one `IfcOpeningElement` for each floor, roof or ceiling it passes through, each
    /// voiding its host.
    fn shaft(&mut self, object: &Object, storey: ObjectId, level: f64) {
        let Some(span) = archbuilder_model::opening::shaft_span(self.snapshot, object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCOPENINGELEMENT", object);
        };
        if span.boundary.has_arcs() {
            self.lose(reasons::ARC_FACETED, "IFCOPENINGELEMENT", object);
        }
        let mut hosts: Vec<(std::sync::Arc<Object>, (f64, f64))> = Vec::new();
        for floor in spatial::objects(self.snapshot, FLOOR) {
            if let Some(d) = Derived::of(self.snapshot, &floor) {
                hosts.push((floor, (d.top - d.floor.thickness, d.top)));
            }
        }
        for roof in spatial::objects(self.snapshot, ROOF) {
            if let Ok(d) = archbuilder_model::roof::Derived::of(self.snapshot, &roof) {
                hosts.push((roof, (d.surface.base - d.surface.drop, d.surface.top())));
            }
        }
        for ceiling in spatial::objects(self.snapshot, CEILING) {
            if let Some(d) = archbuilder_model::ceiling::derived(self.snapshot, &ceiling) {
                hosts.push((ceiling, d.z_range()));
            }
        }
        hosts.sort_by_key(|(o, _)| o.id);
        let mut index = 0;
        for (host, z) in hosts {
            if span.z.1 <= z.0 + 1.0e-6 || span.z.0 >= z.1 - 1.0e-6 {
                continue;
            }
            let outline = archbuilder_model::opening::host_outline(self.snapshot, &host);
            if !outline
                .as_ref()
                .is_some_and(|o| archbuilder_model::stair::outlines_overlap(o, &span.outline))
            {
                continue;
            }
            // A shaft that covers the whole host cuts nothing in the model (the host would
            // vanish), so it is no void of it in the file either.
            let footprint =
                archbuilder_model::floor::Boundary::polygons(vec![span.outline.clone()]);
            if outline
                .as_ref()
                .is_some_and(|o| o.iter().all(|p| footprint.contains(*p)))
            {
                self.lose(reasons::CUT_NOT_APPLIED, "IFCOPENINGELEMENT", object);
                continue;
            }
            index += 1;
            self.opening_element(object, &host, storey, level, z, &span.outline, index);
        }
    }

    /// A roof as an `IfcRoof` with its slab as a faceted B-rep in the storey's coordinates, its
    /// type's layers on the type, and its projected and sloped areas. The predefined type names
    /// the form: a hip roof when every edge slopes, a gable roof for four edges of which two are
    /// gables, a shed roof for one sloped edge, else a free form.
    fn roof(&mut self, object: &Object, storey: ObjectId, level: f64) {
        let Ok(derived) = archbuilder_model::roof::Derived::of(self.snapshot, object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCROOF", object);
        };
        let mut element = Element::new(
            object.id.as_u128(),
            ElementKind::Roof,
            &name_of(object),
            storey.as_u128(),
        );
        let edges = &derived.roof.edges;
        let sloped = edges.iter().filter(|e| e.slope.is_some()).count();
        element.predefined_type = Some(
            match (edges.len(), sloped) {
                (n, s) if n == s => "HIP_ROOF",
                (4, 2) => "GABLE_ROOF",
                (_, 1) => "SHED_ROOF",
                _ => "FREEFORM",
            }
            .to_owned(),
        );
        element.element_type =
            et::type_of(self.snapshot, TypeKind::Roof, &object.values).map(|t| t.id.as_u128());
        let surface = &derived.surface;
        let origin = Frame {
            origin: [0.0, 0.0],
            u: [1.0, 0.0],
        };
        let cut = derived.solid_cut().ok();
        if cut.as_ref().is_some_and(|(_, applied)| !applied) {
            self.lose(reasons::CUT_NOT_APPLIED, "IFCROOF", object);
        }
        match cut.and_then(|(s, _)| brep([&s], &origin, level)) {
            Some(body) => {
                element.body = Some(body);
                let sloped_area: f64 = surface
                    .pieces()
                    .iter()
                    .map(|p| {
                        let g = p.plane.grad;
                        archbuilder_model::terrain::double_area(&p.points).abs() / 2.0
                            * (1.0 + g[0] * g[0] + g[1] * g[1]).sqrt()
                    })
                    .sum();
                element.quantity_sets.push(QuantitySet::new(
                    "Qto_RoofBaseQuantities",
                    vec![
                        Quantity::new("GrossArea", QuantityValue::Area(sloped_area)),
                        Quantity::new("ProjectedArea", QuantityValue::Area(surface.plan_area())),
                    ],
                ));
                element.property_sets.push(PropertySet::new(
                    "Pset_RoofCommon",
                    vec![Property::new("IsExternal", PropertyValue::Boolean(true))],
                ));
            }
            None => self.lose(reasons::BODY_NOT_EXPORTED, "IFCROOF", object),
        }
        self.elements.push(element);
        self.exported += 1;
        self.cuts_of(
            object,
            storey,
            level,
            (surface.base - surface.drop, surface.top()),
        );
    }

    fn floor(&mut self, object: &Object, storey: ObjectId, level: f64) {
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
        if let Some(found) = et::type_of(self.snapshot, TypeKind::Floor, &object.values) {
            element.element_type = Some(found.id.as_u128());
            // The layers run from the top, the placement's plane, downward.
            element.layer_usage = Some(LayerSetUsage::new(LayerDirection::Axis3, false, 0.0));
        }
        // The slab cut back to the cores of the walls it meets, arcs written as chords within
        // the model's flattening tolerance; the quantities are those of the authored boundary.
        let parts = cleanup::clean_floor(self.snapshot, object)
            .map(|c| cleanup::flatten_parts(&c.region))
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| {
                let flat = derived
                    .boundary
                    .flattened(archbuilder_model::floor::FLAT_CHORD);
                flat.split_first()
                    .map(|(o, h)| vec![(o.clone(), h.to_vec())])
                    .unwrap_or_default()
            });
        let slab = match parts.as_slice() {
            [(outer, holes)] => Some(Body::Extrusion {
                outer: profile(outer, true),
                holes: holes.iter().map(|h| profile(h, false)).collect(),
                direction: -Vector3::z(),
                depth: thickness,
            }),
            _ => cleanup::floor_solid(self.snapshot, object)
                .and_then(Result::ok)
                .and_then(|solid| {
                    brep(
                        [&solid],
                        &Frame {
                            origin: [0.0, 0.0],
                            u: [1.0, 0.0],
                        },
                        derived.top,
                    )
                }),
        };
        if let Some(body) = slab {
            element.body = Some(body);
            let gross = signed_area(
                &derived
                    .boundary
                    .faceted(archbuilder_model::floor::FLAT_CHORD)
                    .loops
                    .first()
                    .cloned()
                    .unwrap_or_default(),
            )
            .abs();
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
        self.cuts_of(
            object,
            storey,
            level,
            (derived.top - thickness, derived.top),
        );
    }
}

/// The IFC4 operation type of a door (`IfcDoorTypeOperationEnum`) or partitioning of a window
/// (`IfcWindowTypePartitioningEnum`): a double door swings two leaves, a sliding door slides
/// toward the side opposite its hinge, a sliding window has two panels side by side. The
/// placement's y axis points to the swing side. The product
/// reads the hand facing the leaf from that side, IFC4 along the y axis: opposite viewpoints, so
/// the product's left (hinge at +x) is `SINGLE_SWING_RIGHT` (IFC left has its hinge at -x). Fixed and casement windows are both one panel; IFC4 has
/// no operation attribute on the window itself, so a casement's is reported as approximated.
fn operation(item: &Insert) -> &'static str {
    use insert::Operation;
    match (item.kind, item.operation, item.hand) {
        (Kind::Door, Operation::DoubleSwing, _) => "DOUBLE_DOOR_SINGLE_SWING",
        (Kind::Door, Operation::Sliding, Hand::Left) => "SLIDING_TO_RIGHT",
        (Kind::Door, Operation::Sliding, Hand::Right) => "SLIDING_TO_LEFT",
        (Kind::Door, _, Hand::Left) => "SINGLE_SWING_RIGHT",
        (Kind::Door, _, Hand::Right) => "SINGLE_SWING_LEFT",
        (Kind::Window, Operation::Sliding, _) => "DOUBLE_PANEL_VERTICAL",
        (Kind::Window | Kind::Opening, _, _) => "SINGLE_PANEL",
    }
}

/// The element types of the document: wall and floor types with their layers, door and window
/// types with their operation. A wall type is `PARTITIONING` when it is interior and carries no
/// load, else `STANDARD`. A door type's hand is its occurrences', so a type with one swinging
/// or sliding leaf names its operation as a user-defined value.
fn element_types(snapshot: &Snapshot) -> Vec<ElementType> {
    use insert::Operation;
    let mut out = Vec::new();
    for kind in TypeKind::ALL {
        for object in et::of_kind(snapshot, kind) {
            // A curtain wall type has no wall in the file to be the type of.
            if kind == TypeKind::Wall
                && archbuilder_model::curtain::CurtainType::from_values(&object.values).is_some()
            {
                continue;
            }
            let (ifc_kind, predefined) = match kind {
                TypeKind::Wall => (ElementKind::Wall, "STANDARD"),
                TypeKind::Floor => (ElementKind::Slab, "FLOOR"),
                TypeKind::Door => (ElementKind::Door, "DOOR"),
                TypeKind::Window => (ElementKind::Window, "WINDOW"),
                // A roof type does not decide the roof's form; its occurrences name it.
                TypeKind::Roof => (ElementKind::Roof, ""),
                TypeKind::Ceiling => (ElementKind::Covering, "CEILING"),
            };
            let mut t = ElementType::new(object.id.as_u128(), ifc_kind, et::name(&object));
            t.predefined_type = (!predefined.is_empty()).then(|| predefined.to_owned());
            if let Some(wall) =
                WallType::from_values(&object.values).filter(|_| kind == TypeKind::Wall)
                && wall.exposure == Exposure::Interior
                && !wall.load_bearing
            {
                t.predefined_type = Some("PARTITIONING".to_owned());
            }
            if kind.layered() {
                t.layers = Layers::from_values(&object.values)
                    .map(|l| {
                        l.layers
                            .iter()
                            .map(|layer| {
                                let mut m = MaterialLayer::new(&layer.material, layer.thickness);
                                m.category = match layer.function {
                                    Function::Structure => Some("LoadBearing".to_owned()),
                                    Function::Thermal => Some("Insulation".to_owned()),
                                    Function::Finish => Some("Finish".to_owned()),
                                    Function::Substrate | Function::Membrane => None,
                                };
                                m
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            } else if let Some(i) = InsertType::from_values(&object.values) {
                t.operation_type = Some(
                    match (kind, i.operation) {
                        (TypeKind::Door, Operation::DoubleSwing) => "DOUBLE_DOOR_SINGLE_SWING",
                        (TypeKind::Door, Operation::Sliding) => "SLIDING",
                        (TypeKind::Door, _) => "SINGLE_SWING",
                        (_, Operation::Sliding) => "DOUBLE_PANEL_VERTICAL",
                        _ => "SINGLE_PANEL",
                    }
                    .to_owned(),
                );
            }
            out.push(t);
        }
    }
    out
}

/// The solids of an insert as one faceted B-rep in the insert's frame; `z0` is the elevation of
/// the frame's origin. `None` when a face has holes, which a B-rep of loops cannot hold.
fn brep<'a>(
    solids: impl IntoIterator<Item = &'a arc_geom::Polyhedron>,
    frame: &Frame,
    z0: f64,
) -> Option<Body> {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for solid in solids {
        let first = vertices.len();
        vertices.extend(solid.vertices().iter().map(|v| {
            let [x, y] = frame.local([v.x, v.y]);
            Point3::new(x, y, v.z - z0)
        }));
        for (k, face) in solid.faces().iter().enumerate() {
            let offset = |i: u32| first + usize::try_from(i).unwrap_or(usize::MAX);
            if face.holes.is_empty() {
                faces.push(face.outer.iter().map(|&i| offset(i)).collect());
            } else {
                // A loop of vertices holds no hole: such a face is written as its triangles.
                for t in solid
                    .triangulate_face(k, &arc_base::Tolerance::DEFAULT)
                    .ok()?
                {
                    faces.push(t.iter().map(|&i| offset(i)).collect());
                }
            }
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
    model.types = element_types(snapshot);
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
        arguments.insert("count".to_owned(), Argument::Int(loss.count));
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
