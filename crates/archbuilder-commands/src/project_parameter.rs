//! The commands of project parameters (ADR 0009): `archbuilder.parameter.create`, `update`,
//! `rename`, `delete`, `set` (values on elements or types) and `list` (read-only). Each change is
//! one transaction, so one undo step, and every formula and reject rule of the document is checked
//! before it commits; the rules that only warn come back as `warnings`.
//!
//! A parameter binds categories (element type keys such as `archbuilder.wall`); in instance scope
//! its values are set on the elements, in type scope on their element types. Changing the scope
//! keeps every stored value: going to type scope, a type without a value takes the value of its
//! first instance that has one; going to instance scope, an instance without a value takes its
//! type's. The values left behind stay stored and apply again when the scope changes back.

use std::collections::BTreeSet;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{PromptSequence, PromptStep};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::element_type::{self as et, TypeKind};
use archbuilder_model::error::codes as model_codes;
use archbuilder_model::expr::{self, Stored};
use archbuilder_model::parameter::{self, Parameter, Rule, Scope};
use archbuilder_model::spatial;
use archbuilder_model::types::PARAMETER;
use archbuilder_model::variable;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::parameter::{
    data_type, data_type_step, ensure_formulas, identifier_for, json_of, rewrite_identifier,
    stored_input, text_accept,
};
use crate::structure::{create, ensure_valid, expect};

/// A message of the catalog, named by identifier.
struct MessageId {
    message_id: &'static str,
}

/// The identifier of the create-parameter command.
pub const PARAMETER_CREATE: &str = "archbuilder.parameter.create";
/// The identifier of the update-parameter command.
pub const PARAMETER_UPDATE: &str = "archbuilder.parameter.update";
/// The identifier of the rename-parameter command.
pub const PARAMETER_RENAME: &str = "archbuilder.parameter.rename";
/// The identifier of the delete-parameter command.
pub const PARAMETER_DELETE: &str = "archbuilder.parameter.delete";
/// The identifier of the set-parameter-value command.
pub const PARAMETER_SET: &str = "archbuilder.parameter.set";
/// The identifier of the list-parameters command (read-only).
pub const PARAMETER_LIST: &str = "archbuilder.parameter.list";

/// The prefix of a parameter's field in `archbuilder.property.set`: `parameter:<identifier>`.
pub const FIELD_PREFIX: &str = "parameter:";

/// A rule as given.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleParam {
    /// The least number, in SI base units.
    #[serde(default)]
    pub minimum: Option<f64>,
    /// The greatest number, in SI base units.
    #[serde(default)]
    pub maximum: Option<f64>,
    /// The texts a value may be.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub allowed: Vec<String>,
    /// Whether every bound object needs a value.
    #[serde(default)]
    pub required: bool,
    /// Whether a broken rule refuses the change; otherwise it warns.
    #[serde(default)]
    pub reject: bool,
}

impl RuleParam {
    fn rule(self) -> Rule {
        Rule {
            minimum: self.minimum,
            maximum: self.maximum,
            allowed: self.allowed,
            required: self.required,
            reject: self.reject,
        }
    }
}

/// Parameters of `archbuilder.parameter.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateParameter {
    /// The display name; unused by the other parameters.
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
    /// The formula identifier; proposed from the name without one.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub identifier: Option<String>,
    /// The data type (see `archbuilder.variable.create`).
    pub data_type: String,
    /// `instance` (the default) or `type`.
    #[serde(default)]
    pub scope: Option<String>,
    /// The bound categories (element type keys); without them, those of the selected elements.
    #[serde(default)]
    #[schemars(length(max = 64))]
    pub categories: Option<Vec<String>>,
    /// The group it is shown in.
    #[serde(default)]
    #[schemars(length(max = 64))]
    pub group: Option<String>,
    /// The order within the group.
    #[serde(default)]
    pub order: Option<i64>,
    /// A literal default.
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    /// A default formula.
    #[serde(default)]
    #[schemars(length(max = 2048))]
    pub formula: Option<String>,
    /// The symbol of the unit a formula's bare numbers take.
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub unit: Option<String>,
    /// The rule.
    #[serde(default)]
    pub rule: Option<RuleParam>,
}

/// What `archbuilder.parameter.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ParameterMade {
    /// The new parameter.
    pub parameter: IdParam,
    /// Its formula identifier.
    pub identifier: String,
    /// The rules that warn.
    pub warnings: Vec<String>,
}

/// Parameters of `archbuilder.parameter.update`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateParameter {
    /// The parameter.
    pub parameter: IdParam,
    /// `instance` or `type`; the stored values are kept.
    #[serde(default)]
    pub scope: Option<String>,
    /// The bound categories.
    #[serde(default)]
    #[schemars(length(max = 64))]
    pub categories: Option<Vec<String>>,
    /// The group; an empty text is the common group.
    #[serde(default)]
    #[schemars(length(max = 64))]
    pub group: Option<String>,
    /// The order within the group.
    #[serde(default)]
    pub order: Option<i64>,
    /// A literal default.
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    /// A default formula.
    #[serde(default)]
    #[schemars(length(max = 2048))]
    pub formula: Option<String>,
    /// The symbol of the unit a formula's bare numbers take.
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub unit: Option<String>,
    /// Removes the default.
    #[serde(default)]
    pub clear_default: bool,
    /// The rule.
    #[serde(default)]
    pub rule: Option<RuleParam>,
    /// Removes the rule.
    #[serde(default)]
    pub clear_rule: bool,
}

/// What `archbuilder.parameter.update` and `set` return.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct Checked {
    /// How many values a scope change copied between types and instances.
    pub copied: usize,
    /// The elements whose own value differs from the one their type took when the scope changed
    /// to type; their values stay stored.
    pub diverging: Vec<IdParam>,
    /// The rules that warn.
    pub warnings: Vec<String>,
}

/// Parameters of `archbuilder.parameter.rename`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameParameter {
    /// The parameter.
    pub parameter: IdParam,
    /// The new display name.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    pub name: Option<String>,
    /// The new formula identifier; every formula that reads the old one is rewritten.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub identifier: Option<String>,
}

/// Parameters of `archbuilder.parameter.delete`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteParameter {
    /// The parameter; refused while another formula reads it. Its stored values go with it.
    pub parameter: IdParam,
}

/// Parameters of `archbuilder.parameter.set`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetParameter {
    /// The elements (instance scope) or element types (type scope).
    #[schemars(length(min = 1, max = 10000))]
    pub objects: Vec<IdParam>,
    /// The parameter.
    pub parameter: IdParam,
    /// A literal value.
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    /// A formula.
    #[serde(default)]
    #[schemars(length(max = 2048))]
    pub formula: Option<String>,
    /// The symbol of the unit a formula's bare numbers take.
    #[serde(default)]
    #[schemars(length(max = 32))]
    pub unit: Option<String>,
    /// Removes the objects' own values, so they read their type's or the default.
    #[serde(default)]
    pub clear: bool,
}

/// Parameters of `archbuilder.parameter.list`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParameters {
    /// The objects whose values to list; without them, every object with a value or a bound
    /// category.
    #[serde(default)]
    #[schemars(length(max = 10000))]
    pub objects: Option<Vec<IdParam>>,
}

/// One parameter definition as listed.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ParameterRow {
    /// The parameter.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The formula identifier.
    pub identifier: String,
    /// The data type.
    pub data_type: String,
    /// `instance` or `type`.
    pub scope: String,
    /// The bound categories.
    pub categories: Vec<String>,
    /// The group.
    pub group: String,
    /// The order within the group.
    pub order: i64,
    /// The default formula, when it is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_formula: Option<String>,
}

/// One value of one object as listed.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ValueRow {
    /// The object.
    pub object: String,
    /// The parameter's formula identifier.
    pub parameter: String,
    /// `own`, `type`, `default` or `unset`.
    pub source: String,
    /// The formula read, when it is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// The value (derived for a formula).
    pub value: serde_json::Value,
}

/// What `archbuilder.parameter.list` and `inspect` report.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ParameterList {
    /// The definitions, by group, order and name.
    pub parameters: Vec<ParameterRow>,
    /// The values of the objects, by object and parameter order.
    pub values: Vec<ValueRow>,
    /// The error code of an object that does not evaluate.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// The parameters of `snapshot` and the values of `objects` (every host without a list).
#[must_use]
pub fn parameter_list(snapshot: &Snapshot, objects: Option<&[ObjectId]>) -> ParameterList {
    let all = parameter::all(snapshot);
    let parameters = all
        .iter()
        .map(|p| ParameterRow {
            id: p.id.to_string(),
            name: p.name.clone(),
            identifier: p.identifier.clone(),
            data_type: p.data.name().to_owned(),
            scope: p.scope.name().to_owned(),
            categories: p.categories.clone(),
            group: p.group.clone(),
            order: p.order,
            default_formula: p
                .default
                .as_ref()
                .and_then(Stored::formula)
                .map(|f| f.text().to_owned()),
        })
        .collect();
    let mut values = Vec::new();
    let mut errors = Vec::new();
    if all.is_empty() {
        return ParameterList {
            parameters,
            values,
            errors,
        };
    }
    let variables = variable::evaluate(snapshot).unwrap_or_default();
    let mut hosts: Vec<_> = parameter::hosts(snapshot)
        .into_iter()
        .map(|(_, o)| o)
        .filter(|o| objects.is_none_or(|list| list.contains(&o.id)))
        .collect();
    hosts.sort_by_key(|o| o.id);
    for object in hosts {
        match parameter::evaluate(snapshot, &object, &variables) {
            Ok(rows) => values.extend(rows.into_iter().map(|r| {
                ValueRow {
                    object: object.id.to_string(),
                    parameter: r.parameter.identifier.clone(),
                    source: r.source.name().to_owned(),
                    formula: r
                        .stored
                        .as_ref()
                        .and_then(Stored::formula)
                        .map(|f| f.text().to_owned()),
                    value: json_of(&r.value),
                }
            })),
            Err(e) => errors.push(format!("{}: {}", object.id, e.code())),
        }
    }
    ParameterList {
        parameters,
        values,
        errors,
    }
}

fn scope_of(name: Option<&str>) -> Result<Option<Scope>> {
    name.map(|n| {
        Scope::ALL
            .into_iter()
            .find(|s| s.name() == n)
            .ok_or_else(|| Error::new(codes::VALUE_INVALID))
    })
    .transpose()
}

fn checked_categories(name: &str, categories: Vec<String>) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for c in categories {
        let c = c.trim().to_owned();
        if !parameter::is_category(&c) {
            return Err(Error::new(model_codes::PARAM_CATEGORY).with_arg("parameter", name));
        }
        if !out.contains(&c) {
            out.push(c);
        }
    }
    if out.is_empty() || out.len() > parameter::MAX_CATEGORIES {
        return Err(Error::new(model_codes::PARAM_CATEGORY).with_arg("parameter", name));
    }
    Ok(out)
}

/// Writes the stored values `entries` of `object`.
fn write_entries(
    ctx: &mut Context<'_>,
    object: ObjectId,
    entries: &[(ObjectId, Stored)],
) -> Result<()> {
    ctx.txn()?.modify(
        object,
        FieldPath::new("parameters"),
        parameter::stored_value(entries),
    )?;
    Ok(())
}

fn make(ctx: &mut Context<'_>, p: CreateParameter) -> Result<ParameterMade> {
    let snapshot = ctx.snapshot();
    let project = spatial::project(snapshot).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    if spatial::objects(snapshot, PARAMETER).len() >= expr::MAX_DEFINITIONS {
        return Err(Error::new(model_codes::PARAM_TOO_MANY).with_arg(
            "limit",
            i64::try_from(expr::MAX_DEFINITIONS).unwrap_or(i64::MAX),
        ));
    }
    let name = p.name.trim().to_owned();
    let data = data_type(&p.data_type)?;
    let categories = match p.categories {
        Some(list) => list,
        None => ctx
            .selection()
            .iter()
            .filter_map(|id| spatial::type_of(snapshot, *id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    };
    let categories = checked_categories(&name, categories)?;
    let identifier = identifier_for(snapshot, &name, p.identifier.as_deref(), None)?;
    let default = stored_input(
        ctx,
        (&name, data),
        p.value.as_ref(),
        p.formula.as_deref(),
        p.unit.as_deref(),
    )?;
    let definition = Parameter {
        // The record gets its own identifier; the definition is only written.
        id: arc_base::id::Identifier::from_u128(0),
        name,
        identifier: identifier.clone(),
        data,
        scope: scope_of(p.scope.as_deref())?.unwrap_or_default(),
        categories,
        group: p.group.unwrap_or_default().trim().to_owned(),
        order: p.order.unwrap_or(0),
        default,
        rule: p.rule.map(RuleParam::rule),
    };
    let id = create(ctx, PARAMETER, Some(project.id), definition.values())?;
    ensure_valid(ctx)?;
    let warnings = ensure_formulas(ctx)?;
    Ok(ParameterMade {
        parameter: IdParam(id),
        identifier,
        warnings,
    })
}

/// Moves values between instances and types for a change of `p`'s scope to `to`, keeping every
/// stored value; returns how many values were copied and, going to type scope, the elements
/// whose own value differs from the one their type takes.
fn move_scope(ctx: &mut Context<'_>, p: &Parameter, to: Scope) -> Result<(usize, Vec<ObjectId>)> {
    let kinds: Vec<TypeKind> = p
        .categories
        .iter()
        .filter_map(|c| TypeKind::of_instance_key(c))
        .collect();
    let mut writes: Vec<(ObjectId, Vec<(ObjectId, Stored)>)> = Vec::new();
    let mut diverging = Vec::new();
    let snapshot = ctx.snapshot();
    let own_value = |o: &arc_doc::Object| {
        parameter::stored(snapshot, o)
            .into_iter()
            .find(|(id, _)| *id == p.id)
    };
    for kind in kinds {
        for t in et::of_kind(snapshot, kind) {
            let mut type_entries = parameter::stored(snapshot, &t);
            let type_value = type_entries.iter().find(|(id, _)| *id == p.id).cloned();
            let mut instances = et::instances_raw(snapshot, kind, t.id);
            instances.sort_by_key(|o| o.id);
            match to {
                Scope::Type => {
                    let taken = match type_value {
                        Some(entry) => Some(entry),
                        None => {
                            let first = instances.iter().find_map(|o| own_value(o));
                            if let Some(entry) = &first {
                                type_entries.push(entry.clone());
                                writes.push((t.id, type_entries));
                            }
                            first
                        }
                    };
                    if let Some((_, value)) = taken {
                        diverging.extend(
                            instances
                                .iter()
                                .filter(|o| own_value(o).is_some_and(|(_, v)| v != value))
                                .filter(|o| {
                                    archbuilder_model::group::owner_definition(snapshot, o.id)
                                        .is_none()
                                })
                                .map(|o| o.id),
                        );
                    }
                }
                Scope::Instance => {
                    let Some(entry) = type_value else { continue };
                    for o in instances {
                        let mut own = parameter::stored(snapshot, &o);
                        if !own.iter().any(|(id, _)| *id == p.id) {
                            own.push(entry.clone());
                            writes.push((o.id, own));
                        }
                    }
                }
            }
        }
    }
    let copied = writes.len();
    let before = crate::group::fingerprints(ctx.snapshot(), writes.iter().map(|(id, _)| *id));
    for (id, entries) in writes {
        write_entries(ctx, id, &entries)?;
    }
    crate::group::accept_derived(ctx, &before)?;
    Ok((copied, diverging))
}

fn update(ctx: &mut Context<'_>, u: UpdateParameter) -> Result<Checked> {
    let p = Parameter::of(&expect(ctx.snapshot(), u.parameter.0, PARAMETER)?);
    let (mut copied, mut diverging) = (0, Vec::new());
    let mut changes: Vec<(&str, Option<Value>)> = Vec::new();
    if let Some(scope) = scope_of(u.scope.as_deref())?
        && scope != p.scope
    {
        (copied, diverging) = move_scope(ctx, &p, scope)?;
        changes.push(("scope", Some(Value::Enum(scope.code()))));
    }
    if let Some(categories) = u.categories {
        let list = checked_categories(&p.name, categories)?;
        changes.push((
            "categories",
            Some(Value::List(list.into_iter().map(Value::String).collect())),
        ));
    }
    if let Some(group) = u.group {
        let group = group.trim().to_owned();
        changes.push(("group", (!group.is_empty()).then_some(Value::String(group))));
    }
    if let Some(order) = u.order {
        changes.push(("order", Some(Value::I64(order))));
    }
    if u.clear_default {
        changes.push(("default", None));
    } else if let Some(stored) = stored_input(
        ctx,
        (&p.name, p.data),
        u.value.as_ref(),
        u.formula.as_deref(),
        u.unit.as_deref(),
    )? {
        changes.push(("default", Some(stored.to_value())));
    }
    if u.clear_rule {
        changes.push(("rule", None));
    } else if let Some(rule) = u.rule {
        changes.push(("rule", Some(rule.rule().to_value())));
    }
    let txn = ctx.txn()?;
    for (field, value) in changes {
        txn.modify(p.id, FieldPath::new(field), value)?;
    }
    ensure_valid(ctx)?;
    let warnings = ensure_formulas(ctx)?;
    Ok(Checked {
        copied,
        diverging: diverging.into_iter().map(IdParam).collect(),
        warnings,
    })
}

fn rename(ctx: &mut Context<'_>, r: RenameParameter) -> Result<()> {
    let p = Parameter::of(&expect(ctx.snapshot(), r.parameter.0, PARAMETER)?);
    if r.name.is_none() && r.identifier.is_none() {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    if let Some(name) = &r.name {
        ctx.txn()?.modify(
            p.id,
            FieldPath::new("name"),
            Some(Value::String(name.trim().to_owned())),
        )?;
    }
    if let Some(given) = &r.identifier {
        let identifier = identifier_for(ctx.snapshot(), &p.name, Some(given), Some(p.id))?;
        if identifier != p.identifier {
            rewrite_identifier(ctx, &p.identifier, &identifier)?;
            ctx.txn()?.modify(
                p.id,
                FieldPath::new("identifier"),
                Some(Value::String(identifier)),
            )?;
        }
    }
    ensure_valid(ctx)?;
    ensure_formulas(ctx).map(|_| ())
}

fn delete(ctx: &mut Context<'_>, d: DeleteParameter) -> Result<()> {
    expect(ctx.snapshot(), d.parameter.0, PARAMETER)?;
    let snapshot = ctx.snapshot();
    let writes: Vec<(ObjectId, Vec<(ObjectId, Stored)>)> = parameter::hosts_raw(snapshot)
        .into_iter()
        .filter_map(|(_, o)| {
            let entries = parameter::stored(snapshot, &o);
            let kept: Vec<_> = entries
                .iter()
                .filter(|(id, _)| *id != d.parameter.0)
                .cloned()
                .collect();
            (kept.len() != entries.len()).then_some((o.id, kept))
        })
        .collect();
    let before = crate::group::fingerprints(ctx.snapshot(), writes.iter().map(|(id, _)| *id));
    for (id, entries) in writes {
        write_entries(ctx, id, &entries)?;
    }
    crate::group::accept_derived(ctx, &before)?;
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    txn.delete(d.parameter.0)?;
    txn.apply_delete_rules(&limits)?;
    ensure_valid(ctx)?;
    ensure_formulas(ctx).map(|_| ())
}

/// Sets (or with `stored` absent, clears) the value of `p` on `objects`.
pub(crate) fn set_values(
    ctx: &mut Context<'_>,
    objects: &[ObjectId],
    p: &Parameter,
    stored: Option<&Stored>,
) -> Result<Vec<String>> {
    for &id in objects {
        let snapshot = ctx.snapshot();
        let key = spatial::type_of(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        if !p.set_on(&key) {
            return Err(
                Error::new(model_codes::PARAM_CATEGORY).with_arg("parameter", p.name.as_str())
            );
        }
        let object = spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let mut entries: Vec<(ObjectId, Stored)> = parameter::stored(snapshot, &object)
            .into_iter()
            .filter(|(pid, _)| *pid != p.id)
            .collect();
        if let Some(stored) = stored {
            entries.push((p.id, stored.clone()));
        }
        write_entries(ctx, id, &entries)?;
    }
    ensure_valid(ctx)?;
    ensure_formulas(ctx)
}

fn set(ctx: &mut Context<'_>, s: SetParameter) -> Result<Checked> {
    let p = Parameter::of(&expect(ctx.snapshot(), s.parameter.0, PARAMETER)?);
    let stored = if s.clear {
        None
    } else {
        Some(
            stored_input(
                ctx,
                (&p.name, p.data),
                s.value.as_ref(),
                s.formula.as_deref(),
                s.unit.as_deref(),
            )?
            .ok_or_else(|| Error::new(codes::VALUE_INVALID))?,
        )
    };
    let objects: Vec<ObjectId> = s.objects.iter().map(|IdParam(id)| *id).collect();
    let warnings = set_values(ctx, &objects, &p, stored.as_ref())?;
    Ok(Checked {
        warnings,
        ..Checked::default()
    })
}

/// `archbuilder.property.set` of the field `parameter:<identifier>`: a text that starts with `=`
/// is a formula, an empty text clears the objects' own values, any other value is a literal.
pub(crate) fn set_property(
    ctx: &mut Context<'_>,
    objects: &[ObjectId],
    identifier: &str,
    value: &serde_json::Value,
) -> Result<()> {
    let p = parameter::all(ctx.snapshot())
        .into_iter()
        .find(|p| p.identifier == identifier)
        .ok_or_else(|| Error::new(codes::FIELD_UNKNOWN))?;
    let stored = match value {
        serde_json::Value::String(t) if t.trim().is_empty() => None,
        serde_json::Value::String(t) if t.trim_start().starts_with('=') => stored_input(
            ctx,
            (&p.name, p.data),
            None,
            t.trim_start().strip_prefix('='),
            None,
        )?,
        other => stored_input(ctx, (&p.name, p.data), Some(other), None, None)?,
    };
    set_values(ctx, objects, &p, stored.as_ref()).map(|_| ())
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the command signature returns a result"
)]
fn list(ctx: &mut Context<'_>, l: ListParameters) -> Result<ParameterList> {
    let objects: Option<Vec<ObjectId>> = l
        .objects
        .map(|list| list.into_iter().map(|IdParam(id)| id).collect());
    Ok(parameter_list(ctx.snapshot(), objects.as_deref()))
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
            PARAMETER_CREATE,
            MessageId {
                message_id: "ab-cmd-parameter-create-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-parameter-create-description",
            }
            .message_id,
            make,
        )
        .with_prompts(
            PromptSequence::new()
                .step(PromptStep::new(
                    "name",
                    MessageId {
                        message_id: "ab-prompt-parameter-name",
                    }
                    .message_id,
                    text_accept(256),
                ))
                .step(data_type_step()),
        ),
    )?;
    registry.register(CommandSpec::new(
        PARAMETER_UPDATE,
        MessageId {
            message_id: "ab-cmd-parameter-update-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-parameter-update-description",
        }
        .message_id,
        update,
    ))?;
    registry.register(CommandSpec::new(
        PARAMETER_RENAME,
        MessageId {
            message_id: "ab-cmd-parameter-rename-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-parameter-rename-description",
        }
        .message_id,
        rename,
    ))?;
    registry.register(CommandSpec::new(
        PARAMETER_DELETE,
        MessageId {
            message_id: "ab-cmd-parameter-delete-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-parameter-delete-description",
        }
        .message_id,
        delete,
    ))?;
    registry.register(CommandSpec::new(
        PARAMETER_SET,
        MessageId {
            message_id: "ab-cmd-parameter-set-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-parameter-set-description",
        }
        .message_id,
        set,
    ))?;
    registry.register(
        CommandSpec::new(
            PARAMETER_LIST,
            MessageId {
                message_id: "ab-cmd-parameter-list-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-parameter-list-description",
            }
            .message_id,
            list,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read]),
    )?;
    Ok(())
}
