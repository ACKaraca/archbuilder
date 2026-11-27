//! Document variables (ADR 0009, section 1): named values of the whole document that formulas
//! read by their formula identifier. A variable stores a literal or a formula; a variable's
//! formula reads other variables only. Variables evaluate in dependency order (ties by
//! identifier); a cycle is refused with the cycle named. The results are derived, never stored.

use std::sync::Arc;

use arc_base::{ObjectId, Result, Unit};
use arc_doc::{Bound, EnumVariant, FieldDecl, Object, Snapshot, Value, ValueType};
use arc_solve::Value as SolveValue;

use crate::expr::{self, DataType, Evaluated, Node, Stored};
use crate::spatial;
use crate::types::{self, VARIABLE, variable};

/// One document variable as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    /// The record's identifier, which never changes.
    pub id: ObjectId,
    /// The display name.
    pub name: String,
    /// The formula identifier.
    pub identifier: String,
    /// The data type.
    pub data: DataType,
    /// The value; a variable read without one has its type's zero.
    pub value: Option<Stored>,
}

impl Variable {
    /// The variable stored in `object`.
    #[must_use]
    pub fn of(object: &Object) -> Self {
        let v = &object.values;
        Self {
            id: object.id,
            name: types::text(v, variable::NAME)
                .unwrap_or_default()
                .to_owned(),
            identifier: types::text(v, variable::IDENTIFIER)
                .unwrap_or_default()
                .to_owned(),
            data: DataType::of_field(v, variable::DATA_TYPE),
            value: Stored::from_value(v.get(variable::VALUE).and_then(Option::as_ref)),
        }
    }

    /// The node of this variable in its scope.
    #[must_use]
    pub fn node(&self) -> Node<'_> {
        Node {
            key: self.id,
            identifier: &self.identifier,
            name: &self.name,
            data: self.data,
            value: self.value.as_ref(),
        }
    }
}

/// The values of a variable record.
#[must_use]
pub fn values(name: &str, identifier: &str, data: DataType, value: &Stored) -> Vec<Option<Value>> {
    vec![
        Some(Value::String(name.trim().to_owned())),
        Some(Value::String(identifier.to_owned())),
        Some(Value::Enum(data.code())),
        Some(value.to_value()),
    ]
}

/// The choice of data type as a field.
pub(crate) fn data_type_field() -> FieldDecl {
    FieldDecl::new(
        "data_type",
        ValueType::Enum(
            DataType::ALL
                .iter()
                .map(|d| EnumVariant::new(d.name(), d.code()))
                .collect(),
        ),
    )
    .with_default(Value::Enum(DataType::Length.code()))
    .in_aspect("data")
}

/// The formula identifier as a field.
pub(crate) fn identifier_field() -> FieldDecl {
    FieldDecl::new("identifier", ValueType::String)
        .with_bound(Bound::Length {
            max: expr::MAX_IDENTIFIER as u64,
        })
        .in_aspect("identity")
}

/// The fields of a variable: name, formula identifier, data type and value.
#[must_use]
pub fn fields() -> Vec<FieldDecl> {
    vec![
        types::name_field("identity"),
        identifier_field(),
        data_type_field(),
        FieldDecl::new("value", ValueType::Record(expr::stored_fields()))
            .optional()
            .in_aspect("data"),
    ]
}

/// Every variable of the document, by folded name and then identifier.
#[must_use]
pub fn all(snapshot: &Snapshot) -> Vec<Variable> {
    let mut found: Vec<Arc<Object>> = spatial::objects(snapshot, VARIABLE);
    found.sort_by(|a, b| {
        crate::names::fold(types::text(&a.values, 0).unwrap_or_default())
            .cmp(&crate::names::fold(
                types::text(&b.values, 0).unwrap_or_default(),
            ))
            .then(a.id.cmp(&b.id))
    });
    found.iter().map(|o| Variable::of(o)).collect()
}

/// Every variable with its value, in the order of [`all`].
///
/// # Errors
/// `ab.model.param_cycle` naming a cycle, and the first formula error with the argument
/// `parameter` (the variable's name).
pub fn evaluate(snapshot: &Snapshot) -> Result<Vec<(Variable, Evaluated)>> {
    let all = all(snapshot);
    let nodes: Vec<Node<'_>> = all.iter().map(Variable::node).collect();
    // A variable reads other variables only.
    let values = expr::evaluate_scope(&nodes, &|_| None)?;
    Ok(all.into_iter().zip(values).collect())
}

/// The value of the variable `identifier` among `evaluated`, as a formula reads it.
#[must_use]
pub fn lookup(evaluated: &[(Variable, Evaluated)], identifier: &str) -> Option<SolveValue> {
    evaluated
        .iter()
        .find(|(v, _)| v.identifier == identifier)
        .map(|(v, value)| value.to_solve(v.data))
}

/// The unit bare numbers of a formula of `data` take when the input names none: the project's
/// length unit, square and cubic meters, degrees; none for the dimensionless types.
#[must_use]
pub fn default_unit(snapshot: &Snapshot, data: DataType) -> Option<&'static Unit> {
    match data {
        DataType::Length => spatial::length_unit(snapshot).unit(),
        DataType::Area => Unit::by_symbol("m²").ok(),
        DataType::Volume => Unit::by_symbol("m³").ok(),
        DataType::Angle => Unit::by_symbol("°").ok(),
        _ => None,
    }
}

/// The formula identifiers of the document other than that of `except`.
#[must_use]
pub fn identifiers(snapshot: &Snapshot, except: Option<ObjectId>) -> Vec<String> {
    [VARIABLE]
        .iter()
        .flat_map(|key| spatial::objects(snapshot, key))
        .filter(|o| Some(o.id) != except)
        .filter_map(|o| types::text(&o.values, variable::IDENTIFIER).map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use arc_base::id::Identifier;

    use super::*;

    #[test]
    fn values_read_back() {
        let stored = Stored::Formula(expr::Formula::new("1 m + x", None).unwrap());
        let v = values(" Grid ", "grid", DataType::Length, &stored);
        let object = Object::new(
            ObjectId::from_u128(7),
            arc_doc::SchemaVersion::new(1, 0),
            arc_doc::Header::default(),
            v,
        );
        let read = Variable::of(&object);
        assert_eq!(read.name, "Grid");
        assert_eq!(read.identifier, "grid");
        assert_eq!(read.value, Some(stored));
    }
}
