//! The data an import keeps beside each object it creates: the source class and identity and
//! everything of the source element that has no native field (property sets, quantity sets,
//! classification, material, type, groups). It is stored as a foreign data row of the object
//! ([`KIND_ELEMENT`]) and written back by the export (AB-IO-018), so a round trip keeps it.
//!
//! Lengths inside the values are in meters (the import applies the project's length unit), so the
//! export writes them in its own unit without another conversion.

use serde::{Deserialize, Serialize};

/// The source format name of every foreign row and source map record of an IFC import.
pub const FORMAT: &str = "ifc";
/// The `record_kind` of the row that holds the [`Kept`] data of one object.
pub const KIND_ELEMENT: &str = "ifc_element";
/// The `record_kind` of the row that holds the original file (document level).
pub const KIND_SOURCE: &str = "ifc_source_file";
/// The `record_kind` of the row that holds the loss report of an import (document level).
pub const KIND_REPORT: &str = "ifc_import_report";

/// A value of a property, as the source file states it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Nom {
    /// The wrapping type in upper case (`IFCLABEL`); absent for an unwrapped value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    /// A text value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// A real value; a length, area or volume measure is in meters, square meters or cubic meters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real: Option<f64>,
    /// An integer value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub int: Option<i64>,
    /// An enumeration value (`T`, `F`, `U` for booleans and logicals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enumeration: Option<String>,
}

/// The shape of a property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropKind {
    /// `IfcPropertySingleValue`.
    #[default]
    Single,
    /// `IfcPropertyEnumeratedValue`.
    Enumerated,
    /// `IfcPropertyListValue`.
    List,
    /// `IfcPropertyBoundedValue`.
    Bounded,
}

/// One property of a property set.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Prop {
    /// The name.
    pub name: String,
    /// The description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The shape.
    pub kind: PropKind,
    /// The values: one for a single value, the chosen ones or the list, and upper, lower and set
    /// point (those present) for a bounded value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Nom>,
    /// Whether the value came from the type object.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inherited: bool,
}

/// A property set.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Pset {
    /// The name.
    pub name: String,
    /// The properties.
    pub props: Vec<Prop>,
}

/// One quantity of a quantity set.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Qty {
    /// The name.
    pub name: String,
    /// `length`, `area`, `volume`, `count`, `weight` or `time`.
    pub kind: String,
    /// The value in SI base units.
    pub value: f64,
}

/// A quantity set.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Qto {
    /// The name.
    pub name: String,
    /// The quantities.
    pub quantities: Vec<Qty>,
}

/// A classification reference.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Class {
    /// The identification (the code).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identification: Option<String>,
    /// The name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The name of the classification system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// The edition of the system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
}

/// A material layer, profile or constituent of the source.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MaterialPart {
    /// The material name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// The thickness (layers) in meters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thickness: Option<f64>,
}

/// The material of the source element.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Material {
    /// `material`, `list`, `layers`, `profiles` or `constituents`.
    pub kind: String,
    /// The name of the set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The parts, in order.
    pub parts: Vec<MaterialPart>,
}

/// Everything kept of one source object.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Kept {
    /// The entity class in the schema's spelling (`IfcWall`).
    pub class: String,
    /// The `GlobalId` of the source object.
    pub global_id: String,
    /// The effective predefined type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predefined: Option<String>,
    /// The object type (the user defined kind).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,
    /// The tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// The description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The long name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long_name: Option<String>,
    /// The class of the type object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_class: Option<String>,
    /// The name of the type object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// The property sets, the type's merged in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub psets: Vec<Pset>,
    /// The quantity sets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qtos: Vec<Qto>,
    /// The classification references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classifications: Vec<Class>,
    /// The material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<Material>,
    /// The names of the groups the object belongs to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
}

impl Kept {
    /// The row data.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The data of a row; `None` when it does not parse.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}
