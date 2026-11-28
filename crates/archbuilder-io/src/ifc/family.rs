//! Family instances in the IFC export (ADR 0009, section 4): the entity of each category (the
//! table of the ADR, overridden by the family's own IFC assignment), the type object of the
//! family type with its property sets `ArchBuilder_Type` and `ArchBuilder_Family`, and the type's
//! shared geometry as an `IfcRepresentationMap`, which each occurrence with the same body maps
//! through an `IfcMappedItem`. An occurrence has its `ArchBuilder_Instance` set, the standard sets
//! where the category maps, and its own faceted body only where its geometry differs from the
//! type's. Identifiers are the model's own, so every `GlobalId` is the same after every export.
//!
//! A generic model or specialty equipment has no entity of its own: it is written only as the
//! entity its family declares, and otherwise listed in the loss report, never as a proxy.

use std::collections::BTreeMap;

use arc_base::ObjectId;
use arc_base::id::Identifier;
use arc_doc::Object;
use arc_io_ifc::write::{
    Body, Element, ElementKind, ElementType, Measure, Placement, Property, PropertySet,
    PropertyValue, Quantity, QuantityValue,
};
use archbuilder_model::expr::{DataType, Evaluated};
use archbuilder_model::family::def::{Category, Definition, Header, ParamDef, Scope};
use archbuilder_model::family::loaded::{self, Component, Resolved};
use archbuilder_model::wall::Wall;

use super::{Frame, Mapper, brep, name_of, psets};
use crate::error::reasons;

/// The IFC entity of a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Entity {
    /// `IfcFurniture`.
    Furniture,
    /// `IfcSanitaryTerminal`.
    SanitaryTerminal,
    /// `IfcLightFixture`.
    LightFixture,
    /// `IfcElectricAppliance`.
    ElectricAppliance,
    /// `IfcUnitaryEquipment`.
    UnitaryEquipment,
    /// `IfcDoor`.
    Door,
    /// `IfcWindow`.
    Window,
    /// `IfcColumn`.
    Column,
    /// `IfcBeam`; only by a family's own assignment, no category defaults to it.
    Beam,
    /// `IfcBuildingElementProxy`; only by a family's own assignment, never a default.
    Proxy,
}

impl Entity {
    const ALL: [Self; 10] = [
        Self::Furniture,
        Self::SanitaryTerminal,
        Self::LightFixture,
        Self::ElectricAppliance,
        Self::UnitaryEquipment,
        Self::Door,
        Self::Window,
        Self::Column,
        Self::Beam,
        Self::Proxy,
    ];

    /// The entity name in upper case.
    pub(super) const fn class(self) -> &'static str {
        match self {
            Self::Furniture => "IFCFURNITURE",
            Self::SanitaryTerminal => "IFCSANITARYTERMINAL",
            Self::LightFixture => "IFCLIGHTFIXTURE",
            Self::ElectricAppliance => "IFCELECTRICAPPLIANCE",
            Self::UnitaryEquipment => "IFCUNITARYEQUIPMENT",
            Self::Door => "IFCDOOR",
            Self::Window => "IFCWINDOW",
            Self::Column => "IFCCOLUMN",
            Self::Beam => "IFCBEAM",
            Self::Proxy => "IFCBUILDINGELEMENTPROXY",
        }
    }

    /// The element kind of the writer that writes this entity.
    pub(super) const fn kind(self) -> ElementKind {
        match self {
            Self::Furniture => ElementKind::Furniture,
            Self::SanitaryTerminal => ElementKind::SanitaryTerminal,
            Self::LightFixture => ElementKind::LightFixture,
            Self::ElectricAppliance => ElementKind::ElectricAppliance,
            Self::UnitaryEquipment => ElementKind::UnitaryEquipment,
            Self::Door => ElementKind::Door,
            Self::Window => ElementKind::Window,
            Self::Column => ElementKind::Column,
            Self::Beam => ElementKind::Beam,
            Self::Proxy => ElementKind::BuildingElementProxy,
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let upper = text.trim().to_ascii_uppercase();
        Self::ALL.into_iter().find(|e| e.class() == upper)
    }
}

/// The IFC assignment of a family: the entity and the predefined type (a value the entity's
/// enumeration does not define is written as a user-defined type with that text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Assignment {
    /// The entity.
    pub(super) entity: Entity,
    /// The predefined type; `None` is `NOTDEFINED`.
    pub(super) predefined: Option<String>,
}

/// The default entity of a category (the table of ADR 0009, section 4) and its predefined type.
/// A profile, a generic model and specialty equipment have none: their family must name one.
pub(super) fn default_of(category: Category) -> Option<(Entity, Option<&'static str>)> {
    Some(match category {
        Category::Profile | Category::GenericModel | Category::SpecialtyEquipment => return None,
        Category::Furniture => (Entity::Furniture, None),
        // Casework has no furniture type of its own: a user-defined type with the text.
        Category::Casework => (Entity::Furniture, Some("CASEWORK")),
        Category::PlumbingFixture => (Entity::SanitaryTerminal, None),
        Category::LightingFixture => (Entity::LightFixture, None),
        Category::ElectricalEquipment => (Entity::ElectricAppliance, None),
        Category::MechanicalEquipment => (Entity::UnitaryEquipment, None),
        Category::Door => (Entity::Door, None),
        Category::Window => (Entity::Window, None),
        Category::Column => (Entity::Column, None),
    })
}

/// The assignment of a header: its own entity and type when it names them, else the default of
/// its category.
///
/// # Errors
/// The entity text of an assignment that names an entity this mapping does not know, or the
/// class a family without any entity would need (a generic model that names none).
pub(super) fn assignment(header: &Header) -> Result<Assignment, String> {
    let own = header.ifc_entity.trim();
    let (entity, predefined) = if own.is_empty() {
        match default_of(header.category) {
            Some((entity, predefined)) => (entity, predefined.map(str::to_owned)),
            None => return Err(Entity::Proxy.class().to_owned()),
        }
    } else {
        (Entity::parse(own).ok_or_else(|| own.to_owned())?, None)
    };
    let own_type = header.ifc_type.trim();
    let predefined = if own_type.is_empty() {
        predefined
    } else {
        Some(own_type.to_owned())
    };
    Ok(Assignment { entity, predefined })
}

/// The IFC value of a parameter value of `data`; `None` for an empty text. The second part is
/// whether the value is written as text because IFC has no type for it.
fn value_of(data: DataType, value: &Evaluated) -> Option<(PropertyValue, bool)> {
    let single = |v| Some((v, false));
    match (data, value) {
        (DataType::Length, Evaluated::Number(v)) => single(PropertyValue::Length(*v)),
        (DataType::Area, Evaluated::Number(v)) => single(PropertyValue::Area(*v)),
        (DataType::Volume, Evaluated::Number(v)) => single(PropertyValue::Volume(*v)),
        (DataType::Angle, Evaluated::Number(v)) => PropertyValue::measure(Measure::PlaneAngle, *v)
            .ok()
            .map(|v| (v, false)),
        (DataType::Slope, Evaluated::Number(v)) => PropertyValue::measure(Measure::Ratio, *v)
            .ok()
            .map(|v| (v, false)),
        (DataType::Number, Evaluated::Number(v)) => {
            PropertyValue::real(*v).ok().map(|v| (v, false))
        }
        // The model holds a whole number as a float; the rounding is exact for whole values.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "an integer parameter is whole"
        )]
        (DataType::Integer, Evaluated::Number(v)) if v.is_finite() => {
            single(PropertyValue::Integer(v.round() as i64))
        }
        (DataType::Boolean, Evaluated::Bool(b)) => single(PropertyValue::Boolean(*b)),
        (_, Evaluated::Text(t)) if !t.is_empty() => {
            let v = PropertyValue::label(t).unwrap_or_else(|_| PropertyValue::Text(t.clone()));
            Some((v, data == DataType::FamilyType))
        }
        _ => None,
    }
}

/// The parameters of `scope`, in the order of the definition, as properties named by their keys,
/// and how many were written as text.
fn parameter_set(
    definition: &Definition,
    values: &BTreeMap<String, Evaluated>,
    scope: Scope,
) -> (Vec<Property>, usize) {
    let mut properties = Vec::new();
    let mut as_text = 0;
    let mut sorted: Vec<&ParamDef> = definition
        .parameters
        .iter()
        .filter(|p| p.scope == scope)
        .collect();
    sorted.sort_by(|a, b| (&a.group, a.order, &a.key).cmp(&(&b.group, b.order, &b.key)));
    for p in sorted {
        if let Some((value, text)) = values.get(&p.key).and_then(|v| value_of(p.data, v)) {
            as_text += usize::from(text);
            properties.push(Property::new(&p.key, value));
        }
    }
    (properties, as_text)
}

/// The set `ArchBuilder_Family`: where the family came from.
fn family_set(resolved: &Resolved) -> PropertySet {
    let header = &resolved.definition.header;
    let mut properties: Vec<Option<Property>> = vec![
        psets::identifier("FamilyId", &resolved.family.family.to_string()),
        psets::identifier("Version", &resolved.family.version.to_string()),
        PropertyValue::label(&resolved.family.name)
            .ok()
            .map(|v| Property::new("Name", v)),
        psets::identifier("Checksum", &resolved.family.checksum),
    ];
    for (name, text) in [("Source", &header.source), ("License", &header.license)] {
        properties.push(
            (!text.is_empty())
                .then(|| PropertyValue::label(text).ok())
                .flatten()
                .map(|v| Property::new(name, v)),
        );
    }
    PropertySet::new(
        "ArchBuilder_Family",
        properties.into_iter().flatten().collect(),
    )
}

/// The solid mirrored across the frame's own y axis (its x changes sign), in the same frame.
fn mirror_x(solid: &arc_geom::Polyhedron) -> Option<arc_geom::Polyhedron> {
    let flip = arc_base::math::Transform3::from_matrix_unchecked(arc_base::math::Matrix4::new(
        -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ));
    solid
        .transformed(
            &flip,
            &arc_base::Tolerance::DEFAULT,
            &arc_base::Limits::DEFAULT,
        )
        .ok()
}

impl Mapper<'_> {
    /// A freestanding family instance of the storey whose level is at `level`. Its body is the
    /// family's own solids in the frame of the instance, so every instance with the same values
    /// has the same body, which the type holds once.
    pub(super) fn component(&mut self, object: &Object, storey: ObjectId) {
        let Some(component) = Component::of(object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCBUILDINGELEMENTPROXY", object);
        };
        let Ok(resolved) = loaded::resolve(self.snapshot, &component) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCBUILDINGELEMENTPROXY", object);
        };
        if resolved.definition.header.category == Category::Profile {
            return self.lose(reasons::NOT_EXPORTED, "IFCPROFILEDEF", object);
        }
        let assigned = match assignment(&resolved.definition.header) {
            Ok(a) => a,
            Err(_) => {
                return self.lose(
                    reasons::FAMILY_NOT_WRITTEN,
                    "IFCBUILDINGELEMENTPROXY",
                    object,
                );
            }
        };
        let class = assigned.entity.class();
        // The solids in the family's own frame: the origin, rotation, mirror and offset are the
        // placement of the element, never part of the body.
        let local = Component {
            storey: None,
            origin: [0.0, 0.0],
            rotation: 0.0,
            offset: 0.0,
            mirror: false,
            ..component.clone()
        };
        let Ok(placed) = loaded::place(self.snapshot, &local) else {
            return self.lose(reasons::BODY_NOT_EXPORTED, class, object);
        };
        let (s, c) = component.rotation.sin_cos();
        let Some(frame) = Frame::new(component.origin, [c, s]) else {
            return self.lose(reasons::NOT_EXPORTED, class, object);
        };
        let body = if component.mirror {
            let mut mirrored = Vec::with_capacity(placed.solids.len());
            for solid in &placed.solids {
                match mirror_x(&solid.solid) {
                    Some(m) => mirrored.push(m),
                    None => return self.lose(reasons::BODY_NOT_EXPORTED, class, object),
                }
            }
            brep(mirrored.iter(), &super::WORLD, 0.0)
        } else {
            brep(placed.solids.iter().map(|s| &s.solid), &super::WORLD, 0.0)
        };
        if !placed.shared.is_empty() {
            self.lose_n(
                reasons::FAMILY_SHARED_NOT_WRITTEN,
                class,
                object,
                placed.shared.len(),
            );
        }
        let type_id = self.ensure_type(&resolved, &assigned, &placed.evaluation.values, object);
        let storey_id = storey.as_u128();
        let mut element = Element::new(
            object.id.as_u128(),
            assigned.entity.kind(),
            &name_of(object),
            storey_id,
        );
        element.placement = frame.placement(component.offset);
        element.predefined_type = assigned.predefined.clone();
        let size = placed.evaluation.size;
        element.overall_width = Some(size[0]).filter(|w| *w > 0.0);
        element.overall_height = Some(size[2]).filter(|h| *h > 0.0);
        match body {
            // A mirrored occurrence never becomes the type's shared geometry: the type's shape is
            // the family's own, and a mirror writes its own body.
            Some(body) if !component.mirror && self.share_geometry(type_id, &body) => {
                element.mapping = Some(Placement::default());
            }
            Some(body) => element.body = Some(body),
            None => self.lose(reasons::BODY_NOT_EXPORTED, class, object),
        }
        self.occurrence(
            &mut element,
            (&resolved, &assigned, type_id),
            &placed.evaluation.values,
            object,
        );
        self.elements.push(element);
        self.exported += 1;
    }

    /// The element type of a family type, created on its first occurrence: its predefined type,
    /// its property sets (`ArchBuilder_Type` and `ArchBuilder_Family`) and the parameters IFC has
    /// no value type for, counted once. Returns the identifier of the type.
    fn ensure_type(
        &mut self,
        resolved: &Resolved,
        assigned: &Assignment,
        values: &BTreeMap<String, Evaluated>,
        object: &Object,
    ) -> u128 {
        let type_id = resolved.ty.id.as_u128();
        if self.types.iter().any(|t| t.id == type_id) {
            return type_id;
        }
        let mut ty = ElementType::new(type_id, assigned.entity.kind(), &resolved.ty.name);
        ty.predefined_type = assigned.predefined.clone();
        let (properties, as_text) = parameter_set(&resolved.definition, values, Scope::Type);
        if !properties.is_empty() {
            ty.property_sets
                .push(PropertySet::new("ArchBuilder_Type", properties));
        }
        ty.property_sets.push(family_set(resolved));
        if as_text > 0 {
            self.lose_n(
                reasons::FAMILY_PARAMETER_AS_TEXT,
                assigned.entity.class(),
                object,
                as_text,
            );
        }
        self.types.push(ty);
        type_id
    }

    /// Makes `body` the shared geometry of the type `type_id` when it has none, and whether the
    /// type holds `body`: an occurrence with that body maps the type's representation instead of
    /// writing its own.
    fn share_geometry(&mut self, type_id: u128, body: &Body) -> bool {
        let Some(ty) = self.types.iter_mut().find(|t| t.id == type_id) else {
            return false;
        };
        if ty.representation.is_none() {
            ty.representation = Some(body.clone());
            return true;
        }
        ty.representation.as_ref() == Some(body)
    }

    /// The occurrence's own data: its `ArchBuilder_Instance` set, the standard sets of its entity
    /// where the category maps, and its element type.
    fn occurrence(
        &mut self,
        element: &mut Element,
        (resolved, assigned, type_id): (&Resolved, &Assignment, u128),
        values: &BTreeMap<String, Evaluated>,
        object: &Object,
    ) {
        let (properties, as_text) = parameter_set(&resolved.definition, values, Scope::Instance);
        if !properties.is_empty() {
            element
                .property_sets
                .push(PropertySet::new("ArchBuilder_Instance", properties));
        }
        if as_text > 0 {
            self.lose_n(
                reasons::FAMILY_PARAMETER_AS_TEXT,
                assigned.entity.class(),
                object,
                as_text,
            );
        }
        if let Some(common) = match assigned.entity {
            Entity::Door => Some(("Pset_DoorCommon", "Qto_DoorBaseQuantities")),
            Entity::Window => Some(("Pset_WindowCommon", "Qto_WindowBaseQuantities")),
            _ => None,
        } {
            psets::attach(
                element,
                common.0,
                vec![psets::identifier("Reference", &resolved.ty.name)],
            );
            let mut quantities = Vec::new();
            if let Some(w) = element.overall_width {
                quantities.push(Quantity::new("Width", QuantityValue::Length(w)));
            }
            if let Some(h) = element.overall_height {
                quantities.push(Quantity::new("Height", QuantityValue::Length(h)));
            }
            if !quantities.is_empty() {
                element
                    .quantity_sets
                    .push(arc_io_ifc::write::QuantitySet::new(common.1, quantities));
            }
        }
        element.property_sets.push(family_set(resolved));
        element.element_type = Some(type_id);
    }

    /// A hosted family instance of a wall: the `IfcOpeningElement` its host-cutting void makes
    /// (`IfcRelVoidsElement`) and, for a door or window family, the `IfcDoor` or `IfcWindow` that
    /// fills it (`IfcRelFillsElement`). A hosted entity the writer cannot write is named in the
    /// loss report; its opening is still written, because the wall is cut.
    pub(super) fn family_insert(
        &mut self,
        object: &Object,
        placed: (&Object, &Wall, &Frame),
        storey: ObjectId,
    ) {
        let (host, wall, wall_frame) = placed;
        let Some(insert) = loaded::FamilyInsert::of(object) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCOPENINGELEMENT", object);
        };
        let Ok(resolved) = loaded::resolve_insert(self.snapshot, &insert) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCOPENINGELEMENT", object);
        };
        let Some(item) = loaded::opening_of(self.snapshot, &insert) else {
            // Nothing cuts the host: the instance is a hosted body without an opening.
            return self.lose(reasons::NOT_EXPORTED, "IFCOPENINGELEMENT", object);
        };
        self.wall_opening(object, &item, (host, wall, wall_frame), storey);
        let assigned = match assignment(&resolved.definition.header) {
            Ok(a) => a,
            Err(_) => {
                return self.lose(
                    reasons::FAMILY_NOT_WRITTEN,
                    "IFCBUILDINGELEMENTPROXY",
                    object,
                );
            }
        };
        let Ok(evaluation) = loaded::evaluate_in(
            self.snapshot,
            &resolved.definition,
            &resolved.ty,
            &insert.values,
        ) else {
            return self.lose(reasons::BODY_NOT_EXPORTED, assigned.entity.class(), object);
        };
        let turned = if insert.facing == archbuilder_model::insert::Facing::Interior {
            -1.0
        } else {
            1.0
        };
        let Some(frame) = Frame::new(
            wall_frame.at(insert.distance, 0.0),
            [wall_frame.u[0] * turned, wall_frame.u[1] * turned],
        ) else {
            return self.lose(reasons::NOT_EXPORTED, assigned.entity.class(), object);
        };
        if !evaluation.shared.is_empty() {
            self.lose_n(
                reasons::FAMILY_SHARED_NOT_WRITTEN,
                assigned.entity.class(),
                object,
                evaluation.shared.len(),
            );
        }
        let type_id = self.ensure_type(&resolved, &assigned, &evaluation.values, object);
        let storey_id = storey.as_u128();
        let mut element = Element::new(
            object.id.as_u128(),
            assigned.entity.kind(),
            &name_of(object),
            storey_id,
        );
        element.placement = frame.placement(wall.base_offset + insert.offset);
        element.predefined_type = assigned.predefined.clone();
        element.host = Some(arc_io_ifc::write::derived_id(
            object.id.as_u128(),
            arc_io_ifc::write::role::INSERT_OPENING,
            0,
        ));
        element.overall_width = Some(item.width).filter(|w| *w > 0.0);
        element.overall_height = Some(item.height).filter(|h| *h > 0.0);
        // The right hand mirrors the family across its own left-right axis.
        let mirror = insert.hand == archbuilder_model::insert::Hand::Right;
        let mut solids = Vec::new();
        for s in &evaluation.solids {
            let moved = if mirror {
                mirror_x(&s.solid)
            } else {
                Some(s.solid.clone())
            };
            match moved {
                Some(m) => solids.push(m),
                None => {
                    return self.lose(reasons::BODY_NOT_EXPORTED, assigned.entity.class(), object);
                }
            }
        }
        match brep(solids.iter(), &super::WORLD, 0.0) {
            Some(body) => element.body = Some(body),
            None => self.lose(reasons::BODY_NOT_EXPORTED, assigned.entity.class(), object),
        }
        self.occurrence(
            &mut element,
            (&resolved, &assigned, type_id),
            &evaluation.values,
            object,
        );
        self.elements.push(element);
        self.exported += 1;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use std::collections::BTreeSet;

    use arc_base::id::Identifier;
    use archbuilder_model::family::def::Version;

    use super::*;

    fn header(category: Category, entity: &str, ifc_type: &str) -> Header {
        Header {
            id: ObjectId::from_u128(1),
            version: Version::default(),
            name: "x".into(),
            category,
            hosting: archbuilder_model::family::def::Hosting::None,
            flags: BTreeSet::new(),
            ifc_entity: entity.into(),
            ifc_type: ifc_type.into(),
            functions: (1, 0),
            source: String::new(),
            license: String::new(),
        }
    }

    #[test]
    fn every_category_has_the_default_of_the_adr_table() {
        let class = |c| default_of(c).unwrap().0.class();
        assert_eq!(class(Category::Furniture), "IFCFURNITURE");
        assert_eq!(
            default_of(Category::Casework),
            Some((Entity::Furniture, Some("CASEWORK")))
        );
        assert_eq!(class(Category::PlumbingFixture), "IFCSANITARYTERMINAL");
        assert_eq!(class(Category::LightingFixture), "IFCLIGHTFIXTURE");
        assert_eq!(class(Category::ElectricalEquipment), "IFCELECTRICAPPLIANCE");
        assert_eq!(class(Category::MechanicalEquipment), "IFCUNITARYEQUIPMENT");
        assert_eq!(class(Category::Door), "IFCDOOR");
        assert_eq!(class(Category::Window), "IFCWINDOW");
        assert_eq!(class(Category::Column), "IFCCOLUMN");
        // A proxy is never a default: the categories without an entity name none.
        let none: Vec<Category> = Category::ALL
            .into_iter()
            .filter(|c| default_of(*c).is_none())
            .collect();
        assert_eq!(
            none,
            vec![
                Category::GenericModel,
                Category::SpecialtyEquipment,
                Category::Profile
            ]
        );
    }

    #[test]
    fn the_own_assignment_overrides_the_default() {
        let a = assignment(&header(
            Category::Furniture,
            "ifcsanitaryterminal",
            "WCSEAT",
        ))
        .unwrap();
        assert_eq!(a.entity, Entity::SanitaryTerminal);
        assert_eq!(a.predefined.as_deref(), Some("WCSEAT"));
        // A type alone keeps the entity of the category.
        let a = assignment(&header(Category::Casework, "", "SHELF")).unwrap();
        assert_eq!(
            (a.entity, a.predefined.as_deref()),
            (Entity::Furniture, Some("SHELF"))
        );
        // A generic model writes the proxy only when it declares that entity.
        let a = assignment(&header(
            Category::GenericModel,
            "IfcBuildingElementProxy",
            "COMPLEX",
        ))
        .unwrap();
        assert_eq!(
            (a.entity, a.predefined.as_deref()),
            (Entity::Proxy, Some("COMPLEX"))
        );
        assert_eq!(
            assignment(&header(Category::Column, "IfcBeam", "BEAM"))
                .unwrap()
                .entity,
            Entity::Beam
        );
        assert!(assignment(&header(Category::GenericModel, "", "")).is_err());
        // An entity this mapping does not know is refused, not replaced by a proxy.
        assert_eq!(
            assignment(&header(Category::Furniture, "IfcPump", "")),
            Err("IfcPump".to_owned())
        );
    }

    #[test]
    fn every_entity_is_found_by_its_class() {
        for entity in Entity::ALL {
            assert_eq!(Entity::parse(entity.class()), Some(entity));
        }
    }

    #[test]
    fn parameter_values_get_their_ifc_value_types() {
        let n = Evaluated::Number;
        assert_eq!(
            value_of(DataType::Length, &n(0.9)),
            Some((PropertyValue::Length(0.9), false))
        );
        assert_eq!(
            value_of(DataType::Area, &n(2.0)),
            Some((PropertyValue::Area(2.0), false))
        );
        assert_eq!(
            value_of(DataType::Integer, &n(3.0)),
            Some((PropertyValue::Integer(3), false))
        );
        assert_eq!(
            value_of(DataType::Angle, &n(1.0)),
            Some((PropertyValue::Measure(Measure::PlaneAngle, 1.0), false))
        );
        assert_eq!(
            value_of(DataType::Boolean, &Evaluated::Bool(true)),
            Some((PropertyValue::Boolean(true), false))
        );
        assert_eq!(
            value_of(DataType::FamilyType, &Evaluated::Text("A".into())),
            Some((PropertyValue::Label("A".into()), true))
        );
        assert_eq!(
            value_of(DataType::Text, &Evaluated::Text(String::new())),
            None
        );
        assert!(value_of(DataType::Number, &n(f64::NAN)).is_none());
    }
}
