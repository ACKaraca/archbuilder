//! The commands of document variables (ADR 0009): `archbuilder.variable.create`, `set`,
//! `rename` and `delete`. Each change is one transaction, so one undo step, and every formula of
//! the document is evaluated again before it commits: a change that leaves a formula failing or
//! closes a cycle is refused with the parameter, the span and the reason, and the document is
//! unchanged. Renaming a formula identifier rewrites every formula that reads it in the same
//! transaction.
//!
//! A value is given as a literal (`value`: a number in SI base units, a truth value, a text, or
//! for a quantity type a text such as `"900 mm"`) or as a formula (`formula`, typed with the
//! decimal separator of the session's language and stored with `.`, with `unit`, the unit bare
//! numbers take; by default the project's length unit for lengths, square and cubic meters,
//! degrees, and none for dimensionless types).

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{Accept, PromptInput, PromptSequence, PromptStep};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::error::codes as model_codes;
use archbuilder_model::expr::{self, DataType, Evaluated, Formula, Stored};
use archbuilder_model::parameter as project_parameter;
use archbuilder_model::spatial;
use archbuilder_model::types::VARIABLE;
use archbuilder_model::variable::{self, Variable};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{create, ensure_valid, expect, language};

/// A message of the catalog, named by identifier.
struct MessageId {
    message_id: &'static str,
}

/// The identifier of the create-variable command.
pub const VARIABLE_CREATE: &str = "archbuilder.variable.create";
/// The identifier of the set-variable command.
pub const VARIABLE_SET: &str = "archbuilder.variable.set";
/// The identifier of the rename-variable command.
pub const VARIABLE_RENAME: &str = "archbuilder.variable.rename";
/// The identifier of the delete-variable command.
pub const VARIABLE_DELETE: &str = "archbuilder.variable.delete";
/// The identifier of the list-variables command (read-only).
pub const VARIABLE_LIST: &str = "archbuilder.variable.list";

/// Parameters of `archbuilder.variable.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateVariable {
    /// The display name; unused by the other variables.
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
    /// The formula identifier; proposed from the name without one.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub identifier: Option<String>,
    /// The data type: `length`, `area`, `volume`, `angle`, `slope`, `number`, `integer`,
    /// `boolean`, `text`, `enumeration`, `material` or `family_type`.
    pub data_type: String,
    /// A literal value; without it and without a formula, zero, false or empty.
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
}

/// What `archbuilder.variable.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct VariableMade {
    /// The new variable.
    pub variable: IdParam,
    /// Its formula identifier.
    pub identifier: String,
}

/// Parameters of `archbuilder.variable.set`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetVariable {
    /// The variable.
    pub variable: IdParam,
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
}

/// Parameters of `archbuilder.variable.rename`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameVariable {
    /// The variable.
    pub variable: IdParam,
    /// The new display name.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    pub name: Option<String>,
    /// The new formula identifier; every formula that reads the old one is rewritten.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub identifier: Option<String>,
}

/// Parameters of `archbuilder.variable.delete`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteVariable {
    /// The variable; refused while a formula reads it.
    pub variable: IdParam,
}

/// Parameters of `archbuilder.variable.list`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListVariables {}

/// One variable as `archbuilder.variable.list` and `inspect` report it.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct VariableRow {
    /// The variable.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The formula identifier.
    pub identifier: String,
    /// The data type.
    pub data_type: String,
    /// The formula as stored, when the value is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// The unit of the formula's bare numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The value (derived for a formula): a number in SI base units, a truth value or a text;
    /// absent when the variables do not evaluate.
    pub value: Option<serde_json::Value>,
}

/// What `archbuilder.variable.list` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct VariableList {
    /// The variables, by name.
    pub variables: Vec<VariableRow>,
    /// The error code when the variables do not evaluate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// An evaluated value as JSON.
pub(crate) fn json_of(value: &Evaluated) -> serde_json::Value {
    match value {
        Evaluated::Number(v) => serde_json::Value::from(*v),
        Evaluated::Bool(b) => serde_json::Value::Bool(*b),
        Evaluated::Text(t) => serde_json::Value::String(t.clone()),
    }
}

/// The variables of `snapshot` with their values.
#[must_use]
pub fn variable_list(snapshot: &Snapshot) -> VariableList {
    let (evaluated, error) = match variable::evaluate(snapshot) {
        Ok(v) => (v, None),
        Err(e) => (Vec::new(), Some(e.code().to_owned())),
    };
    let variables = variable::all(snapshot)
        .into_iter()
        .map(|v| {
            let formula = v.value.as_ref().and_then(Stored::formula);
            VariableRow {
                id: v.id.to_string(),
                name: v.name.clone(),
                identifier: v.identifier.clone(),
                data_type: v.data.name().to_owned(),
                formula: formula.map(|f| f.text().to_owned()),
                unit: formula.and_then(|f| f.unit()).map(|u| u.symbol.to_owned()),
                value: evaluated
                    .iter()
                    .find(|(e, _)| e.id == v.id)
                    .map(|(_, value)| json_of(value)),
            }
        })
        .collect();
    VariableList { variables, error }
}

/// The data type named `name`.
pub(crate) fn data_type(name: &str) -> Result<DataType> {
    DataType::ALL
        .into_iter()
        .find(|d| d.name() == name)
        .ok_or_else(|| Error::new(codes::VALUE_INVALID))
}

/// The stored value given by `value` or `formula` for a definition of `data` named `name`;
/// `None` when neither is given.
///
/// # Errors
/// `ab.cmd.value_invalid` for both at once or an unknown unit; the formula's parse errors;
/// `ab.model.param_value_invalid` for a literal that does not fit.
pub(crate) fn stored_input(
    ctx: &Context<'_>,
    (name, data): (&str, DataType),
    value: Option<&serde_json::Value>,
    formula: Option<&str>,
    unit: Option<&str>,
) -> Result<Option<Stored>> {
    let invalid = || Error::new(model_codes::PARAM_VALUE_INVALID).with_arg("parameter", name);
    let unit = match unit {
        Some(symbol) => Some(
            arc_base::Unit::by_symbol(symbol.trim())
                .map_err(|_| Error::new(codes::VALUE_INVALID))?,
        ),
        None => variable::default_unit(ctx.snapshot(), data),
    };
    let separator = archbuilder_model::units::decimal_separator(&language(ctx));
    let constant = |text: &str| -> Result<Evaluated> {
        Formula::localized(text, unit, separator)?
            .evaluate(&|_: &str| Ok(None), data)
            .map_err(|e| e.with_arg("parameter", name))
    };
    let stored = match (value, formula) {
        (Some(_), Some(_)) => return Err(Error::new(codes::VALUE_INVALID)),
        (None, None) => return Ok(None),
        (None, Some(text)) => {
            if !data.takes_formula() {
                return Err(invalid());
            }
            Stored::Formula(
                Formula::localized(text, unit, separator)
                    .map_err(|e| e.with_arg("parameter", name))?,
            )
        }
        (Some(json), None) => Stored::Literal(match json {
            serde_json::Value::Number(n) => Evaluated::Number(n.as_f64().ok_or_else(invalid)?),
            serde_json::Value::Bool(b) => Evaluated::Bool(*b),
            serde_json::Value::String(t) if data.textual() => Evaluated::Text(t.clone()),
            serde_json::Value::String(t) => constant(t)?,
            _ => return Err(invalid()),
        }),
    };
    match &stored {
        Stored::Literal(v) if !v.fits(data) => Err(invalid()),
        _ => Ok(Some(stored)),
    }
}

/// Evaluates every formula of the open transaction's document and checks the parameter rules;
/// returns the warnings of rules that only warn, as text.
///
/// # Errors
/// The first failure: a cycle, a formula error naming its parameter and object, or a broken
/// reject rule, with the objects that fail.
pub(crate) fn ensure_formulas(ctx: &mut Context<'_>) -> Result<Vec<String>> {
    let txn = ctx.txn()?;
    Ok(archbuilder_model::parameter::check(txn.snapshot())?
        .iter()
        .map(warning_text)
        .collect())
}

/// A rule warning as text: `parameter on object: rule`.
fn warning_text(error: &Error) -> String {
    let arg = |name| match error.arguments().get(name) {
        Some(arc_base::error::Value::Text(t)) => t.clone(),
        _ => String::new(),
    };
    format!("{} on {}: {}", arg("parameter"), arg("object"), arg("rule"))
}

/// Rewrites every formula of the document that reads `from` to read `to`.
pub(crate) fn rewrite_identifier(ctx: &mut Context<'_>, from: &str, to: &str) -> Result<()> {
    let mut changes: Vec<(ObjectId, &str, Value)> = Vec::new();
    for v in variable::all(ctx.snapshot()) {
        if let Some(Stored::Formula(f)) = &v.value
            && let Some(g) = f.renamed(from, to)?
        {
            changes.push((v.id, "value", Stored::Formula(g).to_value()));
        }
    }
    let snapshot = ctx.snapshot();
    for p in project_parameter::all(snapshot) {
        if let Some(Stored::Formula(f)) = &p.default
            && let Some(g) = f.renamed(from, to)?
        {
            changes.push((p.id, "default", Stored::Formula(g).to_value()));
        }
    }
    for (_, object) in project_parameter::hosts(snapshot) {
        let mut entries = project_parameter::stored(snapshot, &object);
        let mut changed = false;
        for (_, stored) in &mut entries {
            if let Stored::Formula(f) = stored
                && let Some(g) = f.renamed(from, to)?
            {
                *stored = Stored::Formula(g);
                changed = true;
            }
        }
        if let (true, Some(value)) = (changed, project_parameter::stored_value(&entries)) {
            changes.push((object.id, "parameters", value));
        }
    }
    let txn = ctx.txn()?;
    for (id, field, value) in changes {
        txn.modify(id, FieldPath::new(field), Some(value))?;
    }
    Ok(())
}

/// The identifier `given`, checked, or one proposed from `name`; unused by the document's other
/// definitions (`except` aside).
pub(crate) fn identifier_for(
    snapshot: &Snapshot,
    name: &str,
    given: Option<&str>,
    except: Option<ObjectId>,
) -> Result<String> {
    let taken = variable::identifiers(snapshot, except);
    match given {
        Some(text) => {
            let text = text.trim();
            expr::check_identifier(text)?;
            if taken.iter().any(|t| t == text) {
                return Err(
                    Error::new(model_codes::PARAM_IDENTIFIER_TAKEN).with_arg("identifier", text)
                );
            }
            Ok(text.to_owned())
        }
        None => Ok(expr::propose_identifier(name, &|c| {
            taken.iter().any(|t| t == c)
        })),
    }
}

fn make(ctx: &mut Context<'_>, p: CreateVariable) -> Result<VariableMade> {
    let snapshot = ctx.snapshot();
    let project = spatial::project(snapshot).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    if spatial::objects(snapshot, VARIABLE).len() >= expr::MAX_DEFINITIONS {
        return Err(Error::new(model_codes::PARAM_TOO_MANY).with_arg(
            "limit",
            i64::try_from(expr::MAX_DEFINITIONS).unwrap_or(i64::MAX),
        ));
    }
    let name = p.name.trim();
    let data = data_type(&p.data_type)?;
    let identifier = identifier_for(snapshot, name, p.identifier.as_deref(), None)?;
    let stored = stored_input(
        ctx,
        (name, data),
        p.value.as_ref(),
        p.formula.as_deref(),
        p.unit.as_deref(),
    )?
    .unwrap_or_else(|| Stored::Literal(data.zero()));
    let id = create(
        ctx,
        VARIABLE,
        Some(project.id),
        variable::values(name, &identifier, data, &stored),
    )?;
    ensure_valid(ctx)?;
    ensure_formulas(ctx)?;
    Ok(VariableMade {
        variable: IdParam(id),
        identifier,
    })
}

fn set(ctx: &mut Context<'_>, p: SetVariable) -> Result<()> {
    let v = Variable::of(&expect(ctx.snapshot(), p.variable.0, VARIABLE)?);
    let stored = stored_input(
        ctx,
        (&v.name, v.data),
        p.value.as_ref(),
        p.formula.as_deref(),
        p.unit.as_deref(),
    )?
    .ok_or_else(|| Error::new(codes::VALUE_INVALID))?;
    ctx.txn()?
        .modify(v.id, FieldPath::new("value"), Some(stored.to_value()))?;
    ensure_valid(ctx)?;
    ensure_formulas(ctx).map(|_| ())
}

fn rename(ctx: &mut Context<'_>, p: RenameVariable) -> Result<()> {
    let v = Variable::of(&expect(ctx.snapshot(), p.variable.0, VARIABLE)?);
    if p.name.is_none() && p.identifier.is_none() {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    if let Some(name) = &p.name {
        ctx.txn()?.modify(
            v.id,
            FieldPath::new("name"),
            Some(Value::String(name.trim().to_owned())),
        )?;
    }
    if let Some(given) = &p.identifier {
        let identifier = identifier_for(ctx.snapshot(), &v.name, Some(given), Some(v.id))?;
        if identifier != v.identifier {
            rewrite_identifier(ctx, &v.identifier, &identifier)?;
            ctx.txn()?.modify(
                v.id,
                FieldPath::new("identifier"),
                Some(Value::String(identifier)),
            )?;
        }
    }
    ensure_valid(ctx)?;
    ensure_formulas(ctx).map(|_| ())
}

fn delete(ctx: &mut Context<'_>, p: DeleteVariable) -> Result<()> {
    expect(ctx.snapshot(), p.variable.0, VARIABLE)?;
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    txn.delete(p.variable.0)?;
    txn.apply_delete_rules(&limits)?;
    ensure_valid(ctx)?;
    ensure_formulas(ctx).map(|_| ())
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the command signature returns a result"
)]
fn list(ctx: &mut Context<'_>, _: ListVariables) -> Result<VariableList> {
    Ok(variable_list(ctx.snapshot()))
}

/// A text prompt of one to `max` characters.
pub(crate) fn text_accept(max: usize) -> Accept {
    Accept::Text {
        min_chars: 1,
        max_chars: Some(max),
    }
}

/// The prompt of a data type by name; `length` by default.
pub(crate) fn data_type_step() -> PromptStep {
    PromptStep::new(
        "data_type",
        MessageId {
            message_id: "ab-prompt-data-type",
        }
        .message_id,
        text_accept(32),
    )
    .with_default(PromptInput::Text(DataType::Length.name().to_owned()))
    .with_check(|_, input| match input {
        PromptInput::Text(name) => data_type(name.trim()).map(|_| ()),
        _ => Ok(()),
    })
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
            VARIABLE_CREATE,
            MessageId {
                message_id: "ab-cmd-variable-create-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-variable-create-description",
            }
            .message_id,
            make,
        )
        .with_prompts(
            PromptSequence::new()
                .step(PromptStep::new(
                    "name",
                    MessageId {
                        message_id: "ab-prompt-variable-name",
                    }
                    .message_id,
                    text_accept(256),
                ))
                .step(data_type_step())
                .step(PromptStep::new(
                    "formula",
                    MessageId {
                        message_id: "ab-prompt-variable-formula",
                    }
                    .message_id,
                    text_accept(2048),
                )),
        ),
    )?;
    registry.register(CommandSpec::new(
        VARIABLE_SET,
        MessageId {
            message_id: "ab-cmd-variable-set-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-variable-set-description",
        }
        .message_id,
        set,
    ))?;
    registry.register(CommandSpec::new(
        VARIABLE_RENAME,
        MessageId {
            message_id: "ab-cmd-variable-rename-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-variable-rename-description",
        }
        .message_id,
        rename,
    ))?;
    registry.register(CommandSpec::new(
        VARIABLE_DELETE,
        MessageId {
            message_id: "ab-cmd-variable-delete-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-variable-delete-description",
        }
        .message_id,
        delete,
    ))?;
    registry.register(
        CommandSpec::new(
            VARIABLE_LIST,
            MessageId {
                message_id: "ab-cmd-variable-list-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-variable-list-description",
            }
            .message_id,
            list,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read]),
    )?;
    Ok(())
}
