//! A curtain wall as an `IfcCurtainWall` with no body of its own that aggregates its parts
//! (`IfcRelAggregates`): an `IfcPlate` of the predefined type `CURTAIN_PANEL` for each glazed or
//! solid panel and an `IfcMember` of the predefined type `MULLION` for each mullion (interior,
//! border and corner alike). The identifier of a part is the one the model gives the panel or
//! mullion ([`archbuilder_model::curtain::child_id`]), so it is the same after every
//! regeneration. The wall type is an `IfcCurtainWallType`; the panels of a type share an
//! `IfcPlateType` for each kind of panel and its mullions an `IfcMemberType` for each position.
//!
//! IFC4 lets a door or window that is a panel of a curtain wall be a part of it, but the pinned
//! writer aggregates plates and members only. A door or window panel is therefore an `IfcDoor` or
//! `IfcWindow` that fills an `IfcOpeningElement` voiding the curtain wall, the way it fills one
//! in a basic wall, and the loss report names the curtain wall as approximated. The curtain wall
//! inside a basic wall that cuts an opening in it is written with that wall, as before.

use arc_base::ObjectId;
use arc_base::id::Identifier;
use arc_doc::Object;
use arc_io_ifc::write::{
    Element, ElementKind, ElementType, Property, PropertySet, PropertyValue, Quantity, QuantitySet,
    QuantityValue, derived_id, role,
};
use archbuilder_model::curtain::{self, Class, Derived, MullionRole, PANEL_ROLE, PanelKind};
use archbuilder_model::element_type::{self as et, Exposure, TypeKind, WallType};

use super::{Frame, Mapper, Own, WORLD, brep, name_of};
use crate::error::reasons;

/// The role of the plate type of a panel kind of a curtain type in [`derived_id`].
const PLATE_TYPE: u32 = role::CALLER_FIRST + 5;
/// The role of the member type of a mullion position of a curtain type.
const MULLION_TYPE: u32 = role::CALLER_FIRST + 6;

impl Mapper<'_> {
    /// The type of the parts of one kind, made once for the curtain type `owner`.
    fn part_type(
        &mut self,
        owner: &Object,
        (role_id, index): (u32, u32),
        kind: ElementKind,
        name: &str,
        predefined: &str,
    ) -> u128 {
        let id = derived_id(owner.id.as_u128(), role_id, index);
        if !self.types.iter().any(|t| t.id == id) {
            let mut t = ElementType::new(id, kind, &format!("{} {name}", et::name(owner)));
            t.predefined_type = Some(predefined.to_owned());
            self.types.push(t);
        }
        id
    }

    /// A curtain wall of the storey whose level is at `level`.
    #[allow(
        clippy::too_many_lines,
        reason = "one wall: its panels and its mullions"
    )]
    pub(super) fn curtain_wall(
        &mut self,
        object: &Object,
        derived: &Derived,
        storey: ObjectId,
        level: f64,
    ) {
        let (id, storey_id, name) = (object.id.as_u128(), storey.as_u128(), name_of(object));
        let Some(wall_type) = et::type_of(self.snapshot, TypeKind::Wall, &object.values) else {
            return self.lose(reasons::NOT_EXPORTED, "IFCCURTAINWALL", object);
        };
        let flags = WallType::from_values(&wall_type.values);
        let external = flags
            .as_ref()
            .is_some_and(|t| t.exposure == Exposure::Exterior);
        let load_bearing = flags.as_ref().is_some_and(|t| t.load_bearing);
        let mut wall = Element::new(id, ElementKind::CurtainWall, &name, storey_id);
        wall.element_type = Some(wall_type.id.as_u128());
        wall.property_sets.push(PropertySet::new(
            "Pset_CurtainWallCommon",
            vec![Property::new(
                "IsExternal",
                PropertyValue::Boolean(external),
            )],
        ));
        let (length, height) = (derived.wall.length(), derived.wall.height);
        wall.quantity_sets.push(QuantitySet::new(
            "Qto_CurtainWallQuantities",
            vec![
                Quantity::new("Length", QuantityValue::Length(length)),
                Quantity::new("Height", QuantityValue::Length(height)),
                Quantity::new("Width", QuantityValue::Length(derived.ty.depth())),
                Quantity::new("GrossSideArea", QuantityValue::Area(length * height)),
            ],
        ));
        self.elements.push(wall);
        self.exported += 1;
        let common = |external: bool, load_bearing: bool| {
            vec![
                Property::new("IsExternal", PropertyValue::Boolean(external)),
                Property::new("LoadBearing", PropertyValue::Boolean(load_bearing)),
            ]
        };
        let Ok(solids) = derived.solids() else {
            return self.lose(reasons::BODY_NOT_EXPORTED, "IFCCURTAINWALL", object);
        };
        let thickness = derived.ty.panel_thickness.min(derived.wall.thickness);
        for solid in &solids {
            let volume = solid.solid.volume();
            let body = brep([&solid.solid], &WORLD, level);
            let mut part = match (solid.class, solid.key.split_once(':')) {
                (Class::Glass | Class::Solid, Some(("p", key))) if !key.contains('#') => {
                    let Some(panel) = derived.panels.iter().find(|p| p.key == key) else {
                        continue;
                    };
                    let kind = if solid.class == Class::Solid {
                        PanelKind::Solid
                    } else {
                        PanelKind::Glazed
                    };
                    let mut plate = Element::new(
                        solid.id,
                        ElementKind::Plate,
                        &format!("{name} panel {key}"),
                        storey_id,
                    );
                    plate.predefined_type = Some("CURTAIN_PANEL".to_owned());
                    let (label, code) = match kind {
                        PanelKind::Solid => ("solid panel", 1),
                        _ => ("glazed panel", 0),
                    };
                    plate.element_type = Some(self.part_type(
                        &wall_type,
                        (PLATE_TYPE, code),
                        ElementKind::Plate,
                        label,
                        "CURTAIN_PANEL",
                    ));
                    plate.property_sets.push(PropertySet::new(
                        "Pset_PlateCommon",
                        common(external, load_bearing),
                    ));
                    plate.quantity_sets.push(QuantitySet::new(
                        "Qto_PlateBaseQuantities",
                        vec![
                            Quantity::new("Width", QuantityValue::Length(thickness)),
                            Quantity::new(
                                "Perimeter",
                                QuantityValue::Length(2.0 * (panel.width() + panel.height())),
                            ),
                            Quantity::new("GrossArea", QuantityValue::Area(panel.area())),
                            Quantity::new("NetArea", QuantityValue::Area(panel.area())),
                            Quantity::new("GrossVolume", QuantityValue::Volume(volume)),
                            Quantity::new("NetVolume", QuantityValue::Volume(volume)),
                        ],
                    ));
                    plate
                }
                (Class::Mullion, Some(("m", key))) => {
                    let Some(mullion) = derived.mullions.iter().find(|m| m.key == key) else {
                        continue;
                    };
                    let mut member = Element::new(
                        solid.id,
                        ElementKind::Member,
                        &format!("{name} mullion {key}"),
                        storey_id,
                    );
                    member.predefined_type = Some("MULLION".to_owned());
                    let (label, code) = match mullion.role {
                        MullionRole::Interior => ("interior mullion", 0),
                        MullionRole::Border => ("border mullion", 1),
                        MullionRole::Corner => ("corner mullion", 2),
                    };
                    member.element_type = Some(self.part_type(
                        &wall_type,
                        (MULLION_TYPE, code),
                        ElementKind::Member,
                        label,
                        "MULLION",
                    ));
                    member.property_sets.push(PropertySet::new(
                        "Pset_MemberCommon",
                        common(external, load_bearing),
                    ));
                    let long = mullion.length();
                    member.quantity_sets.push(QuantitySet::new(
                        "Qto_MemberBaseQuantities",
                        vec![
                            Quantity::new("Length", QuantityValue::Length(long)),
                            Quantity::new("CrossSectionArea", QuantityValue::Area(volume / long)),
                            Quantity::new("GrossVolume", QuantityValue::Volume(volume)),
                            Quantity::new("NetVolume", QuantityValue::Volume(volume)),
                        ],
                    ));
                    member
                }
                // The frame, leaf and glass of a door or window panel are its own body.
                _ => continue,
            };
            part.host = Some(id);
            match body {
                Some(body) => part.body = Some(body),
                None => self.lose(reasons::BODY_NOT_EXPORTED, "IFCCURTAINWALL", object),
            }
            self.elements.push(part);
        }
        self.panel_leaves(object, derived, storey, level);
    }

    /// The doors and windows that are panels of the curtain wall, each filling an opening in it.
    fn panel_leaves(&mut self, object: &Object, derived: &Derived, storey: ObjectId, level: f64) {
        let direction = [
            derived.wall.end[0] - derived.wall.start[0],
            derived.wall.end[1] - derived.wall.start[1],
        ];
        let Some(frame) = Frame::new(derived.wall.start, direction) else {
            return;
        };
        let mut leaves = 0;
        for panel in &derived.panels {
            let (Some(item), PanelKind::Door | PanelKind::Window) = (&panel.insert, panel.kind)
            else {
                continue;
            };
            let kind = if panel.kind == PanelKind::Door {
                TypeKind::Door
            } else {
                TypeKind::Window
            };
            let type_id = panel.type_ref.as_ref().map(|(t, _)| *t).filter(|t| {
                et::of_kind(self.snapshot, kind)
                    .iter()
                    .any(|found| found.id == *t)
            });
            let own = Own {
                id: curtain::child_id(object.id, PANEL_ROLE, &format!("p:{}", panel.key)),
                name: format!("{} {}", name_of(object), panel.key),
                type_id: type_id.map(|t| t.as_u128()),
            };
            self.insert_as(
                object,
                item,
                (object, &derived.wall, &frame),
                storey,
                level,
                Some(own),
            );
            leaves += 1;
        }
        if leaves > 0 {
            self.lose_n(
                reasons::CURTAIN_PANEL_FILLED,
                "IFCCURTAINWALL",
                object,
                leaves,
            );
        }
    }
}
