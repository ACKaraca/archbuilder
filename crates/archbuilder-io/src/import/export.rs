//! The export of what an import kept (AB-IO-018): the property sets, quantity sets and
//! classification of every imported object and the foreign elements themselves.
//!
//! The writer of the foundation has no entity for an arbitrary class, so a foreign element is
//! written as a stand-in member with its mesh as a faceted B-rep and then rewritten, through the
//! edit API of the reader, as a record of its source class: the `GlobalId`, name, placement,
//! representation and tag are kept as the writer made them, the predefined type and object type
//! are the source's, every other attribute is omitted. A class the output schema does not have is
//! written as a building element proxy and reported; the stand-in is never left in the file.
//!
//! Property values of the source are written back with the types the writer has; a value it cannot
//! write (a list, a bounded or a table value, a type it has no measure for) is reported, never
//! dropped silently.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use arc_base::id::Identifier;
use arc_base::math::{Point3, Vector3};
use arc_base::{Cancel, Limits, ObjectId};
use arc_doc::{Snapshot, Value};
use arc_io_common::loss::{Argument, LossItem, Reason};
use arc_io_ifc::schema::{Named, TypeRef};
use arc_io_ifc::write::{
    Body, Element, ElementKind, Measure, Placement, Property, PropertySet, PropertyValue, Quantity,
    QuantitySet, QuantityValue, global_id,
};
use arc_io_ifc::{IfcFile, Parameter, ReadOptions, Record, WriteModel, read};
use arc_mesh::TriMesh;
use archbuilder_model::spatial;
use archbuilder_model::types::{FOREIGN_ELEMENT, foreign_element};

use super::data::{FORMAT, KIND_ELEMENT, Kept, Nom, PropKind, Pset, Qto};
use crate::error::reasons;

/// The data of the foreign rows of a snapshot and the foreign elements being written.
pub(crate) struct Hooks {
    kept: HashMap<ObjectId, Kept>,
    foreign: Vec<Out>,
    losses: Vec<LossItem>,
}

/// A foreign element written as a stand-in.
struct Out {
    gid: String,
    class: String,
    predefined: Option<String>,
    object_type: Option<String>,
    description: Option<String>,
}

const MEASURES: [Measure; 25] = [
    Measure::Length,
    Measure::PositiveLength,
    Measure::NonNegativeLength,
    Measure::Area,
    Measure::Volume,
    Measure::PlaneAngle,
    Measure::PositivePlaneAngle,
    Measure::Ratio,
    Measure::PositiveRatio,
    Measure::NormalisedRatio,
    Measure::Count,
    Measure::ThermalTransmittance,
    Measure::ThermodynamicTemperature,
    Measure::Power,
    Measure::Pressure,
    Measure::VolumetricFlowRate,
    Measure::Mass,
    Measure::MassDensity,
    Measure::AreaDensity,
    Measure::Time,
    Measure::Illuminance,
    Measure::LinearVelocity,
    Measure::HeatFluxDensity,
    Measure::PlanarForce,
    Measure::Monetary,
];

fn item(reason: Reason, kind: &str, id: Option<ObjectId>, count: u64) -> LossItem {
    let mut arguments = BTreeMap::new();
    arguments.insert(
        "count".to_owned(),
        Argument::Int(i64::try_from(count).unwrap_or(i64::MAX)),
    );
    LossItem {
        outcome: reason.outcome,
        severity: reason.severity,
        source_id: None,
        record_kind: Some(kind.to_owned()),
        object_id: id.map(|id| id.to_string()),
        reason: reason.code.to_owned(),
        message_id: reason.message_id.to_owned(),
        arguments,
    }
}

/// The property value the writer has for one source value, or `None`.
fn value_of(n: &Nom) -> Option<(PropertyValue, bool)> {
    let ty = n.ty.as_deref().unwrap_or("");
    let exact = |v: PropertyValue| Some((v, true));
    if let Some(e) = &n.enumeration {
        return match (ty, e.as_str()) {
            ("IFCLOGICAL", "U") => exact(PropertyValue::Logical(None)),
            ("IFCLOGICAL", "T") => exact(PropertyValue::Logical(Some(true))),
            ("IFCLOGICAL", "F") => exact(PropertyValue::Logical(Some(false))),
            (_, "T") => exact(PropertyValue::Boolean(true)),
            (_, "F") => exact(PropertyValue::Boolean(false)),
            _ => None,
        };
    }
    if let Some(t) = &n.text {
        return match ty {
            "IFCTEXT" => exact(PropertyValue::Text(t.clone())),
            "IFCIDENTIFIER" if t.chars().count() <= 255 => {
                exact(PropertyValue::Identifier(t.clone()))
            }
            "IFCLABEL" | "" if t.chars().count() <= 255 => exact(PropertyValue::Label(t.clone())),
            // A text the label cannot hold, or a text of another defined type.
            _ => Some((
                PropertyValue::Text(t.clone()),
                ty == "IFCLABEL" || ty.is_empty(),
            )),
        };
    }
    if let Some(i) = n.int {
        #[allow(clippy::cast_precision_loss, reason = "a measure")]
        return match ty {
            "IFCINTEGER" | "" => exact(PropertyValue::Integer(i)),
            "IFCCOUNTMEASURE" => exact(PropertyValue::Measure(Measure::Count, i as f64)),
            _ => Some((PropertyValue::Integer(i), false)),
        };
    }
    let r = n.real?;
    if !r.is_finite() {
        return None;
    }
    Some(match ty {
        "IFCREAL" | "" => (PropertyValue::Real(r), true),
        "IFCLENGTHMEASURE" => (PropertyValue::Length(r), true),
        "IFCAREAMEASURE" => (PropertyValue::Area(r), true),
        "IFCVOLUMEMEASURE" => (PropertyValue::Volume(r), true),
        other => match MEASURES.iter().find(|m| m.type_name() == other) {
            Some(m) if m.accepts(r) => (PropertyValue::Measure(*m, r), true),
            _ => (PropertyValue::Real(r), false),
        },
    })
}

/// The property set of the writer for `set`, and the number of properties it could not carry or
/// carried with another type.
fn writer_pset(set: &Pset) -> (PropertySet, u64, u64) {
    let (mut dropped, mut approximated) = (0, 0);
    let mut properties: Vec<Property> = Vec::new();
    for p in &set.props {
        if properties.iter().any(|q| q.name == p.name) || p.name.is_empty() {
            dropped += 1;
            continue;
        }
        let value = match p.kind {
            PropKind::Single => p.values.first().and_then(value_of),
            PropKind::Enumerated => {
                let items: Vec<String> = p
                    .values
                    .iter()
                    .filter_map(|v| v.text.clone().or_else(|| v.enumeration.clone()))
                    .collect();
                let mut unique = items.clone();
                unique.sort();
                unique.dedup();
                (!items.is_empty()).then(|| {
                    (
                        PropertyValue::Enumerated {
                            enumeration: format!("PEnum_{}", p.name),
                            items: unique,
                            values: items,
                        },
                        true,
                    )
                })
            }
            // Lists and bounded values have no value in the writer.
            PropKind::List | PropKind::Bounded => None,
        };
        match value {
            Some((v, exact)) => {
                if !exact {
                    approximated += 1;
                }
                properties.push(Property::new(&p.name, v));
            }
            None => dropped += 1,
        }
    }
    (
        PropertySet::new(&set.name, properties),
        dropped,
        approximated,
    )
}

fn writer_qto(set: &Qto) -> QuantitySet {
    QuantitySet::new(
        &set.name,
        set.quantities
            .iter()
            .filter(|q| q.value.is_finite() && q.value >= 0.0)
            .map(|q| {
                Quantity::new(
                    &q.name,
                    match q.kind.as_str() {
                        "length" => QuantityValue::Length(q.value),
                        "area" => QuantityValue::Area(q.value),
                        "volume" => QuantityValue::Volume(q.value),
                        "weight" => QuantityValue::Weight(q.value),
                        "time" => QuantityValue::Time(q.value),
                        _ => QuantityValue::Count(q.value),
                    },
                )
            })
            .collect(),
    )
}

fn merge_psets(into: &mut Vec<PropertySet>, set: PropertySet) {
    if set.properties.is_empty() {
        return;
    }
    match into.iter_mut().find(|s| s.name == set.name) {
        Some(existing) => {
            for p in set.properties {
                if !existing.properties.iter().any(|q| q.name == p.name) {
                    existing.properties.push(p);
                }
            }
        }
        None => into.push(set),
    }
}

fn merge_qtos(into: &mut Vec<QuantitySet>, set: QuantitySet) {
    if set.quantities.is_empty() || into.iter().any(|s| s.name == set.name) {
        return;
    }
    into.push(set);
}

impl Hooks {
    /// The kept data of the objects of `snapshot`.
    pub(crate) fn of(snapshot: &Snapshot) -> Self {
        let mut kept = HashMap::new();
        for row in snapshot.foreign_rows() {
            if row.source_format != FORMAT || row.record_kind != KIND_ELEMENT {
                continue;
            }
            let (Some(bytes), Some(data)) = (
                row.object
                    .as_deref()
                    .and_then(|b| <[u8; 16]>::try_from(b).ok()),
                Kept::from_bytes(&row.data),
            ) else {
                continue;
            };
            kept.insert(ObjectId::from_u128(u128::from_be_bytes(bytes)), data);
        }
        Self {
            kept,
            foreign: Vec::new(),
            losses: Vec::new(),
        }
    }

    /// The stand-ins of the foreign elements of `storey`, in the coordinates of the storey.
    pub(crate) fn elements(
        &mut self,
        snapshot: &Snapshot,
        storey: ObjectId,
        level: f64,
        limits: &Limits,
        present: &BTreeSet<u128>,
    ) -> (Vec<Element>, BTreeSet<ObjectId>) {
        let mut out = Vec::new();
        let mut rooms = BTreeSet::new();
        for room in spatial::objects(snapshot, archbuilder_model::types::ROOM) {
            if room.header.owner != Some(storey)
                || present.contains(&room.id.as_u128())
                || !self.kept.contains_key(&room.id)
            {
                continue;
            }
            let number = archbuilder_model::types::text(
                &room.values,
                archbuilder_model::types::room::NUMBER,
            )
            .filter(|n| !n.is_empty());
            let room_name =
                archbuilder_model::types::text(&room.values, archbuilder_model::types::room::NAME)
                    .unwrap_or("Room");
            let (name, long_name) = match number {
                Some(n) => (n.to_owned(), Some(room_name.to_owned())),
                None => (room_name.to_owned(), None),
            };
            let mut element = Element::new(
                room.id.as_u128(),
                ElementKind::Space,
                &name,
                storey.as_u128(),
            );
            element.long_name = long_name;
            let base = archbuilder_model::types::meters(
                &room.values,
                archbuilder_model::types::room::BASE_OFFSET,
            )
            .unwrap_or(0.0);
            element.placement = Placement::new(
                Point3::new(0.0, 0.0, base),
                Vector3::z(),
                Vector3::new(1.0, 0.0, 0.0),
            );
            self.losses.push(item(
                reasons::ROOM_BODY_MISSING,
                "IFCSPACE",
                Some(room.id),
                1,
            ));
            rooms.insert(room.id);
            out.push(element);
        }
        for object in spatial::objects(snapshot, FOREIGN_ELEMENT) {
            if object.header.owner != Some(storey) {
                continue;
            }
            let name = archbuilder_model::types::text(&object.values, foreign_element::NAME)
                .unwrap_or("Element")
                .to_owned();
            let class = archbuilder_model::types::text(&object.values, foreign_element::CLASS)
                .unwrap_or("IfcBuildingElementProxy")
                .to_ascii_uppercase();
            let text = |at| {
                archbuilder_model::types::text(&object.values, at)
                    .map(str::to_owned)
                    .filter(|t| !t.is_empty())
            };
            let mut element = Element::new(
                object.id.as_u128(),
                ElementKind::Member,
                &name,
                storey.as_u128(),
            );
            element.placement = Placement::new(
                Point3::new(0.0, 0.0, 0.0),
                Vector3::z(),
                Vector3::new(1.0, 0.0, 0.0),
            );
            if let Some(Some(Value::Geometry(bytes))) = object.values.get(foreign_element::MESH) {
                match TriMesh::decode(&bytes.0, limits) {
                    Ok(mesh) => element.body = brep(&mesh, level),
                    Err(_) => self.losses.push(item(
                        reasons::BODY_NOT_EXPORTED,
                        "IFCBUILDINGELEMENTPROXY",
                        Some(object.id),
                        1,
                    )),
                }
            }
            let description = self
                .kept
                .get(&object.id)
                .and_then(|k| k.description.clone());
            self.foreign.push(Out {
                gid: global_id(object.id.as_u128()),
                class,
                predefined: text(foreign_element::PREDEFINED_TYPE),
                object_type: text(foreign_element::OBJECT_TYPE),
                description,
            });
            out.push(element);
        }
        (out, rooms)
    }

    /// Adds the kept property sets, quantity sets of every imported object to the model.
    pub(crate) fn augment(&mut self, model: &mut WriteModel) {
        let kept = &self.kept;
        let mut dropped = 0;
        let mut approximated = 0;
        let mut add = |id: u128, props: &mut Vec<PropertySet>, qtos: &mut Vec<QuantitySet>| {
            let Some(k) = kept.get(&ObjectId::from_u128(id)) else {
                return;
            };
            for set in &k.psets {
                let (w, d, a) = writer_pset(set);
                dropped += d;
                approximated += a;
                merge_psets(props, w);
            }
            for set in &k.qtos {
                merge_qtos(qtos, writer_qto(set));
            }
        };
        let (p, s, b) = (model.project.id, model.site.id, model.building.id);
        for (id, spatial) in [
            (p, &mut model.project),
            (s, &mut model.site),
            (b, &mut model.building),
        ] {
            add(id, &mut spatial.property_sets, &mut spatial.quantity_sets);
        }
        for storey in &mut model.storeys {
            add(
                storey.id,
                &mut storey.property_sets,
                &mut storey.quantity_sets,
            );
        }
        for element in &mut model.elements {
            add(
                element.id,
                &mut element.property_sets,
                &mut element.quantity_sets,
            );
        }
        if dropped > 0 {
            self.losses.push(item(
                reasons::FOREIGN_PROPERTY_NOT_WRITTEN,
                "IFCPROPERTYSET",
                None,
                dropped,
            ));
        }
        if approximated > 0 {
            self.losses.push(item(
                reasons::FOREIGN_PROPERTY_RETYPED,
                "IFCPROPERTYSET",
                None,
                approximated,
            ));
        }
    }

    /// The `GlobalId`s of the foreign elements written.
    pub(crate) fn foreign_ids(&self) -> BTreeSet<String> {
        self.foreign.iter().map(|f| f.gid.clone()).collect()
    }

    /// Rewrites each stand-in as a record of its source class.
    pub(crate) fn finish(&mut self, bytes: Vec<u8>, limits: &Limits, cancel: &Cancel) -> Vec<u8> {
        if self.foreign.is_empty() {
            return bytes;
        }
        let mut options = ReadOptions::with_limits(*limits);
        options.cancel = cancel.clone();
        let Ok(output) = read(&bytes, &mut options) else {
            return bytes;
        };
        let mut file = output.file;
        let mut by_gid: HashMap<String, u64> = HashMap::new();
        for instance in file.instances() {
            if let Some(Record {
                type_name,
                parameters,
            }) = instance.records.first()
                && type_name == "IFCMEMBER"
                && let Some(Parameter::Text(g)) = parameters.first()
            {
                by_gid.insert(g.clone(), instance.name);
            }
        }
        let mut changed = false;
        for out in std::mem::take(&mut self.foreign) {
            let Some(name) = by_gid.get(&out.gid).copied() else {
                continue;
            };
            let Some(record) = file.instance(name).and_then(|i| i.records.first()).cloned() else {
                continue;
            };
            let (class, proxied) = target_class(&file, &out.class);
            if proxied {
                self.losses
                    .push(item(reasons::FOREIGN_CLASS_CHANGED, &out.class, None, 1));
            }
            let parameters = class_parameters(&file, &record, &class, &out);
            if file
                .replace(
                    name,
                    vec![Record {
                        type_name: class,
                        parameters,
                    }],
                )
                .is_ok()
            {
                changed = true;
            }
        }
        if !changed {
            return bytes;
        }
        file.write_step(limits).unwrap_or(bytes)
    }

    /// The losses found while preparing and finishing.
    pub(crate) fn take_losses(&mut self) -> Vec<LossItem> {
        std::mem::take(&mut self.losses)
    }
}

/// The class a foreign element is written as, in upper case, and whether it is the proxy standing
/// in for a class the output schema does not have.
fn target_class(file: &IfcFile, wanted: &str) -> (String, bool) {
    let proxy = "IFCBUILDINGELEMENTPROXY".to_owned();
    let Some(tables) = file.tables() else {
        return (wanted.to_owned(), false);
    };
    match (
        tables.entity_index(wanted),
        tables.entity_index("IFCELEMENT"),
    ) {
        (Some(entity), Some(element)) if tables.is_subtype_of(entity, element) => {
            let abstract_class = tables.entity(wanted).is_some_and(|e| e.is_abstract);
            if abstract_class {
                (proxy, true)
            } else {
                (wanted.to_owned(), false)
            }
        }
        _ => (proxy, true),
    }
}

/// The parameters of the record of `class`: the first eight of the stand-in (the root, object
/// type, placement, representation and tag), the predefined type when the class has one that
/// accepts the source's, and nothing for every other attribute.
fn class_parameters(file: &IfcFile, stand_in: &Record, class: &str, out: &Out) -> Vec<Parameter> {
    let mut parameters: Vec<Parameter> = stand_in.parameters.iter().take(8).cloned().collect();
    if let Some(object_type) = &out.object_type
        && let Some(slot) = parameters.get_mut(4)
    {
        *slot = Parameter::Text(object_type.clone());
    }
    if let Some(description) = &out.description
        && let Some(slot) = parameters.get_mut(3)
    {
        *slot = Parameter::Text(description.clone());
    }
    let Some(tables) = file.tables() else {
        return parameters;
    };
    let Some(entity) = tables.entity_index(class) else {
        return parameters;
    };
    for position in tables.positions(entity).iter().skip(8) {
        let value = if position.derived {
            Parameter::Derived
        } else if position
            .attribute
            .name
            .eq_ignore_ascii_case("PredefinedType")
        {
            predefined(
                tables,
                position.attribute.ty,
                out.predefined.as_deref(),
                out.object_type.is_some(),
            )
        } else {
            Parameter::Omitted
        };
        parameters.push(value);
    }
    parameters
}

fn predefined(
    tables: &arc_io_ifc::schema::SchemaTables,
    ty: TypeRef,
    wanted: Option<&str>,
    has_user_type: bool,
) -> Parameter {
    let Some(wanted) = wanted else {
        return Parameter::Omitted;
    };
    let TypeRef::Named(Named::Enumeration(at)) = ty else {
        return Parameter::Omitted;
    };
    let Some(enumeration) = tables.enumerations.get(usize::from(at)) else {
        return Parameter::Omitted;
    };
    let upper = wanted.to_ascii_uppercase();
    let known = enumeration
        .items
        .iter()
        .any(|i| i.eq_ignore_ascii_case(&upper));
    // `USERDEFINED` needs the object type text, which the stand-in carries when the source had one.
    if known && (upper != "USERDEFINED" || has_user_type) {
        Parameter::Enumeration(upper)
    } else {
        Parameter::Omitted
    }
}

/// The faceted B-rep of a mesh with its heights relative to `level`.
fn brep(mesh: &TriMesh, level: f64) -> Option<Body> {
    let vertices: Vec<Point3> = mesh
        .positions()
        .iter()
        .map(|p| Point3::new(p.x, p.y, p.z - level))
        .collect();
    let faces: Vec<Vec<usize>> = mesh
        .triangles()
        .iter()
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .map(|t| t.iter().map(|&i| i as usize).collect())
        .collect();
    (!faces.is_empty()).then_some(Body::Brep { vertices, faces })
}
