//! The commands that change the spatial structure: `archbuilder.project.new`,
//! `archbuilder.storey.add`, `archbuilder.storey.rename`, `archbuilder.storey.delete`,
//! `archbuilder.building.rename`, `archbuilder.level.set_elevation`, `archbuilder.object.rename`,
//! `archbuilder.object.select` (the selection command of the outliner and the viewport) and
//! `archbuilder.property.set` (the edit command of the property grid).
//!
//! Each command that changes the document is one transaction, so one undo step. Before a command returns it runs the model's
//! rules ([`archbuilder_model::validate::check`]) on its open transaction, so a refused change
//! reports the rule and leaves the document unchanged.

use arc_base::{Dimension, Error, ObjectId, Result};
use arc_command::{
    CommandSpec, Context, IdParam, Permission, QuantityParam, Registry, UndoBehavior,
};
use arc_doc::{FieldPath, Header, Object, Snapshot, Value, ValueType};
use archbuilder_model::spatial::{self, StoreyLevel};
use archbuilder_model::types::{
    self, BUILDING, ELEVATION_BASE_PROJECT, LEVEL, PROJECT, SITE, STOREY, WALL, level, storey,
};
use archbuilder_model::units::LengthUnit;
use archbuilder_model::{defaults, names, validate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;

/// The identifier of the new-project command.
pub const PROJECT_NEW: &str = "archbuilder.project.new";
/// The identifier of the add-storey command.
pub const STOREY_ADD: &str = "archbuilder.storey.add";
/// The identifier of the rename-storey command.
pub const STOREY_RENAME: &str = "archbuilder.storey.rename";
/// The identifier of the delete-storey command.
pub const STOREY_DELETE: &str = "archbuilder.storey.delete";
/// The identifier of the rename-building command.
pub const BUILDING_RENAME: &str = "archbuilder.building.rename";
/// The identifier of the set-level-elevation command.
pub const LEVEL_SET_ELEVATION: &str = "archbuilder.level.set_elevation";
/// The identifier of the generic rename command of the outliner.
pub const OBJECT_RENAME: &str = "archbuilder.object.rename";
/// The identifier of the property grid's edit command.
pub const PROPERTY_SET: &str = "archbuilder.property.set";
/// The identifier of the selection command of the outliner and the viewport.
pub const OBJECT_SELECT: &str = "archbuilder.object.select";
/// The session setting that holds the language tag of the interface (`en`, `tr`), which names
/// the objects a command creates. The windowed application keeps it current; headless runs
/// have none and get English names.
pub const LANGUAGE_SETTING: &str = "app.language";

/// A length unit as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnitParam {
    /// Meters.
    Meter,
    /// Millimeters.
    Millimeter,
    /// Centimeters.
    Centimeter,
    /// Feet.
    Foot,
    /// Inches.
    Inch,
}

impl From<LengthUnitParam> for LengthUnit {
    fn from(unit: LengthUnitParam) -> Self {
        match unit {
            LengthUnitParam::Meter => Self::Meter,
            LengthUnitParam::Millimeter => Self::Millimeter,
            LengthUnitParam::Centimeter => Self::Centimeter,
            LengthUnitParam::Foot => Self::Foot,
            LengthUnitParam::Inch => Self::Inch,
        }
    }
}

/// Parameters of `archbuilder.project.new`; both are optional.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NewProject {
    /// The project name.
    #[serde(default)]
    pub name: Option<String>,
    /// The length unit lengths are shown and typed in; meters when absent.
    #[serde(default)]
    pub length_unit: Option<LengthUnitParam>,
}

/// What `archbuilder.project.new` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ProjectCreated {
    /// The project.
    pub project: IdParam,
    /// The site.
    pub site: IdParam,
    /// The building.
    pub building: IdParam,
    /// The storeys, from the ground up.
    pub storeys: Vec<IdParam>,
}

/// How `archbuilder.object.select` combines its objects with the selection.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SelectMode {
    /// The objects become the selection.
    #[default]
    Replace,
    /// The objects are added.
    Add,
    /// Each object is added when absent and removed when present.
    Toggle,
}

/// Parameters of `archbuilder.object.select`, the shape of the selection intents of the outliner
/// and the viewport.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectObjects {
    /// The objects; a level outline stands for its storey.
    #[schemars(length(max = 100000))]
    pub objects: Vec<IdParam>,
    /// The combination.
    #[serde(default)]
    pub mode: SelectMode,
    /// The viewport gesture that produced the selection.
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub gesture: Option<String>,
}

/// Parameters of `archbuilder.storey.add`; all are optional.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddStorey {
    /// The building; else the building of the selection, else the first building.
    #[serde(default)]
    pub building: Option<IdParam>,
    /// The storey name; else the first free `Floor <n>`.
    #[serde(default)]
    pub name: Option<String>,
    /// The elevation of the storey's level, a number in meters or a text with a unit such as
    /// `"3.2 m"`; else the highest level plus the default storey height.
    #[serde(default)]
    pub elevation: Option<QuantityParam>,
}

/// What `archbuilder.storey.add` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct StoreyAdded {
    /// The new storey.
    pub storey: IdParam,
    /// Its level.
    pub level: IdParam,
}

/// Parameters of `archbuilder.storey.delete`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteStorey {
    /// The storey; else the storey of the selection (a selected level stands for its storey).
    #[serde(default)]
    pub storey: Option<IdParam>,
}

/// Parameters of `archbuilder.storey.rename`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameStorey {
    /// The storey.
    pub storey: IdParam,
    /// The new name.
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
}

/// Parameters of `archbuilder.building.rename`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameBuilding {
    /// The building.
    pub building: IdParam,
    /// The new name.
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
}

/// Parameters of `archbuilder.object.rename`, the shape of the outliner's rename intent.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameObject {
    /// The project, site, building, storey or level.
    pub object: IdParam,
    /// The new name.
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
}

/// Parameters of `archbuilder.level.set_elevation`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetElevation {
    /// The level, or a storey whose level is set.
    pub level: IdParam,
    /// The elevation, a number in meters or a text with a unit such as `"3200 mm"`.
    pub elevation: QuantityParam,
}

/// What `archbuilder.level.set_elevation` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ElevationSet {
    /// The level that changed.
    pub level: IdParam,
    /// Its elevation in meters.
    pub elevation: f64,
}

/// Parameters of `archbuilder.property.set`, the shape of the property grid's edit intent.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetProperty {
    /// The objects to change; each must have the field.
    #[schemars(length(min = 1, max = 1000))]
    pub objects: Vec<IdParam>,
    /// The field: `name` of any object, `elevation` of a level, a storey (its level's) or a
    /// building, `number`, `client`, `address`, `length_unit` or `template` of the project,
    /// `start_x`, `start_y`, `end_x`, `end_y`, `thickness`, `height`, `base_offset` (lengths) or
    /// `location_line` (`center`, `exterior`, `interior`) of a wall.
    pub field: String,
    /// The value in canonical JSON form: a text, a number in meters, an enum name or code, or a
    /// truth value.
    pub value: serde_json::Value,
}

pub(crate) fn named(name: &str) -> Value {
    Value::String(name.trim().to_owned())
}

/// Runs the model's rules on the open transaction.
pub(crate) fn ensure_valid(ctx: &mut Context<'_>) -> Result<()> {
    let txn = ctx.txn()?;
    match validate::check(txn.snapshot(), txn.touched())
        .into_iter()
        .next()
    {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

pub(crate) fn create(
    ctx: &mut Context<'_>,
    key: &str,
    owner: Option<ObjectId>,
    values: Vec<Option<Value>>,
) -> Result<ObjectId> {
    let id: ObjectId = ctx.new_id();
    let header = Header {
        owner,
        ..Header::default()
    };
    ctx.txn()?.create(key, id, header, values)?;
    Ok(id)
}

/// Creates a level and its storey in `building`.
fn create_storey(
    ctx: &mut Context<'_>,
    building: ObjectId,
    name: &str,
    meters: f64,
) -> Result<(ObjectId, ObjectId)> {
    let level = create(
        ctx,
        LEVEL,
        Some(building),
        vec![
            Some(named(name)),
            Some(types::length(meters)),
            Some(Value::Enum(ELEVATION_BASE_PROJECT)),
            Some(Value::Bool(true)),
        ],
    )?;
    let storey = create(
        ctx,
        STOREY,
        Some(building),
        vec![Some(named(name)), Some(Value::Reference(level))],
    )?;
    Ok((storey, level))
}

fn new_project(ctx: &mut Context<'_>, p: NewProject) -> Result<ProjectCreated> {
    let snapshot = ctx.snapshot();
    if types::KEYS
        .iter()
        .any(|key| !spatial::objects(snapshot, key).is_empty())
    {
        return Err(Error::new(codes::NOT_EMPTY));
    }
    let unit = p.length_unit.map_or(LengthUnit::Meter, Into::into);
    let language = language(ctx);
    let names = defaults::names(&language);
    let name = p.name.as_deref().unwrap_or(names.project);
    let project_id = create(
        ctx,
        PROJECT,
        None,
        vec![
            Some(named(name)),
            None,
            None,
            None,
            Some(Value::Enum(unit.code())),
            Some(Value::Bool(false)),
        ],
    )?;
    let site_id = create(ctx, SITE, Some(project_id), vec![Some(named(names.site))])?;
    let building_id = create(
        ctx,
        BUILDING,
        Some(site_id),
        vec![Some(named(names.building)), Some(types::length(0.0)), None],
    )?;
    let mut storeys = Vec::new();
    for (at, storey_name) in names.storeys.iter().enumerate() {
        let meters = defaults::STOREY_HEIGHT * f64::from(u32::try_from(at).unwrap_or(0));
        storeys.push(IdParam(
            create_storey(ctx, building_id, storey_name, meters)?.0,
        ));
    }
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![project_id]);
    Ok(ProjectCreated {
        project: IdParam(project_id),
        site: IdParam(site_id),
        building: IdParam(building_id),
        storeys,
    })
}

/// The building a command works in: the given one, the one the selection is in, or the first.
fn building_for(ctx: &Context<'_>, given: Option<IdParam>) -> Result<ObjectId> {
    let snapshot = ctx.snapshot();
    if let Some(IdParam(id)) = given {
        return expect(snapshot, id, BUILDING).map(|_| id);
    }
    for id in ctx.selection() {
        match spatial::type_of(snapshot, *id).as_deref() {
            Some(BUILDING) => return Ok(*id),
            Some(STOREY | LEVEL) => {
                if let Some(owner) = spatial::get(snapshot, *id).and_then(|o| o.header.owner) {
                    return Ok(owner);
                }
            }
            _ => {}
        }
    }
    spatial::buildings(snapshot)
        .first()
        .map(|b| b.id)
        .ok_or_else(|| Error::new(codes::NO_BUILDING))
}

pub(crate) fn expect(snapshot: &Snapshot, id: ObjectId, key: &str) -> Result<Object> {
    match (spatial::type_of(snapshot, id), spatial::get(snapshot, id)) {
        (Some(found), Some(object)) if found == key => Ok((*object).clone()),
        _ => Err(Error::new(codes::KIND_MISMATCH).with_arg("expected", key)),
    }
}

/// The interface language the application told this session (`app.language`); empty without one.
pub(crate) fn language(ctx: &Context<'_>) -> String {
    ctx.setting(LANGUAGE_SETTING)
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn next_free_name(used: &[StoreyLevel], language: &str) -> String {
    let taken = |name: &str| {
        used.iter().any(|s| {
            types::text(&s.storey.values, storey::NAME).is_some_and(|n| names::same(n, name))
        })
    };
    (used.len()..)
        .map(|n| defaults::floor(language, n))
        .find(|name| !taken(name))
        .unwrap_or_default()
}

fn add_storey(ctx: &mut Context<'_>, p: AddStorey) -> Result<StoreyAdded> {
    let building = building_for(ctx, p.building)?;
    let existing = spatial::storeys(ctx.snapshot(), building);
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => name.to_owned(),
        None => next_free_name(&existing, &language(ctx)),
    };
    let meters = match &p.elevation {
        Some(param) => ctx.quantity(param, Dimension::LENGTH)?.value(),
        None => spatial::levels(ctx.snapshot(), building)
            .last()
            .and_then(|l| types::meters(&l.values, level::ELEVATION))
            .map_or(0.0, |top| top + defaults::STOREY_HEIGHT),
    };
    let (storey_id, level_id) = create_storey(ctx, building, &name, meters)?;
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![storey_id]);
    // The new storey is selected, with its level, which the viewport highlights.
    ctx.set_selection(vec![storey_id, level_id])?;
    Ok(StoreyAdded {
        storey: IdParam(storey_id),
        level: IdParam(level_id),
    })
}

fn rename(ctx: &mut Context<'_>, id: ObjectId, name: &str) -> Result<()> {
    let name = name.trim();
    let snapshot = ctx.snapshot();
    let key = spatial::type_of(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let object = spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    if !types::KEYS.contains(&key.as_str()) {
        return Err(Error::new(codes::KIND_MISMATCH));
    }
    // A storey and its level keep one name until the level is renamed on its own.
    let level = (key == STOREY)
        .then(|| spatial::level_of(snapshot, &object))
        .flatten()
        .filter(|l| {
            let own = |o: &Object| types::text(&o.values, 0).unwrap_or_default().to_owned();
            names::same(&own(l), &own(&object))
        });
    let txn = ctx.txn()?;
    txn.modify(id, FieldPath::new("name"), Some(named(name)))?;
    if let Some(level) = level {
        txn.modify(level.id, FieldPath::new("name"), Some(named(name)))?;
    }
    ensure_valid(ctx)
}

fn rename_storey(ctx: &mut Context<'_>, p: RenameStorey) -> Result<()> {
    expect(ctx.snapshot(), p.storey.0, STOREY)?;
    rename(ctx, p.storey.0, &p.name)
}

fn rename_building(ctx: &mut Context<'_>, p: RenameBuilding) -> Result<()> {
    expect(ctx.snapshot(), p.building.0, BUILDING)?;
    rename(ctx, p.building.0, &p.name)
}

fn rename_object(ctx: &mut Context<'_>, p: RenameObject) -> Result<()> {
    rename(ctx, p.object.0, &p.name)
}

/// The level `id` names, or the level of the storey it names.
fn level_of(snapshot: &Snapshot, id: ObjectId) -> Result<ObjectId> {
    match spatial::type_of(snapshot, id).as_deref() {
        Some(LEVEL) => Ok(id),
        Some(STOREY) => spatial::get(snapshot, id)
            .and_then(|s| spatial::level_of(snapshot, &s))
            .map(|l| l.id)
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH).with_arg("expected", LEVEL)),
        _ => Err(Error::new(codes::KIND_MISMATCH).with_arg("expected", LEVEL)),
    }
}

fn set_level_elevation(
    ctx: &mut Context<'_>,
    id: ObjectId,
    elevation: &QuantityParam,
) -> Result<ElevationSet> {
    let level_id = level_of(ctx.snapshot(), id)?;
    let meters = ctx.quantity(elevation, Dimension::LENGTH)?.value();
    ctx.txn()?.modify(
        level_id,
        FieldPath::new("elevation"),
        Some(types::length(meters)),
    )?;
    ensure_valid(ctx)?;
    Ok(ElevationSet {
        level: IdParam(level_id),
        elevation: meters,
    })
}

fn set_elevation(ctx: &mut Context<'_>, p: SetElevation) -> Result<ElevationSet> {
    set_level_elevation(ctx, p.level.0, &p.elevation)
}

/// The storey the selection names: a selected storey, or the storey of a selected level.
fn storey_in_selection(ctx: &Context<'_>) -> Option<ObjectId> {
    let snapshot = ctx.snapshot();
    ctx.selection()
        .iter()
        .find_map(|id| match spatial::type_of(snapshot, *id).as_deref() {
            Some(STOREY) => Some(*id),
            Some(LEVEL) => spatial::objects(snapshot, STOREY)
                .into_iter()
                .find_map(|s| {
                    spatial::level_of(snapshot, &s)
                        .filter(|l| l.id == *id)
                        .map(|_| s.id)
                }),
            _ => None,
        })
}

pub(crate) fn delete_storey(ctx: &mut Context<'_>, p: DeleteStorey) -> Result<()> {
    let id = match p.storey {
        Some(IdParam(id)) => id,
        None => storey_in_selection(ctx).ok_or_else(|| Error::new(codes::NOTHING_SELECTED))?,
    };
    let object = expect(ctx.snapshot(), id, STOREY)?;
    let walls: usize = types::STOREY_ELEMENTS
        .iter()
        .map(|k| spatial::children(ctx.snapshot(), k, id).len())
        .sum();
    if walls > 0 {
        return Err(Error::new(codes::STOREY_HAS_WALLS).with_arg("count", walls.to_string()));
    }
    if let Some(building) = object.header.owner
        && spatial::storeys(ctx.snapshot(), building).len() <= 1
    {
        return Err(Error::new(codes::LAST_STOREY));
    }
    let level = spatial::level_of(ctx.snapshot(), &object);
    // Walls whose top follows the level that goes keep the height they have now.
    let detached: Vec<(ObjectId, f64)> = level
        .as_ref()
        .map(|l| {
            let snapshot = ctx.snapshot();
            spatial::objects(snapshot, WALL)
                .iter()
                .filter(|w| archbuilder_model::wall::Wall::top_level(&w.values) == Some(l.id))
                .filter_map(|w| {
                    archbuilder_model::wall::Wall::of(snapshot, w).map(|d| (w.id, d.height))
                })
                .collect()
        })
        .unwrap_or_default();
    let limits = *ctx.limits();
    let txn = ctx.txn()?;
    for (wall, height) in detached {
        txn.modify(wall, FieldPath::new("height"), Some(types::length(height)))?;
        txn.modify(wall, FieldPath::new("top_level"), None)?;
    }
    txn.delete(id)?;
    // The storey's own level goes with it; a level flagged as no storey level stays.
    if let Some(level) = level
        && matches!(
            level.values.get(level::STOREY),
            Some(Some(Value::Bool(true)))
        )
    {
        txn.delete(level.id)?;
    }
    txn.apply_delete_rules(&limits)?;
    ensure_valid(ctx)
}

/// The storey a level is the level of.
fn storey_of_level(snapshot: &Snapshot, level: ObjectId) -> Option<ObjectId> {
    spatial::objects(snapshot, STOREY)
        .into_iter()
        .find_map(|s| {
            spatial::level_of(snapshot, &s)
                .filter(|l| l.id == level)
                .map(|_| s.id)
        })
}

/// What selecting `id` selects: a storey with its level, a level outline as its storey with its
/// level, any other object as itself.
fn selected_with_level(snapshot: &Snapshot, id: ObjectId) -> Vec<ObjectId> {
    let storey = match spatial::type_of(snapshot, id).as_deref() {
        Some(LEVEL) => storey_of_level(snapshot, id),
        Some(STOREY) => Some(id),
        _ => None,
    };
    let Some(storey) = storey else {
        return vec![id];
    };
    let level = spatial::get(snapshot, storey).and_then(|s| spatial::level_of(snapshot, &s));
    std::iter::once(storey).chain(level.map(|l| l.id)).collect()
}

fn select_objects(ctx: &mut Context<'_>, p: SelectObjects) -> Result<()> {
    let snapshot = ctx.snapshot();
    let current = ctx.selection().to_vec();
    let mut chosen: Vec<ObjectId> = match p.mode {
        SelectMode::Replace => Vec::new(),
        SelectMode::Add | SelectMode::Toggle => current,
    };
    for IdParam(id) in p.objects {
        let unit = selected_with_level(snapshot, id);
        let present = chosen.contains(&unit[0]);
        if present && matches!(p.mode, SelectMode::Toggle) {
            chosen.retain(|c| !unit.contains(c));
        } else {
            for u in unit {
                if !chosen.contains(&u) {
                    chosen.push(u);
                }
            }
        }
    }
    // A band that takes a wall takes the doors and windows in it: they lie inside the wall.
    if matches!(p.gesture.as_deref(), Some("window" | "crossing"))
        && !matches!(p.mode, SelectMode::Toggle)
    {
        let walls: Vec<ObjectId> = chosen
            .iter()
            .copied()
            .filter(|id| spatial::type_of(snapshot, *id).as_deref() == Some(WALL))
            .collect();
        for wall in walls {
            for (object, _) in archbuilder_model::insert::hosted(snapshot, wall) {
                if !chosen.contains(&object.id) {
                    chosen.push(object.id);
                }
            }
        }
    }
    ctx.set_selection(chosen)
}

/// Decodes `json` for a field of `value_type`.
fn decode(ctx: &Context<'_>, value_type: &ValueType, json: &serde_json::Value) -> Option<Value> {
    match value_type {
        ValueType::String => Some(Value::String(json.as_str()?.trim().to_owned())),
        ValueType::Bool => Some(Value::Bool(json.as_bool()?)),
        ValueType::Enum(variants) => {
            let code = match json.as_str() {
                Some(name) => variants.iter().find(|v| v.name == name)?.code,
                None => u32::try_from(json.as_u64()?).ok()?,
            };
            variants
                .iter()
                .any(|v| v.code == code)
                .then_some(Value::Enum(code))
        }
        ValueType::Quantity(dimension) => {
            let param: QuantityParam = serde_json::from_value(json.clone()).ok()?;
            ctx.quantity(&param, *dimension).ok().map(Value::Quantity)
        }
        _ => None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per special field of the property command"
)]
fn set_property(ctx: &mut Context<'_>, p: SetProperty) -> Result<()> {
    for IdParam(id) in p.objects {
        let snapshot = ctx.snapshot();
        let key = spatial::type_of(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        match (key.as_str(), p.field.as_str()) {
            (_, "name") => {
                let name = p
                    .value
                    .as_str()
                    .ok_or_else(|| Error::new(codes::VALUE_INVALID))?;
                rename(ctx, id, name)?;
            }
            (WALL, "location_line") => {
                let code = match decode(
                    ctx,
                    &ValueType::Enum(
                        archbuilder_model::wall::LocationLine::ALL
                            .iter()
                            .map(|l| arc_doc::EnumVariant::new(l.name(), l.code()))
                            .collect(),
                    ),
                    &p.value,
                ) {
                    Some(Value::Enum(code)) => code,
                    _ => return Err(Error::new(codes::VALUE_INVALID)),
                };
                // The body stays where it is: the baseline moves by the difference of the offsets.
                let object =
                    spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let wall = archbuilder_model::wall::Wall::from_values(&object.values)
                    .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let moved =
                    wall.with_location(archbuilder_model::wall::LocationLine::from_code(code));
                let txn = ctx.txn()?;
                for (field, value) in [
                    ("start_x", moved.start[0]),
                    ("start_y", moved.start[1]),
                    ("end_x", moved.end[0]),
                    ("end_y", moved.end[1]),
                ] {
                    txn.modify(id, FieldPath::new(field), Some(types::length(value)))?;
                }
                txn.modify(id, FieldPath::new("location_line"), Some(Value::Enum(code)))?;
                ensure_valid(ctx)?;
            }
            (WALL, "top_constraint") => {
                let object =
                    spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let wall = archbuilder_model::wall::Wall::of(snapshot, &object)
                    .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let above = archbuilder_model::wall::Wall::levels_above(snapshot, &object);
                // The grid sends the number of its choice (0 is unconnected, then the levels
                // above in order); a level name or `unconnected` also does.
                let top = match &p.value {
                    serde_json::Value::Number(n) => match n.as_u64() {
                        Some(0) => None,
                        Some(k) => Some(
                            usize::try_from(k - 1)
                                .ok()
                                .and_then(|k| above.get(k))
                                .ok_or_else(|| Error::new(codes::VALUE_INVALID))?
                                .id,
                        ),
                        None => return Err(Error::new(codes::VALUE_INVALID)),
                    },
                    serde_json::Value::String(name) if name == "unconnected" => None,
                    serde_json::Value::String(name) => Some(
                        above
                            .iter()
                            .find(|l| types::text(&l.values, level::NAME) == Some(name.as_str()))
                            .ok_or_else(|| Error::new(codes::VALUE_INVALID))?
                            .id,
                    ),
                    _ => return Err(Error::new(codes::VALUE_INVALID)),
                };
                let height = match top.and_then(|t| spatial::get(snapshot, t)) {
                    Some(l) => {
                        let base = spatial::get(snapshot, object.header.owner.unwrap_or(id))
                            .map_or(0.0, |s| spatial::storey_elevation(snapshot, &s))
                            + wall.base_offset;
                        types::meters(&l.values, level::ELEVATION).map_or(wall.height, |e| e - base)
                    }
                    None => wall.height,
                };
                let txn = ctx.txn()?;
                txn.modify(
                    id,
                    FieldPath::new("height"),
                    Some(types::length(height.max(types::HEIGHT_RANGE.0))),
                )?;
                txn.modify(id, FieldPath::new("top_level"), top.map(Value::Reference))?;
                ensure_valid(ctx)?;
            }
            (types::STAIR, "top_constraint") => {
                // 0 is the level above the storey; k is the k-th level above it.
                let object =
                    spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let storey = object
                    .header
                    .owner
                    .and_then(|s| spatial::get(snapshot, s))
                    .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let above = archbuilder_model::room::levels_above(snapshot, &storey);
                let top = match p.value.as_u64() {
                    Some(0) => None,
                    Some(k) => Some(
                        usize::try_from(k - 1)
                            .ok()
                            .and_then(|k| above.get(k))
                            .ok_or_else(|| Error::new(codes::VALUE_INVALID))?
                            .id,
                    ),
                    None => return Err(Error::new(codes::VALUE_INVALID)),
                };
                ctx.txn()?
                    .modify(id, FieldPath::new("top_level"), top.map(Value::Reference))?;
                ensure_valid(ctx)?;
            }
            (types::ROOM, "upper_constraint") => {
                // 0 is the level above the storey; k is the k-th level above it.
                let object =
                    spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let storey = object
                    .header
                    .owner
                    .and_then(|s| spatial::get(snapshot, s))
                    .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
                let above = archbuilder_model::room::levels_above(snapshot, &storey);
                let upper = match &p.value {
                    serde_json::Value::Number(n) => match n.as_u64() {
                        Some(0) => None,
                        Some(k) => Some(
                            usize::try_from(k - 1)
                                .ok()
                                .and_then(|k| above.get(k))
                                .ok_or_else(|| Error::new(codes::VALUE_INVALID))?
                                .id,
                        ),
                        None => return Err(Error::new(codes::VALUE_INVALID)),
                    },
                    _ => return Err(Error::new(codes::VALUE_INVALID)),
                };
                ctx.txn()?.modify(
                    id,
                    FieldPath::new("upper_level"),
                    upper.map(Value::Reference),
                )?;
                ensure_valid(ctx)?;
            }
            (WALL, "height") => {
                // A height leaves the top unconnected; the wall keeps the stored value.
                let value = decode(
                    ctx,
                    &ValueType::Quantity(arc_base::Dimension::LENGTH),
                    &p.value,
                )
                .ok_or_else(|| Error::new(codes::VALUE_INVALID))?;
                let txn = ctx.txn()?;
                txn.modify(id, FieldPath::new("height"), Some(value))?;
                txn.modify(id, FieldPath::new("top_level"), None)?;
                ensure_valid(ctx)?;
            }
            (LEVEL | STOREY, "elevation") => {
                let param: QuantityParam = serde_json::from_value(p.value.clone())
                    .map_err(|_| Error::new(codes::VALUE_INVALID))?;
                set_level_elevation(ctx, id, &param)?;
            }
            _ => {
                let decl = snapshot
                    .registry()
                    .get(&key)
                    .and_then(|d| d.schema.fields.iter().find(|f| f.name == p.field))
                    .ok_or_else(|| {
                        Error::new(codes::FIELD_UNKNOWN).with_arg("field", p.field.as_str())
                    })?;
                let value =
                    if decl.optional && p.value.as_str().is_some_and(|s| s.trim().is_empty()) {
                        None
                    } else {
                        Some(
                            decode(ctx, &decl.value_type, &p.value)
                                .ok_or_else(|| Error::new(codes::VALUE_INVALID))?,
                        )
                    };
                ctx.txn()?
                    .modify(id, FieldPath::new(p.field.as_str()), value)?;
                ensure_valid(ctx)?;
            }
        }
    }
    Ok(())
}

/// A message of the catalog, named by identifier.
struct Message {
    message_id: &'static str,
}

fn spec<P, R, F>(id: &str, label: Message, description: Message, run: F) -> CommandSpec
where
    P: serde::de::DeserializeOwned + JsonSchema,
    R: Serialize + JsonSchema,
    F: Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static,
{
    CommandSpec::new(id, label.message_id, description.message_id, run)
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    let specs = [
        spec(
            PROJECT_NEW,
            Message {
                message_id: "ab-cmd-project-new-label",
            },
            Message {
                message_id: "ab-cmd-project-new-description",
            },
            new_project,
        ),
        spec(
            STOREY_ADD,
            Message {
                message_id: "ab-cmd-storey-add-label",
            },
            Message {
                message_id: "ab-cmd-storey-add-description",
            },
            add_storey,
        ),
        spec(
            STOREY_RENAME,
            Message {
                message_id: "ab-cmd-storey-rename-label",
            },
            Message {
                message_id: "ab-cmd-storey-rename-description",
            },
            rename_storey,
        ),
        spec(
            STOREY_DELETE,
            Message {
                message_id: "ab-cmd-storey-delete-label",
            },
            Message {
                message_id: "ab-cmd-storey-delete-description",
            },
            delete_storey,
        ),
        spec(
            BUILDING_RENAME,
            Message {
                message_id: "ab-cmd-building-rename-label",
            },
            Message {
                message_id: "ab-cmd-building-rename-description",
            },
            rename_building,
        ),
        spec(
            LEVEL_SET_ELEVATION,
            Message {
                message_id: "ab-cmd-level-set-elevation-label",
            },
            Message {
                message_id: "ab-cmd-level-set-elevation-description",
            },
            set_elevation,
        ),
        spec(
            OBJECT_RENAME,
            Message {
                message_id: "ab-cmd-object-rename-label",
            },
            Message {
                message_id: "ab-cmd-object-rename-description",
            },
            rename_object,
        ),
        spec(
            OBJECT_SELECT,
            Message {
                message_id: "ab-cmd-object-select-label",
            },
            Message {
                message_id: "ab-cmd-object-select-description",
            },
            |ctx: &mut Context<'_>, p: SelectObjects| select_objects(ctx, p),
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read]),
        spec(
            PROPERTY_SET,
            Message {
                message_id: "ab-cmd-property-set-label",
            },
            Message {
                message_id: "ab-cmd-property-set-description",
            },
            set_property,
        ),
    ];
    for spec in specs {
        registry.register(spec)?;
    }
    Ok(())
}
