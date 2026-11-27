//! Families in a project (ADR 0009, unit 7): `archbuilder.family.load` copies definitions from a
//! package or another document, `archbuilder.family.update` moves a loaded family to a newer
//! version with a parameter mapping, and `archbuilder.component.place` places a freestanding
//! instance of a family type. `archbuilder.family.load_preview` and `update_preview` report
//! what would happen without changing anything. Each change is one transaction, so one undo
//! step, and nothing is applied when a component would not evaluate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::prompt::{
    Accept, KeywordOption, PointDims, Prompt, PromptInput, PromptMachine, PromptSequence,
    PromptState, PromptStep, Step as Next,
};
use arc_command::{
    CommandSpec, Context, IdParam, Permission, QuantityParam, Registry, UndoBehavior,
};
use arc_doc::package::{Mode, Package};
use arc_doc::{FieldPath, LoadOptions, Snapshot, Value};
use archbuilder_model::error::codes as model_codes;
use archbuilder_model::expr::Stored;
use archbuilder_model::family::def::{Category, Definition, Hosting, TypeDef};
use archbuilder_model::family::loaded::{
    self, Component, FamilyType, Loaded, components_of, family_types, values_value,
};
use archbuilder_model::family::package::{self, Published, Step, step};
use archbuilder_model::family::text::checksum;
use archbuilder_model::spatial;
use archbuilder_model::types::{self, COMPONENT, FAMILY, FAMILY_TYPE};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::parameter::{json_of, stored_input};
use crate::structure::{LANGUAGE_SETTING, create, ensure_valid, expect};

/// A message of the catalog, named by identifier.
struct MessageId {
    message_id: &'static str,
}

/// The identifier of the load command.
pub const FAMILY_LOAD: &str = "archbuilder.family.load";
/// The identifier of the load preview (read-only).
pub const FAMILY_LOAD_PREVIEW: &str = "archbuilder.family.load_preview";
/// The identifier of the update command.
pub const FAMILY_UPDATE: &str = "archbuilder.family.update";
/// The identifier of the update preview (read-only).
pub const FAMILY_UPDATE_PREVIEW: &str = "archbuilder.family.update_preview";
/// The identifier of the component placement command.
pub const COMPONENT_PLACE: &str = "archbuilder.component.place";

/// The identifier of the hosted placement command.
pub const FAMILY_INSERT_PLACE: &str = "archbuilder.family_insert.place";

/// The most families one load takes.
pub const MAX_FAMILIES: usize = 256;
/// The most value changes a preview lists.
pub const MAX_LISTED: usize = 200;

/// What to do with a type of the same name that the family has already.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Conflict {
    /// Keep the type the project has.
    #[default]
    Keep,
    /// Replace its values with the loaded type's.
    Replace,
    /// Load the type under another name.
    Rename,
}

/// Parameters of `archbuilder.family.load` and `load_preview`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoadFamilies {
    /// A `.archbfam` package or an `.archb` document to load from.
    #[schemars(length(min = 1, max = 4096))]
    pub path: String,
    /// The families to load, by name or identifier; all when empty.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub families: Vec<String>,
    /// The types to load, by name; all when empty.
    #[serde(default)]
    #[schemars(length(max = 5000))]
    pub types: Vec<String>,
    /// What to do with a type name the project has for that family.
    #[serde(default)]
    pub conflict: Conflict,
    /// For a family loaded with another checksum at this version: load it as a new family.
    #[serde(default)]
    pub as_new: bool,
}

/// One family of a load report.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct FamilyRow {
    /// The family name.
    pub name: String,
    /// The family identifier.
    pub family: String,
    /// The version.
    pub version: String,
    /// `added`, `unchanged`, `added_as_new` or `conflict`.
    pub action: String,
    /// For a conflict: why.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One type of a load report.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct TypeRow {
    /// The family name.
    pub family: String,
    /// The type's name in the definition.
    pub name: String,
    /// `added`, `kept`, `replaced` or `renamed`.
    pub action: String,
    /// The name it has in the project.
    pub result: String,
}

/// What a load or its preview reports.
#[derive(Debug, Serialize, JsonSchema)]
pub struct LoadReport {
    /// Whether nothing was changed.
    pub preview: bool,
    /// The families.
    pub families: Vec<FamilyRow>,
    /// The types.
    pub types: Vec<TypeRow>,
}

fn invalid() -> Error {
    Error::new(codes::VALUE_INVALID)
}

/// The published definitions of the package or document at `path`, read through the session's
/// file permission. A package is verified (checksums, texts, test cases); dependencies are
/// those of packages, and are not checked here.
fn read_source(ctx: &Context<'_>, path: &str) -> Result<Vec<Published>> {
    let path = Path::new(path);
    drop(ctx.open_read(path)?);
    let limits = *ctx.limits();
    let package = Package::open(path, Mode::ReadOnly, &limits)?;
    let loaded = package.load(
        std::sync::Arc::clone(ctx.snapshot().registry()),
        &LoadOptions {
            limits,
            ..LoadOptions::default()
        },
    )?;
    let snapshot = loaded.snapshot;
    let contents = package::read(&snapshot);
    if !contents.definitions.is_empty() {
        let refusals = package::verify(&contents, &[]);
        if let Some(r) = refusals
            .into_iter()
            .find(|r| r.error.code() != model_codes::FAMILY_DEPENDENCY.code)
        {
            return Err(r.error);
        }
        return Ok(contents.definitions);
    }
    Ok(loaded::families(&snapshot)
        .into_iter()
        .map(|f| Published {
            record: Some(f.record),
            family: f.family,
            version: f.version,
            name: f.name,
            category: String::new(),
            checksum: f.checksum,
            text: f.text,
        })
        .collect())
}

fn selected<'a>(all: &'a [Published], names: &[String]) -> Result<Vec<&'a Published>> {
    if names.is_empty() {
        return Ok(all.iter().collect());
    }
    let mut out = Vec::new();
    for n in names {
        let found: Vec<&Published> = all
            .iter()
            .filter(|p| p.name == *n || p.family.to_string() == *n)
            .collect();
        if found.is_empty() {
            return Err(invalid());
        }
        out.extend(found);
    }
    Ok(out)
}

/// What a load does to one family.
struct FamilyPlan<'a> {
    published: &'a Published,
    definition: Definition,
    /// The record that already holds this family, when it does.
    existing: Option<Loaded>,
    row: FamilyRow,
}

fn plan_families<'a>(
    snapshot: &Snapshot,
    chosen: &[&'a Published],
    as_new: bool,
) -> Result<Vec<FamilyPlan<'a>>> {
    let have = loaded::families(snapshot);
    let mut out = Vec::new();
    for p in chosen {
        let definition = Definition::from_text(&p.text)?;
        let mut row = FamilyRow {
            name: p.name.clone(),
            family: p.family.to_string(),
            version: p.version.to_string(),
            action: "added".into(),
            reason: None,
        };
        let existing = have.iter().find(|h| h.family == p.family).cloned();
        if let Some(h) = &existing {
            if h.version == p.version && h.checksum == p.checksum {
                row.action = "unchanged".into();
            } else if h.version == p.version && as_new {
                row.action = "added_as_new".into();
            } else if h.version == p.version {
                row.action = "conflict".into();
                row.reason = Some("checksum".into());
            } else {
                row.action = "conflict".into();
                row.reason = Some("version_pinned".into());
            }
        }
        out.push(FamilyPlan {
            published: p,
            definition,
            existing,
            row,
        });
    }
    Ok(out)
}

/// A name not among `taken`: `name`, else `name 2`, `name 3`, ...
fn free(name: &str, taken: &BTreeSet<String>) -> String {
    let fold = |s: &str| archbuilder_model::names::fold(s);
    let used: BTreeSet<String> = taken.iter().map(|t| fold(t)).collect();
    if !used.contains(&fold(name)) {
        return name.to_owned();
    }
    (2..)
        .map(|n| format!("{name} {n}"))
        .find(|c| !used.contains(&fold(c)))
        .unwrap_or_else(|| name.to_owned())
}

fn load_error(p: &FamilyPlan<'_>) -> Error {
    let family = p.published.name.as_str();
    match p.row.reason.as_deref() {
        Some("version_pinned") => Error::new(model_codes::FAMILY_VERSION)
            .with_arg("version", p.row.version.as_str())
            .with_arg("reason", "loaded"),
        _ => Error::new(model_codes::FAMILY_CONFLICT).with_arg("family", family),
    }
}

/// Applies or reports a load.
#[allow(
    clippy::too_many_lines,
    reason = "one pass over the families of a load"
)]
fn run_load(ctx: &mut Context<'_>, p: &LoadFamilies, apply: bool) -> Result<LoadReport> {
    let sources = read_source(ctx, &p.path)?;
    let chosen = selected(&sources, &p.families)?;
    if chosen.len() > MAX_FAMILIES {
        return Err(Error::new(model_codes::FAMILY_LIMIT).with_arg("limit", "families"));
    }
    let plans = plan_families(ctx.snapshot(), &chosen, p.as_new)?;
    let mut report = LoadReport {
        preview: !apply,
        families: plans.iter().map(|f| f.row.clone()).collect(),
        types: Vec::new(),
    };
    if apply && let Some(bad) = plans.iter().find(|f| f.row.action == "conflict") {
        return Err(load_error(bad));
    }
    let project = spatial::project(ctx.snapshot()).map(|o| o.id);
    for plan in &plans {
        // The record this family's types hang on.
        let (record, definition, text_checksum, published) = match plan.row.action.as_str() {
            "added_as_new" => {
                let mut d = plan.definition.clone();
                d.header.source = format!(
                    "{} {} {}",
                    plan.published.name, plan.published.version, plan.published.checksum
                );
                let new_id: ObjectId = if apply {
                    ctx.new_id()
                } else {
                    plan.published.family
                };
                d.header.id = new_id;
                let text = d.to_text()?;
                let sum = checksum(&text);
                let mut pubd = plan.published.clone();
                let names: BTreeSet<String> = loaded::families(ctx.snapshot())
                    .into_iter()
                    .map(|f| f.name)
                    .collect();
                pubd.name = free(&pubd.name, &names);
                pubd.family = new_id;
                pubd.text = text;
                pubd.checksum = sum.clone();
                (None, d, sum, pubd)
            }
            "unchanged" => (
                plan.existing.as_ref().map(|e| e.record),
                plan.definition.clone(),
                plan.published.checksum.clone(),
                plan.published.clone(),
            ),
            _ => (
                None,
                plan.definition.clone(),
                plan.published.checksum.clone(),
                plan.published.clone(),
            ),
        };
        let _ = text_checksum;
        let record = match (record, apply) {
            (Some(r), _) => Some(r),
            (None, true) => {
                let owner = project.ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                Some(create(
                    ctx,
                    FAMILY,
                    Some(owner),
                    Loaded::values(&published, &definition),
                )?)
            }
            (None, false) => None,
        };
        // The types: the selected ones of the definition, or a default one for a family without.
        let wanted: Vec<TypeDef> = if definition.types.is_empty() {
            vec![TypeDef {
                id: definition.header.id,
                name: published.name.clone(),
                values: BTreeMap::new(),
            }]
        } else {
            definition
                .types
                .iter()
                .filter(|t| p.types.is_empty() || p.types.contains(&t.name))
                .cloned()
                .collect()
        };
        let mine: Vec<FamilyType> = record
            .map(|r| {
                family_types(ctx.snapshot())
                    .into_iter()
                    .filter(|t| t.family == r)
                    .collect()
            })
            .unwrap_or_default();
        let mut taken: BTreeSet<String> = mine.iter().map(|t| t.name.clone()).collect();
        for t in wanted {
            let same = mine.iter().find(|m| {
                archbuilder_model::names::fold(&m.name) == archbuilder_model::names::fold(&t.name)
            });
            let (action, result) = match (same, p.conflict) {
                (None, _) => ("added", t.name.clone()),
                (Some(_), Conflict::Keep) => ("kept", t.name.clone()),
                (Some(_), Conflict::Replace) => ("replaced", t.name.clone()),
                (Some(_), Conflict::Rename) => ("renamed", free(&t.name, &taken)),
            };
            report.types.push(TypeRow {
                family: published.name.clone(),
                name: t.name.clone(),
                action: action.into(),
                result: result.clone(),
            });
            if !apply {
                continue;
            }
            let (Some(record), Some(owner)) = (record, project) else {
                continue;
            };
            match action {
                "added" | "renamed" => {
                    create(
                        ctx,
                        FAMILY_TYPE,
                        Some(owner),
                        FamilyType::record(&result, record, &t.values),
                    )?;
                    taken.insert(result);
                }
                "replaced" => {
                    if let Some(m) = same {
                        ctx.txn()?.modify(
                            m.id,
                            FieldPath::new("values"),
                            values_value(&t.values),
                        )?;
                    }
                }
                _ => {}
            }
        }
    }
    if apply {
        ensure_valid(ctx)?;
        loaded::check_components(ctx.txn()?.snapshot(), None)?;
    }
    Ok(report)
}

fn load(ctx: &mut Context<'_>, p: LoadFamilies) -> Result<LoadReport> {
    run_load(ctx, &p, true)
}

fn load_preview(ctx: &mut Context<'_>, p: LoadFamilies) -> Result<LoadReport> {
    run_load(ctx, &p, false)
}

/// Parameters of `archbuilder.family.update` and `update_preview`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateFamily {
    /// The loaded family (its record), by identifier or name.
    #[schemars(length(min = 1, max = 256))]
    pub family: String,
    /// A `.archbfam` package or an `.archb` document that holds the newer version.
    #[schemars(length(min = 1, max = 4096))]
    pub path: String,
    /// For parameters that are removed or retyped: the parameter of the new version that takes
    /// the value (by formula identifier), or null to drop it. Required for each of them.
    #[serde(default)]
    pub mapping: BTreeMap<String, Option<String>>,
    /// Types of the new version to add to the project, by name.
    #[serde(default)]
    pub add_types: Vec<String>,
}

/// One value that an update changes.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ValueChange {
    /// The type or component, by name.
    pub owner: String,
    /// The parameter, by display name.
    pub parameter: String,
    /// The value before.
    pub before: Option<serde_json::Value>,
    /// The value after.
    pub after: Option<serde_json::Value>,
}

/// An instance that would not evaluate after the update.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct Failing {
    /// The component, by mark or identifier.
    pub instance: String,
    /// The error code.
    pub reason: String,
}

/// What an update or its preview reports.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateReport {
    /// Whether nothing was changed.
    pub preview: bool,
    /// The family's name.
    pub family: String,
    /// The version it has.
    pub from: String,
    /// The version it gets.
    pub to: String,
    /// Whether the change is a major one.
    pub major: bool,
    /// Parameters of the old version the new one does not have.
    pub removed: Vec<String>,
    /// Parameters whose data type changed.
    pub retyped: Vec<String>,
    /// Parameters the new version adds.
    pub added: Vec<String>,
    /// Parameters whose formula identifier changed, `old -> new`.
    pub renamed: Vec<String>,
    /// Removed or retyped parameters that have no entry in the mapping.
    pub needs_mapping: Vec<String>,
    /// The family types the update changes.
    pub types: usize,
    /// The components of the family.
    pub instances: usize,
    /// Values that change (at most [`MAX_LISTED`]).
    pub changes: Vec<ValueChange>,
    /// Instances that would fail.
    pub failing: Vec<Failing>,
}

fn find_family(snapshot: &Snapshot, name: &str) -> Result<Loaded> {
    let all = loaded::families(snapshot);
    all.into_iter()
        .find(|f| f.record.to_string() == name || f.family.to_string() == name || f.name == name)
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH).with_arg("expected", FAMILY))
}

/// Moves `values` onto the parameters of `new`: by identifier (a rename follows), a mapped
/// parameter to its target, a removed or retyped one dropped unless mapped. Formulas are
/// rewritten for every renamed identifier.
fn carry(
    old: &Definition,
    new: &Definition,
    mapping: &BTreeMap<String, Option<String>>,
    values: &BTreeMap<String, Stored>,
) -> BTreeMap<String, Stored> {
    let mut renames: Vec<(String, String)> = Vec::new();
    let mut target: BTreeMap<String, String> = BTreeMap::new();
    for o in &old.parameters {
        let by_id = new.parameters.iter().find(|n| n.id == o.id);
        let to = match (by_id, mapping.get(&o.key)) {
            (_, Some(Some(t))) => Some(t.clone()),
            (_, Some(None)) => None,
            (Some(n), None) if n.data == o.data => Some(n.key.clone()),
            _ => None,
        };
        if let Some(t) = to {
            if t != o.key {
                renames.push((o.key.clone(), t.clone()));
            }
            target.insert(o.key.clone(), t);
        }
    }
    let mut out = BTreeMap::new();
    for (k, v) in values {
        let Some(t) = target.get(k) else { continue };
        let mut v = v.clone();
        if let Stored::Formula(f) = &v {
            let mut f = f.clone();
            for (from, to) in &renames {
                if let Ok(Some(g)) = f.renamed(from, to) {
                    f = g;
                }
            }
            v = Stored::Formula(f);
        }
        out.insert(t.clone(), v);
    }
    out
}

fn shown(
    snapshot: &Snapshot,
    def: &Definition,
    ty: &FamilyType,
    own: &BTreeMap<String, Stored>,
) -> BTreeMap<String, serde_json::Value> {
    loaded::evaluate_in(snapshot, def, ty, own)
        .map(|e| {
            e.values
                .iter()
                .map(|(k, v)| (k.clone(), json_of(v)))
                .collect()
        })
        .unwrap_or_default()
}

#[allow(clippy::too_many_lines, reason = "one report built in one pass")]
fn plan_update(
    snapshot: &Snapshot,
    p: &UpdateFamily,
    new: &Published,
) -> Result<(Loaded, Definition, UpdateReport, UpdatePlan)> {
    let have = find_family(snapshot, &p.family)?;
    let old = have.definition()?;
    let new_def = Definition::from_text(&new.text)?;
    if new.family != have.family || new.version <= have.version {
        return Err(Error::new(model_codes::FAMILY_VERSION)
            .with_arg("version", new.version.to_string())
            .with_arg("reason", "not_newer"));
    }
    let name_of = |d: &Definition, key: &str| {
        d.parameter(key)
            .map_or_else(|| key.to_owned(), |p| p.name.clone())
    };
    let mut report = UpdateReport {
        preview: true,
        family: have.name.clone(),
        from: have.version.to_string(),
        to: new.version.to_string(),
        major: step(&old, &new_def) == Step::Major,
        removed: Vec::new(),
        retyped: Vec::new(),
        added: Vec::new(),
        renamed: Vec::new(),
        needs_mapping: Vec::new(),
        types: 0,
        instances: 0,
        changes: Vec::new(),
        failing: Vec::new(),
    };
    for o in &old.parameters {
        match new_def.parameters_by_id(o.id) {
            None => report.removed.push(o.name.clone()),
            Some(n) if n.data != o.data => report.retyped.push(o.name.clone()),
            Some(n) if n.key != o.key => report.renamed.push(format!("{} -> {}", o.key, n.key)),
            Some(_) => {}
        }
    }
    for n in &new_def.parameters {
        if !old.parameters.iter().any(|o| o.id == n.id) {
            report.added.push(n.name.clone());
        }
    }
    for o in &old.parameters {
        let gone = new_def
            .parameters
            .iter()
            .find(|n| n.id == o.id)
            .is_none_or(|n| n.data != o.data);
        if gone && !p.mapping.contains_key(&o.key) {
            report.needs_mapping.push(o.key.clone());
        }
    }
    let mine: Vec<FamilyType> = family_types(snapshot)
        .into_iter()
        .filter(|t| t.family == have.record)
        .collect();
    let comps = components_of(snapshot, have.record);
    report.types = mine.len();
    report.instances = comps.len();
    let mut type_updates = Vec::new();
    let mut component_updates = Vec::new();
    let carried_type: BTreeMap<ObjectId, BTreeMap<String, Stored>> = mine
        .iter()
        .map(|t| (t.id, carry(&old, &new_def, &p.mapping, &t.values)))
        .collect();
    for t in &mine {
        let after = FamilyType {
            values: carried_type[&t.id].clone(),
            ..t.clone()
        };
        let (b, a) = (
            shown(snapshot, &old, t, &BTreeMap::new()),
            shown(snapshot, &new_def, &after, &BTreeMap::new()),
        );
        for (key, before) in &b {
            let a_value = new_def
                .parameters
                .iter()
                .find(|n| old.parameter(key).is_some_and(|o| o.id == n.id))
                .and_then(|n| a.get(&n.key));
            if a_value != Some(before) && report.changes.len() < MAX_LISTED {
                report.changes.push(ValueChange {
                    owner: t.name.clone(),
                    parameter: name_of(&old, key),
                    before: Some(before.clone()),
                    after: a_value.cloned(),
                });
            }
        }
        type_updates.push((t.id, carried_type[&t.id].clone()));
    }
    for c in &comps {
        let Some(ty) = mine.iter().find(|t| t.id == c.family_type) else {
            continue;
        };
        let own = carry(&old, &new_def, &p.mapping, &c.values);
        let after_ty = FamilyType {
            values: carried_type[&ty.id].clone(),
            ..ty.clone()
        };
        if let Err(e) = loaded::evaluate_in(snapshot, &new_def, &after_ty, &own) {
            report.failing.push(Failing {
                instance: if c.name.is_empty() {
                    c.id.to_string()
                } else {
                    c.name.clone()
                },
                reason: e.code().to_owned(),
            });
        }
        component_updates.push((c.id, own));
    }
    Ok((
        have,
        new_def,
        report,
        UpdatePlan {
            types: type_updates,
            components: component_updates,
        },
    ))
}

struct UpdatePlan {
    types: Vec<(ObjectId, BTreeMap<String, Stored>)>,
    components: Vec<(ObjectId, BTreeMap<String, Stored>)>,
}

trait ById {
    fn parameters_by_id(&self, id: ObjectId) -> Option<&archbuilder_model::family::def::ParamDef>;
}

impl ById for Definition {
    fn parameters_by_id(&self, id: ObjectId) -> Option<&archbuilder_model::family::def::ParamDef> {
        self.parameters.iter().find(|p| p.id == id)
    }
}

fn newer(ctx: &Context<'_>, p: &UpdateFamily) -> Result<Published> {
    let have = find_family(ctx.snapshot(), &p.family)?;
    read_source(ctx, &p.path)?
        .into_iter()
        .filter(|s| s.family == have.family)
        .max_by_key(|s| s.version)
        .ok_or_else(|| {
            Error::new(model_codes::FAMILY_VERSION)
                .with_arg("version", "")
                .with_arg("reason", "not_found")
        })
}

fn update_preview(ctx: &mut Context<'_>, p: UpdateFamily) -> Result<UpdateReport> {
    let new = newer(ctx, &p)?;
    plan_update(ctx.snapshot(), &p, &new).map(|(_, _, r, _)| r)
}

fn update(ctx: &mut Context<'_>, p: UpdateFamily) -> Result<UpdateReport> {
    let new = newer(ctx, &p)?;
    let (have, new_def, mut report, plan) = plan_update(ctx.snapshot(), &p, &new)?;
    if !report.needs_mapping.is_empty() {
        return Err(Error::new(model_codes::FAMILY_MAPPING)
            .with_arg("parameters", report.needs_mapping.join(", ")));
    }
    if let Some(f) = report.failing.first() {
        return Err(Error::new(model_codes::FAMILY_EVALUATION)
            .with_arg("instance", f.instance.as_str())
            .with_arg("reason", f.reason.as_str())
            .with_arg(
                "count",
                i64::try_from(report.failing.len()).unwrap_or(i64::MAX),
            ));
    }
    let project = spatial::project(ctx.snapshot()).map(|o| o.id);
    let fields = Loaded::values(&new, &new_def);
    let names = [
        "name",
        "family",
        "version",
        "category",
        "hosting",
        "checksum",
        "ifc_entity",
        "ifc_type",
        "source",
        "license",
        "text",
    ];
    for (name, value) in names.into_iter().zip(fields) {
        ctx.txn()?
            .modify(have.record, FieldPath::new(name), value)?;
    }
    for (id, values) in &plan.types {
        ctx.txn()?
            .modify(*id, FieldPath::new("values"), values_value(values))?;
    }
    for (id, values) in &plan.components {
        ctx.txn()?
            .modify(*id, FieldPath::new("values"), values_value(values))?;
    }
    let mine: BTreeSet<String> = family_types(ctx.snapshot())
        .into_iter()
        .filter(|t| t.family == have.record)
        .map(|t| t.name)
        .collect();
    for t in new_def
        .types
        .iter()
        .filter(|t| p.add_types.contains(&t.name))
    {
        if mine.contains(&t.name) {
            continue;
        }
        let owner = project.ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        create(
            ctx,
            FAMILY_TYPE,
            Some(owner),
            FamilyType::record(&t.name, have.record, &t.values),
        )?;
    }
    ensure_valid(ctx)?;
    loaded::check_components(ctx.txn()?.snapshot(), None)?;
    report.preview = false;
    Ok(report)
}

/// Parameters of `archbuilder.component.place`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceComponent {
    /// The storey; else the storey of the selection, else the lowest storey.
    #[serde(default)]
    pub storey: Option<IdParam>,
    /// The family type, by identifier, or by name (`Family: Type` or the type's name when only
    /// one family type has it).
    #[serde(rename = "type")]
    #[schemars(length(min = 1, max = 600))]
    pub family_type: String,
    /// The insertion point `[x, y]` in meters.
    pub point: [f64; 2],
    /// The rotation about the vertical axis (a bare number is radians).
    #[serde(default)]
    pub rotation: Option<QuantityParam>,
    /// Whether to mirror across the component's own left-right axis.
    #[serde(default)]
    pub mirror: bool,
    /// The distance above the storey's level (a bare number is meters).
    #[serde(default)]
    pub offset: Option<QuantityParam>,
    /// A mark.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub name: Option<String>,
    /// Instance values by parameter key: a literal, or `{"formula": "...", "unit": "mm"}`.
    #[serde(default)]
    pub values: BTreeMap<String, serde_json::Value>,
}

/// What `archbuilder.component.place` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ComponentPlaced {
    /// The new component.
    pub component: IdParam,
}

/// The family type `text` names: an identifier of a family type record, `Family: Type`, or a
/// type name that only one family type has.
fn resolve_type(snapshot: &Snapshot, text: &str) -> Result<FamilyType> {
    let all = family_types(snapshot);
    if let Ok(id) = text.parse::<ObjectId>()
        && let Some(t) = all.iter().find(|t| t.id == id)
    {
        return Ok(t.clone());
    }
    let families = loaded::families(snapshot);
    let fam_name = |t: &FamilyType| {
        families
            .iter()
            .find(|f| f.record == t.family)
            .map(|f| f.name.clone())
            .unwrap_or_default()
    };
    let fold = |s: &str| archbuilder_model::names::fold(s);
    let mut found: Vec<&FamilyType> = all
        .iter()
        .filter(|t| fold(&format!("{}: {}", fam_name(t), t.name)) == fold(text))
        .collect();
    if found.is_empty() {
        found = all.iter().filter(|t| fold(&t.name) == fold(text)).collect();
    }
    match found.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(Error::new(codes::KIND_MISMATCH).with_arg("expected", FAMILY_TYPE)),
        _ => Err(Error::new(model_codes::NAME_TAKEN).with_arg("name", text)),
    }
}

/// Whether the family type `text` (an identifier or a name) is hosted by walls: its instances go
/// into a wall with `archbuilder.family_insert.place`, not into the storey with
/// `archbuilder.component.place`.
#[must_use]
pub fn hosted_by_walls(snapshot: &Snapshot, text: &str) -> bool {
    resolve_type(snapshot, text)
        .ok()
        .and_then(|t| spatial::get(snapshot, t.family))
        .and_then(|o| Loaded::of(&o))
        .and_then(|f| f.definition().ok())
        .is_some_and(|d| loaded::host_keys(&d).contains(&types::WALL))
}

fn instance_values(
    ctx: &Context<'_>,
    def: &Definition,
    given: &BTreeMap<String, serde_json::Value>,
) -> Result<BTreeMap<String, Stored>> {
    let mut out = BTreeMap::new();
    for (key, json) in given {
        let p = def.parameter(key).ok_or_else(invalid)?;
        let (value, formula, unit) = match json.as_object() {
            Some(o) => (
                None,
                o.get("formula").and_then(serde_json::Value::as_str),
                o.get("unit").and_then(serde_json::Value::as_str),
            ),
            None => (Some(json), None, None),
        };
        let stored: Option<Stored> = stored_input(ctx, (&p.name, p.data), value, formula, unit)?;
        out.insert(key.clone(), stored.ok_or_else(invalid)?);
    }
    Ok(out)
}

fn place(ctx: &mut Context<'_>, p: PlaceComponent) -> Result<ComponentPlaced> {
    let storey = crate::wall::storey_for(ctx, p.storey)?;
    let ty = resolve_type(ctx.snapshot(), &p.family_type)?;
    let fam = spatial::get(ctx.snapshot(), ty.family)
        .and_then(|o| Loaded::of(&o))
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let def = fam.definition()?;
    // A hosted family goes into its host (`archbuilder.family_insert.place`); a profile family
    // has no body and is used by edges and bands.
    if def.header.hosting != Hosting::None || def.header.category == Category::Profile {
        return Err(Error::new(model_codes::FAMILY_INVALID)
            .with_arg("part", "definition")
            .with_arg("reason", "not_freestanding"));
    }
    let rotation = match &p.rotation {
        Some(q) => ctx.quantity(q, Dimension::ANGLE)?.value(),
        None => 0.0,
    };
    let offset = match &p.offset {
        Some(q) => crate::length(ctx, q)?.value(),
        None => 0.0,
    };
    if !p.point.iter().all(|v| v.is_finite()) {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let values = instance_values(ctx, &def, &p.values)?;
    let c = Component {
        id: storey.id,
        storey: Some(storey.id),
        name: p.name.unwrap_or_default().trim().to_owned(),
        family_type: ty.id,
        offset,
        origin: p.point,
        rotation,
        mirror: p.mirror,
        values,
    };
    let id = create(ctx, COMPONENT, Some(storey.id), c.record())?;
    ensure_valid(ctx)?;
    loaded::check_components(ctx.txn()?.snapshot(), Some(&[id]))?;
    Ok(ComponentPlaced {
        component: IdParam(id),
    })
}

/// Parameters of `archbuilder.family_insert.place`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceInsert {
    /// The host: a wall for a family hosted by walls.
    pub host: IdParam,
    /// The family type, by identifier, or by name (`Family: Type`).
    #[serde(rename = "type")]
    #[schemars(length(min = 1, max = 600))]
    pub family_type: String,
    /// The distance along the host's baseline to the insertion point (a bare number is meters).
    pub distance: QuantityParam,
    /// The height of the insertion point above the host's base (a bare number is meters).
    #[serde(default)]
    pub offset: Option<QuantityParam>,
    /// Whether the family has the right hand (mirrored across its own left-right axis).
    #[serde(default)]
    pub flip_hand: bool,
    /// Whether the family faces the interior side (turned half a turn).
    #[serde(default)]
    pub flip_facing: bool,
    /// A mark.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub name: Option<String>,
    /// Instance values by parameter key: a literal, or `{"formula": "...", "unit": "mm"}`.
    #[serde(default)]
    pub values: BTreeMap<String, serde_json::Value>,
}

/// What `archbuilder.family_insert.place` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct InsertPlaced {
    /// The new hosted instance.
    pub insert: IdParam,
}

fn place_insert(ctx: &mut Context<'_>, p: PlaceInsert) -> Result<InsertPlaced> {
    use archbuilder_model::family::loaded::FamilyInsert;
    use archbuilder_model::insert::{Facing, Hand};
    let host =
        spatial::get(ctx.snapshot(), p.host.0).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let host_key = spatial::type_of(ctx.snapshot(), host.id).unwrap_or_default();
    let ty = resolve_type(ctx.snapshot(), &p.family_type)?;
    let fam = spatial::get(ctx.snapshot(), ty.family)
        .and_then(|o| Loaded::of(&o))
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let def = fam.definition()?;
    loaded::check_host(&def, &host_key)?;
    let distance = crate::length(ctx, &p.distance)?.value();
    let offset = match &p.offset {
        Some(q) => crate::length(ctx, q)?.value(),
        None => 0.0,
    };
    let values = instance_values(ctx, &def, &p.values)?;
    let insert = FamilyInsert {
        id: host.id,
        host: Some(host.id),
        name: p.name.unwrap_or_default().trim().to_owned(),
        family_type: ty.id,
        distance,
        offset,
        hand: if p.flip_hand { Hand::Right } else { Hand::Left },
        facing: if p.flip_facing {
            Facing::Interior
        } else {
            Facing::Exterior
        },
        values,
    };
    let id = create(ctx, types::FAMILY_INSERT, Some(host.id), insert.record())?;
    ensure_valid(ctx)?;
    Ok(InsertPlaced {
        insert: IdParam(id),
    })
}

/// Sets the name and the values of the family type `object` (the family-type branch of
/// `archbuilder.type.set`): each value is a literal or `{"formula", "unit"}`; every component of
/// the type is evaluated again before the change commits.
///
/// # Errors
/// The parse and evaluation errors, naming the parameter, form or component.
pub(crate) fn set_type_values(
    ctx: &mut Context<'_>,
    id: ObjectId,
    name: Option<&str>,
    given: &BTreeMap<String, serde_json::Value>,
) -> Result<()> {
    let object = expect(ctx.snapshot(), id, FAMILY_TYPE)?;
    let ty = FamilyType::of(&object).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let fam = spatial::get(ctx.snapshot(), ty.family)
        .and_then(|o| Loaded::of(&o))
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let def = fam.definition()?;
    if let Some(n) = name {
        ctx.txn()?.modify(
            id,
            FieldPath::new("name"),
            Some(Value::String(n.trim().to_owned())),
        )?;
    }
    if !given.is_empty() {
        let mut values = ty.values.clone();
        for (k, v) in instance_values(ctx, &def, given)? {
            values.insert(k, v);
        }
        ctx.txn()?
            .modify(id, FieldPath::new("values"), values_value(&values))?;
    }
    ensure_valid(ctx)?;
    loaded::check_components(ctx.txn()?.snapshot(), None)
}

/// A text prompt of one to `max` characters.
fn text(max: usize) -> Accept {
    Accept::Text {
        min_chars: 1,
        max_chars: Some(max),
    }
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
#[allow(clippy::too_many_lines, reason = "one registration after another")]
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
            FAMILY_LOAD,
            MessageId {
                message_id: "ab-cmd-family-load-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-family-load-description",
            }
            .message_id,
            load,
        )
        .with_permissions([Permission::Read, Permission::Edit, Permission::FileRead])
        .with_prompts(
            PromptSequence::new().step(PromptStep::new(
                "path",
                MessageId {
                    message_id: "ab-prompt-family-package",
                }
                .message_id,
                text(4096),
            )),
        ),
    )?;
    registry.register(
        CommandSpec::new(
            FAMILY_LOAD_PREVIEW,
            MessageId {
                message_id: "ab-cmd-family-load-preview-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-family-load-preview-description",
            }
            .message_id,
            load_preview,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read, Permission::FileRead]),
    )?;
    registry.register(
        CommandSpec::new(
            FAMILY_UPDATE,
            MessageId {
                message_id: "ab-cmd-family-update-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-family-update-description",
            }
            .message_id,
            update,
        )
        .with_permissions([Permission::Read, Permission::Edit, Permission::FileRead]),
    )?;
    registry.register(
        CommandSpec::new(
            FAMILY_UPDATE_PREVIEW,
            MessageId {
                message_id: "ab-cmd-family-update-preview-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-family-update-preview-description",
            }
            .message_id,
            update_preview,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read, Permission::FileRead]),
    )?;
    registry.register(
        CommandSpec::new(
            COMPONENT_PLACE,
            MessageId {
                message_id: "ab-cmd-component-place-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-component-place-description",
            }
            .message_id,
            place,
        )
        .with_prompts(
            PromptSequence::new()
                .step(PromptStep::new(
                    "type",
                    MessageId {
                        message_id: "ab-prompt-component-type",
                    }
                    .message_id,
                    text(600),
                ))
                .step(PromptStep::new(
                    "point",
                    MessageId {
                        message_id: "ab-prompt-component-point",
                    }
                    .message_id,
                    Accept::Point {
                        dims: PointDims::Two,
                    },
                )),
        ),
    )?;
    registry.register(
        CommandSpec::new(
            FAMILY_INSERT_PLACE,
            MessageId {
                message_id: "ab-cmd-family-insert-place-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-family-insert-place-description",
            }
            .message_id,
            place_insert,
        )
        .with_prompt(Arc::new(FamilyInsertPrompts)),
    )?;
    Ok(())
}

/// The prompts of `archbuilder.family_insert.place`, as the door's: the family type (preset, else
/// asked), the host wall (preset, picked, or the wall the pointer is on) and the distance along it.
/// The flip keywords toggle the hand and the facing, and they carry to the next prompt.
#[derive(Debug)]
struct FamilyInsertPrompts;

/// The answers of the prompts so far: the family type, the host wall, the flips.
struct Asked {
    family: Option<String>,
    wall: Option<ObjectId>,
    flip_hand: bool,
    flip_facing: bool,
}

impl FamilyInsertPrompts {
    /// What the answers and the preset give so far.
    fn asked(state: &PromptState<'_>) -> Asked {
        let flag = |key: &str| state.preset.get(key).and_then(serde_json::Value::as_bool);
        let mut asked = Asked {
            family: state
                .preset
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            wall: state
                .preset
                .get("wall")
                .and_then(serde_json::Value::as_str)
                .and_then(|s| s.parse().ok()),
            flip_hand: flag("flip_hand").unwrap_or(false),
            flip_facing: flag("flip_facing").unwrap_or(false),
        };
        for answer in state.answers {
            match &answer.input {
                PromptInput::Text(text) => asked.family = Some(text.clone()),
                PromptInput::Pick(id) => asked.wall = Some(*id),
                PromptInput::Keyword(k) if k == "flip_hand" => asked.flip_hand = !asked.flip_hand,
                PromptInput::Keyword(k) if k == "flip_facing" => {
                    asked.flip_facing = !asked.flip_facing;
                }
                _ => {}
            }
        }
        asked
    }
}

impl PromptMachine for FamilyInsertPrompts {
    fn next(&self, state: &PromptState<'_>) -> Result<Next> {
        let asked = Self::asked(state);
        let Some(family) = asked.family.clone() else {
            return Ok(Next::Ask(Prompt::new(
                "type",
                MessageId {
                    message_id: "ab-prompt-family-insert-type",
                }
                .message_id,
                text(600),
            )));
        };
        // The distance is the answer that completes the command.
        let distance = state.answers.iter().rev().find_map(|a| match a.input {
            PromptInput::Distance(d) => Some(d),
            _ => None,
        });
        if let (Some(distance), Some(wall)) = (distance, asked.wall) {
            let mut params = serde_json::Map::new();
            params.insert("type".to_owned(), family.into());
            params.insert("host".to_owned(), wall.to_string().into());
            params.insert("distance".to_owned(), distance.into());
            params.insert("flip_hand".to_owned(), asked.flip_hand.into());
            params.insert("flip_facing".to_owned(), asked.flip_facing.into());
            return Ok(Next::Done(params));
        }
        let turkish = state
            .setting(LANGUAGE_SETTING)
            .and_then(|v| v.as_str().map(str::to_owned))
            .is_some_and(|l| l.starts_with("tr"));
        let mut arg =
            Prompt::new(
                format!("point-{}", state.answers.len()),
                "ab-prompt-family-insert-point",
                Accept::Distance {
                    min: Some(0.0),
                    max: Some(types::DISTANCE_RANGE.1),
                },
            )
            .or(Accept::Pick {
                types: vec![types::WALL.to_owned()],
            })
            .or(Accept::keywords(vec![
                KeywordOption::new("flip_hand", "ab-keyword-insert-hand")
                    .with_shortcut(if turkish { 'k' } else { 'f' }),
                KeywordOption::new("flip_facing", "ab-keyword-insert-facing").with_shortcut('a'),
            ]));
        arg = arg
            .with_arg("hand", if asked.flip_hand { "right" } else { "left" })
            .with_arg(
                "facing",
                if asked.flip_facing {
                    "interior"
                } else {
                    "exterior"
                },
            );
        Ok(Next::Ask(arg))
    }

    fn check(&self, state: &PromptState<'_>, prompt: &Prompt, input: &PromptInput) -> Result<()> {
        if matches!(input, PromptInput::Distance(_))
            && prompt.key.starts_with("point")
            && Self::asked(state).wall.is_none()
        {
            return Err(Error::new(codes::NO_WALL_SELECTED));
        }
        Ok(())
    }
}

/// The record values of `moved` laid over those of `object`, so that fields the component struct
/// does not carry (project parameter values) stay.
fn moved_values(object: &arc_doc::Object, moved: &Component) -> Vec<Option<arc_doc::Value>> {
    let mut values = object.values.clone();
    for (slot, v) in moved
        .record()
        .into_iter()
        .enumerate()
        .take(types::component::VALUES + 1)
        .skip(1)
    {
        if let Some(at) = values.get_mut(slot) {
            *at = v;
        }
    }
    values
}

/// Moves the components `ids` under the plane motion `xf` (the modify commands).
pub(crate) fn move_components(
    ctx: &mut Context<'_>,
    snapshot: &arc_doc::Snapshot,
    ids: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
) -> Result<()> {
    for id in ids {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let moved = Component::of(&object)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
            .moved(xf);
        let mut record = moved.record();
        // The parameters field is not the placement's: it stays as it is.
        record.truncate(types::component::VALUES + 1);
        crate::circulation::write_changes(ctx, COMPONENT, &object, record)?;
    }
    Ok(())
}

/// Copies the components `ids` moved by `xf`, onto `target` or their own storey; the family type
/// stays, the offset above the level and the values are kept.
pub(crate) fn copy_components(
    ctx: &mut Context<'_>,
    snapshot: &arc_doc::Snapshot,
    ids: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
    target: Option<ObjectId>,
) -> Result<Vec<ObjectId>> {
    let mut out = Vec::new();
    for id in ids {
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let storey = target
            .or(object.header.owner)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let moved = Component::of(&object)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?
            .moved(xf);
        out.push(create(
            ctx,
            COMPONENT,
            Some(storey),
            moved_values(&object, &moved),
        )?);
    }
    Ok(out)
}
