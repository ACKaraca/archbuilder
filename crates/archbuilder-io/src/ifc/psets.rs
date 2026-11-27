//! Standard property sets with the data types of their IFC4 templates. A property is written
//! only when ArchBuilder knows its value and the value satisfies the rule of the template's
//! measure type (a positive length is greater than zero, a count is finite); a set with no
//! property is not written.

use arc_io_ifc::write::{Element, Measure, Property, PropertySet, PropertyValue};

/// A measure property; `None` when `value` breaks the rule of `measure`.
pub(super) fn measure(name: &str, measure: Measure, value: f64) -> Option<Property> {
    PropertyValue::measure(measure, value)
        .ok()
        .map(|v| Property::new(name, v))
}

/// A count property (`IfcCountMeasure`) of `n`.
pub(super) fn count(name: &str, n: usize) -> Option<Property> {
    measure(
        name,
        Measure::Count,
        f64::from(u32::try_from(n).unwrap_or(u32::MAX)),
    )
}

/// An `IfcBoolean` property.
pub(super) fn flag(name: &str, value: bool) -> Option<Property> {
    Some(Property::new(name, PropertyValue::Boolean(value)))
}

/// An `IfcIdentifier` property; `None` for an empty text or one the type refuses.
pub(super) fn identifier(name: &str, text: &str) -> Option<Property> {
    if text.is_empty() {
        return None;
    }
    PropertyValue::identifier(text)
        .ok()
        .map(|v| Property::new(name, v))
}

/// Adds the set `name` of the known `properties` to `element`; nothing when none is known.
pub(super) fn attach(element: &mut Element, name: &str, properties: Vec<Option<Property>>) {
    let properties: Vec<Property> = properties.into_iter().flatten().collect();
    if !properties.is_empty() {
        element
            .property_sets
            .push(PropertySet::new(name, properties));
    }
}
