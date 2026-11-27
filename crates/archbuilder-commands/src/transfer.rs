//! Starting a project from a template and transferring standards between documents
//! (AB-DATUM-001, AB-DATUM-005). The functions take the snapshot of the other document, which the
//! file layer (`archbuilder-io`) reads from a file; they change the open document only through
//! its transaction, so each is one undo step.
//!
//! Objects copied from another document get new identifiers (an identifier never repeats across
//! projects, and the IFC `GlobalId`s derive from it); references inside the copied set are
//! remapped, and a reference to something that was not copied is cleared.

use std::collections::{BTreeMap, BTreeSet};

use arc_base::{Error, ObjectId, Result};
use arc_command::{Context, IdParam};
use arc_doc::{FieldPath, Header, Object, Snapshot, Value};
use archbuilder_model::names;
use archbuilder_model::spatial;
use archbuilder_model::types::{
    self, BUILDING, CEILING_TYPE, DOOR_TYPE, FLOOR_TYPE, LEVEL, PLAN_VIEW, PROJECT, RAILING_TYPE,
    RAMP_TYPE, ROOF_TYPE, SITE, STOREY, WALL_TYPE, WINDOW_TYPE, project,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::error::codes;

/// The kinds of element type a project keeps its standards in.
pub const TYPE_KINDS: [&str; 8] = [
    WALL_TYPE,
    FLOOR_TYPE,
    DOOR_TYPE,
    WINDOW_TYPE,
    ROOF_TYPE,
    CEILING_TYPE,
    RAILING_TYPE,
    RAMP_TYPE,
];

/// The fields of the project record that are settings: the display units, decimals and rounding.
const SETTINGS_FIELDS: [usize; 15] = [
    project::LENGTH_UNIT,
    project::SLOPE_UNIT,
    project::AREA_UNIT,
    project::VOLUME_UNIT,
    project::ANGLE_UNIT,
    project::LENGTH_PRECISION,
    project::AREA_PRECISION,
    project::VOLUME_PRECISION,
    project::ANGLE_PRECISION,
    project::SLOPE_PRECISION,
    project::LENGTH_ROUNDING,
    project::AREA_ROUNDING,
    project::VOLUME_ROUNDING,
    project::ANGLE_ROUNDING,
    project::SLOPE_ROUNDING,
];

/// How many objects of one kind were not copied from a template.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Skipped {
    /// The type key of the kind.
    pub kind: String,
    /// The count.
    pub count: usize,
}

/// What a project made from a template holds.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TemplateApplied {
    /// The project of the new document.
    pub project: IdParam,
    /// The objects copied.
    pub copied: usize,
    /// What the template holds that a template does not carry (elements, datums, selection sets),
    /// not copied.
    pub skipped: Vec<Skipped>,
}

/// `value` with every reference replaced by its new identifier; `None` when it referred to
/// something that was not copied. Lists and maps lose such entries; a record keeps its shape.
pub(crate) fn remap(value: &Value, map: &BTreeMap<ObjectId, ObjectId>) -> Option<Value> {
    match value {
        Value::Reference(id) => map.get(id).copied().map(Value::Reference),
        Value::List(items) => Some(Value::List(
            items.iter().filter_map(|v| remap(v, map)).collect(),
        )),
        Value::Map(entries) => Some(Value::Map(
            entries
                .iter()
                .filter_map(|(k, v)| remap(v, map).map(|v| (k.clone(), v)))
                .collect(),
        )),
        Value::Record(fields) => Some(Value::Record(
            fields
                .iter()
                .map(|f| f.as_ref().and_then(|v| remap(v, map)))
                .collect(),
        )),
        other => Some(other.clone()),
    }
}

/// Whether `value` holds a reference to another object.
pub(crate) fn has_reference(value: &Value) -> bool {
    match value {
        Value::Reference(_) | Value::ExternalReference(_) => true,
        Value::List(items) => items.iter().any(has_reference),
        Value::Map(entries) => entries.values().any(has_reference),
        Value::Record(fields) => fields.iter().flatten().any(has_reference),
        _ => false,
    }
}

fn name_of(object: &Object) -> String {
    types::text(&object.values, 0)
        .unwrap_or_default()
        .to_owned()
}

/// Starts the project of the open document from the template `source`: the project record
/// (units and information, marked as no template and naming `file_name` as its origin), the site,
/// building, storeys and levels, the element types and the plan views are copied under new
/// identifiers, replacing the structure the document has. The template is left unchanged.
///
/// # Errors
/// `ab.cmd.template_invalid` when `source` has no project with a site, a building and a storey;
/// `ab.cmd.template_target_has_elements` when the open document has elements.
pub fn from_template(
    ctx: &mut Context<'_>,
    source: &Snapshot,
    file_name: &str,
) -> Result<TemplateApplied> {
    let valid = spatial::project(source).is_some()
        && spatial::site(source).is_some()
        && spatial::buildings(source)
            .first()
            .is_some_and(|b| !spatial::storeys(source, b.id).is_empty());
    if !valid {
        return Err(Error::new(codes::TEMPLATE_INVALID));
    }
    let elements: usize = types::ELEMENTS
        .iter()
        .map(|k| spatial::objects(ctx.snapshot(), k).len())
        .sum();
    if elements > 0 {
        return Err(Error::new(codes::TEMPLATE_TARGET_HAS_ELEMENTS)
            .with_arg("count", i64::try_from(elements).unwrap_or(i64::MAX)));
    }
    let structure = [PROJECT, SITE, BUILDING, STOREY, LEVEL];
    let copied_keys: Vec<&str> = structure
        .iter()
        .copied()
        .chain(TYPE_KINDS)
        .chain([PLAN_VIEW])
        .collect();
    // What the document has goes first, the structure last (a storey refers to its level).
    let mut gone: Vec<ObjectId> = Vec::new();
    for key in types::KEYS.iter().filter(|k| !structure.contains(k)) {
        gone.extend(spatial::objects(ctx.snapshot(), key).iter().map(|o| o.id));
    }
    for key in [STOREY, LEVEL, BUILDING, SITE, PROJECT] {
        gone.extend(spatial::objects(ctx.snapshot(), key).iter().map(|o| o.id));
    }
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    for id in gone {
        txn.delete(id)?;
    }
    txn.apply_delete_rules(&limits)?;

    let mut sources: Vec<(&str, std::sync::Arc<Object>)> = Vec::new();
    for key in &copied_keys {
        sources.extend(spatial::objects(source, key).into_iter().map(|o| (*key, o)));
    }
    let mut map: BTreeMap<ObjectId, ObjectId> = BTreeMap::new();
    for (_, object) in &sources {
        map.insert(object.id, ctx.new_id());
    }
    let mut new_project = None;
    for (key, object) in &sources {
        let Some(&id) = map.get(&object.id) else {
            continue;
        };
        let mut values: Vec<Option<Value>> = object
            .values
            .iter()
            .map(|v| v.as_ref().and_then(|v| remap(v, &map)))
            .collect();
        if *key == PROJECT {
            let fields = ctx
                .snapshot()
                .registry()
                .get(key)
                .map_or(values.len(), |d| d.schema.fields.len());
            values.resize(fields, None);
            values[project::TEMPLATE] = Some(Value::Bool(false));
            values[project::TEMPLATE_SOURCE] =
                (!file_name.is_empty()).then(|| Value::String(file_name.to_owned()));
            new_project = Some(id);
        }
        if *key == PLAN_VIEW {
            // The views start without the hidden elements and grid overrides of the template.
            for at in [types::plan_view::HIDDEN, types::plan_view::DATUM_VIEWS] {
                if let Some(slot) = values.get_mut(at) {
                    *slot = None;
                }
            }
        }
        let header = Header {
            owner: object.header.owner.and_then(|o| map.get(&o).copied()),
            ..object.header.clone()
        };
        ctx.txn()?.create(key, id, header, values)?;
    }
    let new_project = new_project.ok_or_else(|| Error::new(codes::TEMPLATE_INVALID))?;
    crate::structure::ensure_valid(ctx)?;
    ctx.set_selection_after(vec![new_project]);
    let skipped = types::KEYS
        .iter()
        .filter(|k| !copied_keys.contains(k))
        .filter_map(|k| {
            let count = spatial::objects(source, k).len();
            (count > 0).then(|| Skipped {
                kind: (*k).to_owned(),
                count,
            })
        })
        .collect();
    Ok(TemplateApplied {
        project: IdParam(new_project),
        copied: sources.len(),
        skipped,
    })
}

/// What to do with a type whose name the document already has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Conflict {
    /// Keep the type the document has.
    #[default]
    Keep,
    /// Replace the values of the type the document has, when nothing uses it.
    Overwrite,
    /// Copy it under a free name.
    Rename,
}

impl Conflict {
    /// The conflict named `text`: `keep`, `overwrite` or `rename`.
    ///
    /// # Errors
    /// `ab.cmd.value_invalid` for any other text.
    pub fn parse(text: &str) -> Result<Self> {
        match text.trim() {
            "keep" => Ok(Self::Keep),
            "overwrite" => Ok(Self::Overwrite),
            "rename" => Ok(Self::Rename),
            _ => Err(Error::new(codes::VALUE_INVALID)),
        }
    }
}

/// What to transfer and how to settle name conflicts.
#[derive(Debug, Default)]
pub struct TransferOptions {
    /// The kinds of type to copy, named without the product prefix (`wall_type`); all when empty.
    pub kinds: Vec<String>,
    /// The type names to copy; all when empty.
    pub names: Vec<String>,
    /// The default for a name the document has.
    pub conflict: Conflict,
    /// The conflict per type name, over the default.
    pub resolutions: BTreeMap<String, Conflict>,
    /// Whether the display unit settings are copied too.
    pub settings: bool,
}

/// What happened to one type.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TransferItem {
    /// The type key of the kind.
    pub kind: String,
    /// The name in the other document.
    pub name: String,
    /// `copied`, `overwritten`, `renamed`, `kept` or `skipped`.
    pub action: String,
    /// Why a type was kept or skipped: `exists`, `in_use` or `references`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The type in this document, when one was made or changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element_type: Option<IdParam>,
}

/// What `archbuilder.file.transfer_standards` reports.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Transferred {
    /// Each type of the other document that was considered.
    pub items: Vec<TransferItem>,
    /// How many were copied as new types (also the renamed).
    pub copied: usize,
    /// How many existing types were overwritten.
    pub overwritten: usize,
    /// How many were kept or skipped.
    pub skipped: usize,
    /// Whether the unit settings were copied.
    pub settings_copied: bool,
}

/// The objects that refer to each of `targets`, other than the target itself.
pub(crate) fn referrers(
    snapshot: &Snapshot,
    targets: &BTreeSet<ObjectId>,
) -> BTreeMap<ObjectId, BTreeSet<ObjectId>> {
    fn scan(
        value: &Value,
        from: ObjectId,
        targets: &BTreeSet<ObjectId>,
        out: &mut BTreeMap<ObjectId, BTreeSet<ObjectId>>,
    ) {
        match value {
            Value::Reference(id) if *id != from && targets.contains(id) => {
                out.entry(*id).or_default().insert(from);
            }
            Value::List(items) => items.iter().for_each(|v| scan(v, from, targets, out)),
            Value::Map(entries) => entries.values().for_each(|v| scan(v, from, targets, out)),
            Value::Record(fields) => fields
                .iter()
                .flatten()
                .for_each(|v| scan(v, from, targets, out)),
            _ => {}
        }
    }
    let mut out = BTreeMap::new();
    for decl in snapshot.registry().iter() {
        for object in spatial::objects(snapshot, decl.key.as_str()) {
            for value in object.values.iter().flatten() {
                scan(value, object.id, targets, &mut out);
            }
        }
    }
    out
}

fn free_name(taken: &[String], name: &str) -> String {
    (2..)
        .map(|n| format!("{name} ({n})"))
        .find(|candidate| !taken.iter().any(|t| names::same(t, candidate)))
        .unwrap_or_else(|| name.to_owned())
}

/// Copies the element types of `source` into the open document, and with them the display unit
/// settings, settling a name the document already has by `options`.
///
/// # Errors
/// The errors of the transaction and of the model's rules.
pub fn transfer(
    ctx: &mut Context<'_>,
    source: &Snapshot,
    options: &TransferOptions,
) -> Result<Transferred> {
    let mut items: Vec<TransferItem> = Vec::new();
    let (mut copied, mut overwritten, mut skipped) = (0, 0, 0);
    for key in TYPE_KINDS {
        let short = key.strip_prefix("archbuilder.").unwrap_or(key);
        if !options.kinds.is_empty() && !options.kinds.iter().any(|k| k == short) {
            continue;
        }
        let mut taken: Vec<String> = spatial::objects(ctx.snapshot(), key)
            .iter()
            .map(|o| name_of(o))
            .collect();
        for object in spatial::objects(source, key) {
            let name = name_of(&object);
            if !options.names.is_empty() && !options.names.iter().any(|n| names::same(n, &name)) {
                continue;
            }
            let item = |action: &str, reason: Option<&str>, id: Option<ObjectId>| TransferItem {
                kind: key.to_owned(),
                name: name.clone(),
                action: action.to_owned(),
                reason: reason.map(str::to_owned),
                element_type: id.map(IdParam),
            };
            if object.values.iter().flatten().any(has_reference) {
                skipped += 1;
                items.push(item("skipped", Some("references"), None));
                continue;
            }
            let existing = spatial::objects(ctx.snapshot(), key)
                .into_iter()
                .find(|o| names::same(&name_of(o), &name));
            let conflict = options
                .resolutions
                .iter()
                .find(|(n, _)| names::same(n, &name))
                .map_or(options.conflict, |(_, c)| *c);
            match (existing, conflict) {
                (None, _) => {
                    let id = create_type(ctx, key, &object.values, &name)?;
                    taken.push(name.clone());
                    copied += 1;
                    items.push(item("copied", None, Some(id)));
                }
                (Some(_), Conflict::Keep) => {
                    skipped += 1;
                    items.push(item("kept", Some("exists"), None));
                }
                (Some(_), Conflict::Rename) => {
                    let renamed = free_name(&taken, &name);
                    let id = create_type(ctx, key, &object.values, &renamed)?;
                    taken.push(renamed);
                    copied += 1;
                    items.push(item("renamed", None, Some(id)));
                }
                (Some(found), Conflict::Overwrite) => {
                    let targets = BTreeSet::from([found.id]);
                    if referrers(ctx.snapshot(), &targets).contains_key(&found.id) {
                        skipped += 1;
                        items.push(item("skipped", Some("in_use"), None));
                        continue;
                    }
                    let fields: Vec<String> = ctx
                        .snapshot()
                        .registry()
                        .get(key)
                        .map(|d| d.schema.fields.iter().map(|f| f.name.clone()).collect())
                        .unwrap_or_default();
                    let txn = ctx.txn()?;
                    for (at, field) in fields.iter().enumerate().skip(1) {
                        let value = object.values.get(at).cloned().flatten();
                        txn.modify(found.id, FieldPath::new(field.as_str()), value)?;
                    }
                    overwritten += 1;
                    items.push(item("overwritten", None, Some(found.id)));
                }
            }
        }
    }
    let settings_copied = options.settings && copy_settings(ctx, source)?;
    if copied + overwritten == 0 && !settings_copied {
        return Ok(Transferred {
            items,
            copied,
            overwritten,
            skipped,
            settings_copied,
        });
    }
    crate::structure::ensure_valid(ctx)?;
    Ok(Transferred {
        items,
        copied,
        overwritten,
        skipped,
        settings_copied,
    })
}

/// Copies the display unit settings of the project of `source` to the open document's project;
/// whether there were both projects to copy between.
fn copy_settings(ctx: &mut Context<'_>, source: &Snapshot) -> Result<bool> {
    let (Some(from), Some(to)) = (spatial::project(source), spatial::project(ctx.snapshot()))
    else {
        return Ok(false);
    };
    let fields: Vec<String> = ctx
        .snapshot()
        .registry()
        .get(PROJECT)
        .map(|d| d.schema.fields.iter().map(|f| f.name.clone()).collect())
        .unwrap_or_default();
    let txn = ctx.txn()?;
    for at in SETTINGS_FIELDS {
        if let Some(field) = fields.get(at) {
            let value = from.values.get(at).cloned().flatten();
            txn.modify(to.id, FieldPath::new(field.as_str()), value)?;
        }
    }
    Ok(true)
}

/// A new type of kind `key` with the values of another document's type under `name`.
fn create_type(
    ctx: &mut Context<'_>,
    key: &str,
    values: &[Option<Value>],
    name: &str,
) -> Result<ObjectId> {
    let mut values = values.to_vec();
    if let Some(slot) = values.first_mut() {
        *slot = Some(Value::String(name.to_owned()));
    }
    // Types belong to the project.
    let owner = spatial::project(ctx.snapshot()).map(|p| p.id);
    crate::structure::create(ctx, key, owner, values)
}
