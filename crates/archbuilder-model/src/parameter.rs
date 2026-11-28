//! Project parameters (ADR 0009, section 1): definitions owned by the project, bound to categories
//! of physical elements, with a type or instance scope, a group and order, a default and an
//! optional rule. Values live in the `parameters` field of the elements (instance scope) and of
//! the element type records (type scope): a keyed list of (parameter, stored value) holding only
//! values that differ from the default.
//!
//! An instance reads its own value, else its type's, else the default. A type formula reads type
//! parameters and document variables; an instance formula also reads the instance's parameters.
//! Results are derived, never stored.

use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_doc::{
    Bound, Cardinality, EnumVariant, FieldDecl, Object, OnDelete, Snapshot, TypeKey, Value,
    ValueType,
};
use arc_solve::Value as SolveValue;

use crate::element_type::{self as et, TypeKind};
use crate::error::codes;
use crate::expr::{self, DataType, Evaluated, Node, Stored};
use crate::spatial;
use crate::types::{self, PARAMETER, PARAMETER_HOSTS, parameter};
use crate::variable::{self, Variable};

/// The most categories one parameter binds.
pub const MAX_CATEGORIES: usize = 64;
/// The most allowed values of one rule.
pub const MAX_ALLOWED: usize = 256;
/// The longest group name, in bytes.
pub const MAX_GROUP: u64 = 64;

coded! {
    /// Where a parameter's value is set.
    Scope {
        /// On each element.
        #[default]
        Instance = 0, "instance";
        /// On the element type; every instance of the type shares it.
        Type = 1, "type";
    }
}

/// The rule of a parameter: bounds, allowed values, a required value; broken values are refused
/// when `reject`, otherwise reported as warnings.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rule {
    /// The least number, in SI base units.
    pub minimum: Option<f64>,
    /// The greatest number, in SI base units.
    pub maximum: Option<f64>,
    /// The texts a value may be; empty allows any.
    pub allowed: Vec<String>,
    /// Whether every bound object needs a value.
    pub required: bool,
    /// Whether a broken value refuses the change (otherwise it warns).
    pub reject: bool,
}

/// Field positions of a rule record.
pub mod rule {
    /// The least number.
    pub const MINIMUM: usize = 0;
    /// The greatest number.
    pub const MAXIMUM: usize = 1;
    /// The allowed texts.
    pub const ALLOWED: usize = 2;
    /// Whether a value is required.
    pub const REQUIRED: usize = 3;
    /// Whether a broken rule refuses.
    pub const REJECT: usize = 4;
}

impl Rule {
    /// The record value.
    #[must_use]
    pub fn to_value(&self) -> Value {
        Value::Record(vec![
            self.minimum.map(Value::F64),
            self.maximum.map(Value::F64),
            (!self.allowed.is_empty())
                .then(|| Value::List(self.allowed.iter().cloned().map(Value::String).collect())),
            Some(Value::Bool(self.required)),
            Some(Value::Bool(self.reject)),
        ])
    }

    /// The rule of a record value.
    #[must_use]
    pub fn from_value(value: Option<&Value>) -> Option<Self> {
        let Some(Value::Record(f)) = value else {
            return None;
        };
        let get = |at: usize| f.get(at).and_then(Option::as_ref);
        let number = |at| match get(at) {
            Some(Value::F64(v)) => Some(*v),
            _ => None,
        };
        let flag = |at| matches!(get(at), Some(Value::Bool(true)));
        Some(Self {
            minimum: number(rule::MINIMUM),
            maximum: number(rule::MAXIMUM),
            allowed: match get(rule::ALLOWED) {
                Some(Value::List(items)) => items
                    .iter()
                    .take(MAX_ALLOWED)
                    .filter_map(|v| match v {
                        Value::String(t) => Some(t.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            required: flag(rule::REQUIRED),
            reject: flag(rule::REJECT),
        })
    }

    /// What `value` breaks (`required`, `minimum 0.5`, `maximum 2`, `allowed`); `None` when it
    /// keeps the rule. `None` stands for no value.
    #[must_use]
    pub fn broken(&self, value: Option<&Evaluated>) -> Option<String> {
        match value {
            None => self.required.then(|| "required".to_owned()),
            Some(Evaluated::Text(t)) if t.is_empty() && self.required => {
                Some("required".to_owned())
            }
            Some(Evaluated::Number(v)) => match (self.minimum, self.maximum) {
                (Some(min), _) if *v < min => Some(format!("minimum {min}")),
                (_, Some(max)) if *v > max => Some(format!("maximum {max}")),
                _ => None,
            },
            Some(Evaluated::Text(t)) => (!self.allowed.is_empty() && !self.allowed.contains(t))
                .then(|| "allowed".to_owned()),
            Some(Evaluated::Bool(_)) => None,
        }
    }
}

/// One project parameter as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    /// The record's identifier, which never changes.
    pub id: ObjectId,
    /// The display name.
    pub name: String,
    /// The formula identifier.
    pub identifier: String,
    /// The data type.
    pub data: DataType,
    /// Where values are set.
    pub scope: Scope,
    /// The element type keys of the categories it is bound to.
    pub categories: Vec<String>,
    /// The group it is shown in; empty for the common group.
    pub group: String,
    /// The order within the group.
    pub order: i64,
    /// The default value.
    pub default: Option<Stored>,
    /// The rule.
    pub rule: Option<Rule>,
}

impl Parameter {
    /// The parameter stored in `object`.
    #[must_use]
    pub fn of(object: &Object) -> Self {
        let v = &object.values;
        let get = |at: usize| v.get(at).and_then(Option::as_ref);
        Self {
            id: object.id,
            name: types::text(v, parameter::NAME)
                .unwrap_or_default()
                .to_owned(),
            identifier: types::text(v, parameter::IDENTIFIER)
                .unwrap_or_default()
                .to_owned(),
            data: DataType::of_field(v, parameter::DATA_TYPE),
            scope: Scope::of_field(v, parameter::SCOPE),
            categories: match get(parameter::CATEGORIES) {
                Some(Value::List(items)) => items
                    .iter()
                    .take(MAX_CATEGORIES)
                    .filter_map(|v| match v {
                        Value::String(t) => Some(t.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            },
            group: types::text(v, parameter::GROUP)
                .unwrap_or_default()
                .to_owned(),
            order: match get(parameter::ORDER) {
                Some(Value::I64(n)) => *n,
                _ => 0,
            },
            default: Stored::from_value(get(parameter::DEFAULT)),
            rule: Rule::from_value(get(parameter::RULE)),
        }
    }

    /// The record values.
    #[must_use]
    pub fn values(&self) -> Vec<Option<Value>> {
        vec![
            Some(Value::String(self.name.trim().to_owned())),
            Some(Value::String(self.identifier.clone())),
            Some(Value::Enum(self.data.code())),
            Some(Value::Enum(self.scope.code())),
            Some(Value::List(
                self.categories.iter().cloned().map(Value::String).collect(),
            )),
            (!self.group.is_empty()).then(|| Value::String(self.group.clone())),
            Some(Value::I64(self.order)),
            self.default.as_ref().map(Stored::to_value),
            self.rule.as_ref().map(Rule::to_value),
        ]
    }

    /// Whether values of this parameter are set on objects of type key `key`: elements of a
    /// bound category in instance scope, types of a bound category in type scope.
    #[must_use]
    pub fn set_on(&self, key: &str) -> bool {
        match self.scope {
            Scope::Instance => self.categories.iter().any(|c| c == key),
            Scope::Type => TypeKind::of_key(key)
                .is_some_and(|k| self.categories.iter().any(|c| c == k.instance_key())),
        }
    }

    fn node<'a>(&'a self, value: Option<&'a Stored>) -> Node<'a> {
        Node {
            key: self.id,
            identifier: &self.identifier,
            name: &self.name,
            data: self.data,
            value,
        }
    }
}

/// Whether `key` is a category a parameter can bind.
#[must_use]
pub fn is_category(key: &str) -> bool {
    PARAMETER_HOSTS.contains(&key)
}

/// The fields of a parameter definition.
#[must_use]
pub fn fields() -> Vec<FieldDecl> {
    let text_list = |name: &str, max: usize| {
        FieldDecl::new(name, ValueType::List(Box::new(ValueType::String)))
            .with_bound(Bound::Length { max: max as u64 })
            .optional()
            .in_aspect("data")
    };
    vec![
        types::name_field("identity"),
        variable::identifier_field(),
        variable::data_type_field(),
        FieldDecl::new(
            "scope",
            ValueType::Enum(
                Scope::ALL
                    .iter()
                    .map(|s| EnumVariant::new(s.name(), s.code()))
                    .collect(),
            ),
        )
        .with_default(Value::Enum(Scope::Instance.code()))
        .in_aspect("data"),
        text_list("categories", MAX_CATEGORIES),
        FieldDecl::new("group", ValueType::String)
            .optional()
            .with_bound(Bound::Length { max: MAX_GROUP })
            .in_aspect("identity"),
        FieldDecl::new("order", ValueType::I64)
            .with_default(Value::I64(0))
            .in_aspect("identity"),
        FieldDecl::new("default", ValueType::Record(expr::stored_fields()))
            .optional()
            .in_aspect("data"),
        FieldDecl::new(
            "rule",
            ValueType::Record(vec![
                FieldDecl::new("minimum", ValueType::F64).optional(),
                FieldDecl::new("maximum", ValueType::F64).optional(),
                text_list("allowed", MAX_ALLOWED),
                FieldDecl::new("required", ValueType::Bool).optional(),
                FieldDecl::new("reject", ValueType::Bool).optional(),
            ]),
        )
        .optional()
        .in_aspect("data"),
    ]
}

/// The `parameters` field of a physical element or element type, appended after the latest
/// field of `fields`: a keyed list of (parameter, stored value); optional, so older files load
/// unchanged. A deleted parameter's entries clear.
#[must_use]
pub fn values_field(fields: &[FieldDecl]) -> FieldDecl {
    let minor = fields.iter().map(|f| f.since_minor).max().unwrap_or(0) + 1;
    let mut target = types::reference_to(PARAMETER);
    target.targets = TypeKey::new(PARAMETER).into_iter().collect();
    target.rule.on_delete = OnDelete::Clear;
    target.rule.cardinality = Cardinality::Optional;
    FieldDecl::new(
        "parameters",
        ValueType::List(Box::new(ValueType::Record(vec![
            FieldDecl::new("parameter", ValueType::Reference(target)).optional(),
            FieldDecl::new("value", ValueType::Record(expr::stored_fields())).optional(),
        ]))),
    )
    .with_bound(Bound::Length {
        max: expr::MAX_DEFINITIONS as u64,
    })
    .optional()
    .in_aspect("data")
    .since_minor(minor)
}

/// Every parameter of the document, by group, order, folded name and identifier.
#[must_use]
pub fn all(snapshot: &Snapshot) -> Vec<Parameter> {
    let mut found: Vec<Parameter> = spatial::objects(snapshot, PARAMETER)
        .iter()
        .map(|o| Parameter::of(o))
        .collect();
    found.sort_by(|a, b| {
        (&a.group, a.order, crate::names::fold(&a.name), a.id).cmp(&(
            &b.group,
            b.order,
            crate::names::fold(&b.name),
            b.id,
        ))
    });
    found
}

/// The position of the `parameters` field in records of type `key`.
#[must_use]
pub fn field_at(snapshot: &Snapshot, key: &str) -> Option<usize> {
    snapshot
        .registry()
        .get(key)?
        .schema
        .fields
        .iter()
        .position(|f| f.name == "parameters")
}

/// The values stored on `object`, keyed by parameter; entries of deleted parameters are skipped.
#[must_use]
pub fn stored(snapshot: &Snapshot, object: &Object) -> Vec<(ObjectId, Stored)> {
    let Some(key) = spatial::type_of(snapshot, object.id) else {
        return Vec::new();
    };
    let Some(Some(Value::List(items))) =
        field_at(snapshot, &key).and_then(|at| object.values.get(at))
    else {
        return Vec::new();
    };
    items
        .iter()
        .take(expr::MAX_DEFINITIONS)
        .filter_map(|item| match item {
            Value::Record(f) => match (f.first(), f.get(1)) {
                (Some(Some(Value::Reference(id))), Some(value)) => {
                    Some((*id, Stored::from_value(value.as_ref())?))
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// The value of the `parameters` field holding `entries`, ordered by parameter; absent when empty.
#[must_use]
pub fn stored_value(entries: &[(ObjectId, Stored)]) -> Option<Value> {
    let mut sorted: Vec<&(ObjectId, Stored)> = entries.iter().collect();
    sorted.sort_by_key(|(id, _)| *id);
    sorted.dedup_by_key(|(id, _)| *id);
    (!sorted.is_empty()).then(|| {
        Value::List(
            sorted
                .into_iter()
                .map(|(id, s)| Value::Record(vec![Some(Value::Reference(*id)), Some(s.to_value())]))
                .collect(),
        )
    })
}

/// Where a parameter's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The object's own value.
    Own,
    /// The value of the element's type.
    Type,
    /// The definition's default.
    Default,
    /// Nowhere: the type's zero.
    Unset,
}

impl Source {
    /// The name in reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::Type => "type",
            Self::Default => "default",
            Self::Unset => "unset",
        }
    }
}

/// One parameter of an object with its value.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The parameter.
    pub parameter: Parameter,
    /// Where the value came from.
    pub source: Source,
    /// The stored value read (own, type or default).
    pub stored: Option<Stored>,
    /// The value (derived).
    pub value: Evaluated,
}

/// The rows of `params` in one scope: each value from `pick`, evaluated in dependency order,
/// other names read through `outer`.
fn scope_rows(
    params: &[&Parameter],
    pick: &dyn Fn(&Parameter) -> (Source, Option<Stored>),
    outer: &dyn Fn(&str) -> Option<SolveValue>,
) -> Result<Vec<Row>> {
    let picked: Vec<(Source, Option<Stored>)> = params.iter().map(|p| pick(p)).collect();
    let nodes: Vec<Node<'_>> = params
        .iter()
        .zip(&picked)
        .map(|(p, (_, s))| p.node(s.as_ref()))
        .collect();
    let values = expr::evaluate_scope(&nodes, outer)?;
    Ok(params
        .iter()
        .zip(picked)
        .zip(values)
        .map(|((p, (source, stored)), value)| Row {
            parameter: (*p).clone(),
            source,
            stored,
            value,
        })
        .collect())
}

/// The value an object holds for `p` among `entries`, else the default.
fn own_or_default(
    p: &Parameter,
    entries: &[(ObjectId, Stored)],
    own: Source,
) -> (Source, Option<Stored>) {
    match entries.iter().find(|(id, _)| *id == p.id) {
        Some((_, s)) => (own, Some(s.clone())),
        None => match &p.default {
            Some(d) => (Source::Default, Some(d.clone())),
            None => (Source::Unset, None),
        },
    }
}

/// The parameter rows of an object of type key `key` whose own stored values are `own` and
/// whose type's are `type_values`, in the order of [`all`].
///
/// # Errors
/// A cycle or a formula error, with the argument `parameter`.
pub fn evaluate_values(
    params: &[Parameter],
    key: &str,
    own: &[(ObjectId, Stored)],
    type_values: &[(ObjectId, Stored)],
    variables: &[(Variable, Evaluated)],
) -> Result<Vec<Row>> {
    let read_variable = |name: &str| variable::lookup(variables, name);
    if let Some(kind) = TypeKind::of_key(key) {
        let bound: Vec<&Parameter> = params
            .iter()
            .filter(|p| {
                p.scope == Scope::Type && p.categories.iter().any(|c| c == kind.instance_key())
            })
            .collect();
        return scope_rows(
            &bound,
            &|p| own_or_default(p, own, Source::Own),
            &read_variable,
        );
    }
    if !is_category(key) {
        return Ok(Vec::new());
    }
    let of = |scope: Scope| -> Vec<&Parameter> {
        params
            .iter()
            .filter(|p| p.scope == scope && p.categories.iter().any(|c| c == key))
            .collect()
    };
    let type_rows = scope_rows(
        &of(Scope::Type),
        &|p| own_or_default(p, type_values, Source::Type),
        &read_variable,
    )?;
    let by_type: BTreeMap<&str, SolveValue> = type_rows
        .iter()
        .map(|r| {
            (
                r.parameter.identifier.as_str(),
                r.value.to_solve(r.parameter.data),
            )
        })
        .collect();
    let read = |name: &str| by_type.get(name).cloned().or_else(|| read_variable(name));
    let instance_rows = scope_rows(
        &of(Scope::Instance),
        &|p| own_or_default(p, own, Source::Own),
        &read,
    )?;
    let mut rows: Vec<Row> = type_rows.into_iter().chain(instance_rows).collect();
    let position = |id: ObjectId| params.iter().position(|p| p.id == id);
    rows.sort_by_key(|r| position(r.parameter.id));
    Ok(rows)
}

/// The parameter rows of `object` (an element or an element type) with their values.
///
/// # Errors
/// As [`evaluate_values`].
pub fn evaluate(
    snapshot: &Snapshot,
    object: &Object,
    variables: &[(Variable, Evaluated)],
) -> Result<Vec<Row>> {
    let Some(key) = spatial::type_of(snapshot, object.id) else {
        return Ok(Vec::new());
    };
    let own = stored(snapshot, object);
    let type_values = TypeKind::of_instance_key(&key)
        .and_then(|kind| et::type_of(snapshot, kind, &object.values))
        .map(|t| stored(snapshot, &t))
        .unwrap_or_default();
    evaluate_values(&all(snapshot), &key, &own, &type_values, variables)
}

/// Every object that can hold parameter values, with its type key: the physical elements and the
/// element types.
#[must_use]
pub fn hosts(snapshot: &Snapshot) -> Vec<(String, std::sync::Arc<Object>)> {
    PARAMETER_HOSTS
        .iter()
        .chain(types::TYPE_KEYS.iter())
        .flat_map(|key| {
            spatial::objects(snapshot, key)
                .into_iter()
                .map(move |o| ((*key).to_owned(), o))
        })
        .collect()
}

/// [`hosts`] with the members of group definitions, which the readers hide: a change that
/// rewrites every parameter value of the document must reach them too.
#[must_use]
pub fn hosts_raw(snapshot: &Snapshot) -> Vec<(String, std::sync::Arc<Object>)> {
    PARAMETER_HOSTS
        .iter()
        .chain(types::TYPE_KEYS.iter())
        .flat_map(|key| {
            spatial::objects_raw(snapshot, key)
                .into_iter()
                .map(move |o| ((*key).to_owned(), o))
        })
        .collect()
}

/// The most objects one refusal names.
const MAX_NAMED: usize = 20;

/// Evaluates every variable and every parameter value of the document, the defaults of each
/// bound category included, and checks the rules.
///
/// # Errors
/// The first failure, with the arguments `object` (its name) and `objects` (the names of up to
/// 20 objects that fail) and `count`: a cycle, a formula error, or a broken reject rule
/// (`ab.model.param_rule`).
pub fn check(snapshot: &Snapshot) -> Result<Vec<Error>> {
    let variables = variable::evaluate(snapshot)?;
    let params = all(snapshot);
    if params.is_empty() {
        return Ok(Vec::new());
    }
    let mut failing: Vec<(String, Error)> = Vec::new();
    let mut warnings = Vec::new();
    // The defaults alone, per bound category and per type kind.
    let mut keys: Vec<&str> = params
        .iter()
        .flat_map(|p| p.categories.iter().map(String::as_str))
        .collect();
    keys.extend(TypeKind::ALL.iter().map(|k| k.key()));
    keys.sort_unstable();
    keys.dedup();
    for key in keys {
        evaluate_values(&params, key, &[], &[], &variables)?;
    }
    for (key, object) in hosts(snapshot) {
        if !params
            .iter()
            .any(|p| p.categories.contains(&key) || p.set_on(&key))
        {
            continue;
        }
        let name = types::text(&object.values, 0)
            .unwrap_or_default()
            .to_owned();
        let own = stored(snapshot, &object);
        let type_values = TypeKind::of_instance_key(&key)
            .and_then(|kind| et::type_of(snapshot, kind, &object.values))
            .map(|t| stored(snapshot, &t))
            .unwrap_or_default();
        match evaluate_values(&params, &key, &own, &type_values, &variables) {
            Err(e) => failing.push((name.clone(), e.with_arg("object", name))),
            Ok(rows) => {
                for row in rows {
                    let Some(rule) = &row.parameter.rule else {
                        continue;
                    };
                    let value = (row.source != Source::Unset).then_some(&row.value);
                    if let Some(why) = rule.broken(value) {
                        let error = Error::new(codes::PARAM_RULE)
                            .with_arg("parameter", row.parameter.name.as_str())
                            .with_arg("object", name.as_str())
                            .with_arg("rule", why);
                        if rule.reject {
                            failing.push((name.clone(), error));
                        } else {
                            warnings.push(error);
                        }
                    }
                }
            }
        }
    }
    let count = failing.len();
    let named = failing
        .iter()
        .map(|(n, _)| n.as_str())
        .take(MAX_NAMED)
        .collect::<Vec<_>>()
        .join(", ");
    match failing.into_iter().next() {
        Some((_, error)) => Err(error
            .with_arg("objects", named)
            .with_arg("count", i64::try_from(count).unwrap_or(i64::MAX))),
        None => Ok(warnings),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use arc_base::id::Identifier;

    use super::*;

    fn param(n: u128, identifier: &str, scope: Scope, default: Option<Stored>) -> Parameter {
        Parameter {
            id: ObjectId::from_u128(n),
            name: identifier.to_uppercase(),
            identifier: identifier.to_owned(),
            data: DataType::Length,
            scope,
            categories: vec![types::WALL.to_owned()],
            group: String::new(),
            order: 0,
            default,
            rule: None,
        }
    }

    fn formula(text: &str) -> Stored {
        Stored::Formula(expr::Formula::new(text, None).unwrap())
    }

    #[test]
    fn an_instance_reads_its_own_value_then_its_type_then_the_default() {
        let width = param(
            1,
            "w",
            Scope::Type,
            Some(Stored::Literal(Evaluated::Number(1.0))),
        );
        let depth = param(2, "d", Scope::Instance, Some(formula("w * 2")));
        let params = vec![width.clone(), depth.clone()];
        let rows = evaluate_values(&params, types::WALL, &[], &[], &[]).unwrap();
        assert_eq!(rows[1].value, Evaluated::Number(2.0));
        assert_eq!(rows[1].source, Source::Default);
        let typed = [(width.id, Stored::Literal(Evaluated::Number(3.0)))];
        let rows = evaluate_values(&params, types::WALL, &[], &typed, &[]).unwrap();
        assert_eq!(
            (rows[0].source, &rows[1].value),
            (Source::Type, &Evaluated::Number(6.0))
        );
        let own = [(depth.id, Stored::Literal(Evaluated::Number(0.5)))];
        let rows = evaluate_values(&params, types::WALL, &own, &typed, &[]).unwrap();
        assert_eq!(
            (rows[1].source, &rows[1].value),
            (Source::Own, &Evaluated::Number(0.5))
        );
        // A type formula never reads an instance parameter.
        let bad = param(3, "t", Scope::Type, Some(formula("d + 1 m")));
        let error = evaluate_values(&[width, depth, bad], types::WALL, &[], &[], &[]).unwrap_err();
        assert_eq!(error.code(), "solve.expr.unknown_name");
    }

    #[test]
    fn rules_name_what_breaks_them() {
        let rule = Rule {
            minimum: Some(0.5),
            maximum: Some(2.0),
            allowed: vec!["A".into()],
            required: true,
            reject: true,
        };
        assert_eq!(rule.broken(None).as_deref(), Some("required"));
        assert_eq!(
            rule.broken(Some(&Evaluated::Number(0.1))).as_deref(),
            Some("minimum 0.5")
        );
        assert_eq!(
            rule.broken(Some(&Evaluated::Number(3.0))).as_deref(),
            Some("maximum 2")
        );
        assert_eq!(rule.broken(Some(&Evaluated::Number(1.0))), None);
        assert_eq!(
            rule.broken(Some(&Evaluated::Text("B".into()))).as_deref(),
            Some("allowed")
        );
        assert_eq!(Rule::from_value(Some(&rule.to_value())), Some(rule));
    }
}
