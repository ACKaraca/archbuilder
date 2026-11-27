//! The project information in the IFC file (AB-DATUM-002): the long name, description and phase
//! of the `IfcProject`, the postal address of the site and the building, the author and the
//! issuing organization and the client as actors assigned to the project, and the properties of
//! `Pset_BuildingCommon` that the IFC4 property set templates define (`Reference`,
//! `OccupancyType`, `YearOfConstruction`, `ConstructionMethod`).
//!
//! The values go to the writer's neutral description of the project, the site and the building
//! ([`Info::apply`]): the long name, description and phase of the project, the addresses of the site
//! and the building, the actors assigned to the project and the property set of the building. The
//! file is read back and checked after the write, like every export. IFC4 has no
//! `Pset_ProjectCommon`, and the issue date has no attribute, so the issue date is reported as not
//! written.

use arc_doc::Object;
use arc_io_ifc::WriteModel;
use arc_io_ifc::write::{Actor, Address, Party, Property, PropertySet, PropertyValue};
use archbuilder_model::types::{self, building, project};

/// The values to write; every one is optional.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Info {
    pub number: Option<String>,
    pub client: Option<String>,
    /// The address of the site (the project's).
    pub site_address: Option<String>,
    /// The address of the building: its own, else the project's.
    pub building_address: Option<String>,
    pub status: Option<String>,
    pub author: Option<String>,
    pub organization: Option<String>,
    pub description: Option<String>,
    pub long_name: Option<String>,
    pub occupancy_type: Option<String>,
    pub year_of_construction: Option<String>,
    pub construction_method: Option<String>,
    pub issue_date: Option<String>,
}

impl Info {
    pub(crate) fn of(project_object: &Object, building_object: &Object) -> Self {
        let get = |o: &Object, at: usize| {
            types::text(&o.values, at)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
        };
        let address = get(project_object, project::ADDRESS);
        Self {
            number: get(project_object, project::NUMBER),
            client: get(project_object, project::CLIENT),
            site_address: address.clone(),
            building_address: get(building_object, building::ADDRESS).or(address),
            status: get(project_object, project::STATUS),
            author: get(project_object, project::AUTHOR),
            organization: get(project_object, project::ORGANIZATION),
            description: get(project_object, project::DESCRIPTION),
            long_name: get(project_object, project::LONG_NAME),
            occupancy_type: get(project_object, project::OCCUPANCY_TYPE),
            year_of_construction: get(project_object, project::YEAR_OF_CONSTRUCTION),
            construction_method: get(project_object, project::CONSTRUCTION_METHOD),
            issue_date: get(project_object, project::ISSUE_DATE),
        }
    }
}

/// The identifier of an actor this module adds: the project's identifier with a tag in its top
/// bits, so it is the same on every export and apart from every object identifier.
fn actor_id(project_id: u128, tag: u128) -> u128 {
    project_id ^ ((0xA5_u128 << 120) | tag)
}

/// The address of an address text: one address line per line of the text.
fn address(text: &str) -> Option<Address> {
    let lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    (!lines.is_empty()).then(|| {
        let mut address = Address::default();
        address.lines = lines;
        address
    })
}

impl Info {
    /// Writes the values into the project, the site and the building of `model`.
    pub(crate) fn apply(&self, model: &mut WriteModel, project_id: u128) {
        model.project.description = self.description.clone();
        model.project.long_name = self.long_name.clone();
        model.project.phase = self.status.clone();
        model.site.address = self.site_address.as_deref().and_then(address);
        model.building.address = self.building_address.as_deref().and_then(address);
        // The client, and the author with the issuing organization.
        let mut tag = 1_u128;
        if let Some(client) = &self.client {
            let mut actor = Actor::new(
                actor_id(project_id, tag),
                "Client",
                Party::Organization(client.clone()),
            );
            actor.role = Some("CLIENT".to_owned());
            model.project.actors.push(actor);
            tag += 1;
        }
        if let Some(author) = &self.author {
            let (given, family) = match author.trim().rsplit_once(' ') {
                Some((given, family)) => (Some(given.trim().to_owned()), family.to_owned()),
                None => (None, author.trim().to_owned()),
            };
            model.project.actors.push(Actor::new(
                actor_id(project_id, tag),
                "Author",
                Party::Person {
                    family_name: Some(family),
                    given_name: given,
                },
            ));
            tag += 1;
        }
        if let Some(organization) = &self.organization {
            model.project.actors.push(Actor::new(
                actor_id(project_id, tag),
                "Issuer",
                Party::Organization(organization.clone()),
            ));
        }
        // Pset_BuildingCommon: only properties the IFC4 template defines, with its data types.
        let properties: Vec<Property> = [
            ("Reference", &self.number, true),
            ("OccupancyType", &self.occupancy_type, false),
            ("YearOfConstruction", &self.year_of_construction, false),
            ("ConstructionMethod", &self.construction_method, false),
        ]
        .into_iter()
        .filter_map(|(name, value, identifier)| {
            let value = value.as_ref()?;
            Some(Property::new(
                name,
                if identifier {
                    PropertyValue::Identifier(value.clone())
                } else {
                    PropertyValue::Label(value.clone())
                },
            ))
        })
        .collect();
        if !properties.is_empty() {
            model
                .building
                .property_sets
                .push(PropertySet::new("Pset_BuildingCommon", properties));
        }
    }
}
