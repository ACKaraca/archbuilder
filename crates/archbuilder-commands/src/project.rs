//! The commands of the project record: `archbuilder.project.set_units` (AB-DATUM-004).
//!
//! The values live on the project record ([`archbuilder_model::types::project`]); a command that
//! is given nothing to change selects the project, whose properties list the same fields.

use arc_base::{Error, ObjectId, Result};
use arc_command::{CommandSpec, Context, Registry};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::spatial;
use archbuilder_model::types::PROJECT;
use archbuilder_model::units::{
    AngleUnit, AreaUnit, LengthUnit, MAX_PRECISION, ROUNDING_RANGE, SlopeUnit, VolumeUnit,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;

/// The identifier of the project information command.
pub const PROJECT_SET_INFO: &str = "archbuilder.project.set_info";
/// The identifier of the display units command.
pub const PROJECT_SET_UNITS: &str = "archbuilder.project.set_units";

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

pub(crate) fn project_of(snapshot: &Snapshot) -> Result<ObjectId> {
    spatial::objects(snapshot, PROJECT)
        .first()
        .map(|p| p.id)
        .ok_or_else(|| Error::new(codes::KIND_MISMATCH))
}

/// Parameters of `archbuilder.project.set_info` (AB-DATUM-002): each text that is given replaces
/// the field, an empty text clears it, and a field left out stays. The project name cannot be
/// cleared.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetInfo {
    /// The project name.
    #[serde(default)]
    pub name: Option<String>,
    /// The project number.
    #[serde(default)]
    pub number: Option<String>,
    /// The client.
    #[serde(default)]
    pub client: Option<String>,
    /// The address of the project, the site and the building.
    #[serde(default)]
    pub address: Option<String>,
    /// The status, a short phase name such as `Design` or `Permit`.
    #[serde(default)]
    pub status: Option<String>,
    /// The issue date, `YYYY-MM-DD`.
    #[serde(default)]
    pub issue_date: Option<String>,
    /// The author.
    #[serde(default)]
    pub author: Option<String>,
    /// The organization that issues the project.
    #[serde(default)]
    pub organization: Option<String>,
    /// A description of the project.
    #[serde(default)]
    pub description: Option<String>,
    /// The long name of the project.
    #[serde(default)]
    pub long_name: Option<String>,
    /// What the building is used for, in words.
    #[serde(default)]
    pub occupancy_type: Option<String>,
    /// The year of construction, four digits.
    #[serde(default)]
    pub year_of_construction: Option<String>,
    /// How the building is constructed, in words.
    #[serde(default)]
    pub construction_method: Option<String>,
}

/// What `archbuilder.project.set_info` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct InfoSet {
    /// The names of the fields that were changed.
    pub changed: Vec<String>,
}

fn set_info(ctx: &mut Context<'_>, p: SetInfo) -> Result<InfoSet> {
    let project = project_of(ctx.snapshot())?;
    let given = [
        ("name", p.name),
        ("number", p.number),
        ("client", p.client),
        ("address", p.address),
        ("status", p.status),
        ("issue_date", p.issue_date),
        ("author", p.author),
        ("organization", p.organization),
        ("description", p.description),
        ("long_name", p.long_name),
        ("occupancy_type", p.occupancy_type),
        ("year_of_construction", p.year_of_construction),
        ("construction_method", p.construction_method),
    ];
    // Nothing to change selects the project, whose properties list the same fields.
    if given.iter().all(|(_, v)| v.is_none()) {
        ctx.set_selection(vec![project])?;
        return Ok(InfoSet {
            changed: Vec::new(),
        });
    }
    let mut changed = Vec::new();
    for (field, text) in given {
        let Some(text) = text else { continue };
        let text = text.trim();
        let value = (!text.is_empty() || field == "name").then(|| Value::String(text.to_owned()));
        ctx.txn()?.modify(project, FieldPath::new(field), value)?;
        changed.push(field.to_owned());
    }
    crate::structure::ensure_valid(ctx)?;
    Ok(InfoSet { changed })
}

/// Parameters of `archbuilder.project.set_units`. Stored values stay SI; the units only decide
/// how values are shown and how a typed number is read.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetUnits {
    /// The length unit: `meter`, `millimeter`, `centimeter`, `foot` or `inch`.
    #[serde(default)]
    pub length_unit: Option<String>,
    /// The area unit: `square_meter`, `square_centimeter`, `square_millimeter` or `square_foot`.
    #[serde(default)]
    pub area_unit: Option<String>,
    /// The volume unit: `cubic_meter`, `liter`, `cubic_foot` or `cubic_yard`.
    #[serde(default)]
    pub volume_unit: Option<String>,
    /// The angle unit: `degree`, `radian` or `gradian`.
    #[serde(default)]
    pub angle_unit: Option<String>,
    /// The slope unit: `degrees`, `percent` or `rise_run`.
    #[serde(default)]
    pub slope_unit: Option<String>,
    /// The decimals of lengths, 0 to 8.
    #[serde(default)]
    pub length_precision: Option<u8>,
    /// The decimals of areas, 0 to 8.
    #[serde(default)]
    pub area_precision: Option<u8>,
    /// The decimals of volumes, 0 to 8.
    #[serde(default)]
    pub volume_precision: Option<u8>,
    /// The decimals of angles, 0 to 8.
    #[serde(default)]
    pub angle_precision: Option<u8>,
    /// The decimals of slopes, 0 to 8.
    #[serde(default)]
    pub slope_precision: Option<u8>,
    /// Lengths round to a multiple of this, in the length unit.
    #[serde(default)]
    pub length_rounding: Option<f64>,
    /// Areas round to a multiple of this, in the area unit.
    #[serde(default)]
    pub area_rounding: Option<f64>,
    /// Volumes round to a multiple of this, in the volume unit.
    #[serde(default)]
    pub volume_rounding: Option<f64>,
    /// Angles round to a multiple of this, in the angle unit.
    #[serde(default)]
    pub angle_rounding: Option<f64>,
    /// Slopes round to a multiple of this, in the slope unit.
    #[serde(default)]
    pub slope_rounding: Option<f64>,
    /// Clears every precision and rounding first, so each unit shows its own defaults again.
    #[serde(default)]
    pub reset: Option<bool>,
}

/// What `archbuilder.project.set_units` returns: the units now in force.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UnitsSet {
    /// The length unit.
    pub length_unit: String,
    /// The area unit.
    pub area_unit: String,
    /// The volume unit.
    pub volume_unit: String,
    /// The angle unit.
    pub angle_unit: String,
    /// The slope unit.
    pub slope_unit: String,
}

fn code_of(name: &str, names: &[(&'static str, u32)]) -> Result<Value> {
    names
        .iter()
        .find(|(n, _)| *n == name.trim())
        .map(|(_, code)| Value::Enum(*code))
        .ok_or_else(|| Error::new(codes::VALUE_INVALID))
}

fn describe(snapshot: &Snapshot) -> UnitsSet {
    let u = spatial::unit_settings(snapshot);
    UnitsSet {
        length_unit: u.length.name().to_owned(),
        area_unit: u.area.name().to_owned(),
        volume_unit: u.volume.name().to_owned(),
        angle_unit: u.angle.name().to_owned(),
        slope_unit: u.slope.name().to_owned(),
    }
}

fn set_units(ctx: &mut Context<'_>, p: SetUnits) -> Result<UnitsSet> {
    let project = project_of(ctx.snapshot())?;
    let mut changes: Vec<(&str, Option<Value>)> = Vec::new();
    if p.reset == Some(true) {
        for name in [
            "length_precision",
            "area_precision",
            "volume_precision",
            "angle_precision",
            "slope_precision",
            "length_rounding",
            "area_rounding",
            "volume_rounding",
            "angle_rounding",
            "slope_rounding",
        ] {
            changes.push((name, None));
        }
    }
    macro_rules! unit {
        ($field:ident, $name:literal, $ty:ty) => {
            if let Some(text) = &p.$field {
                let names: Vec<(&'static str, u32)> =
                    <$ty>::ALL.iter().map(|u| (u.name(), u.code())).collect();
                changes.push(($name, Some(code_of(text, &names)?)));
            }
        };
    }
    unit!(length_unit, "length_unit", LengthUnit);
    unit!(area_unit, "area_unit", AreaUnit);
    unit!(volume_unit, "volume_unit", VolumeUnit);
    unit!(angle_unit, "angle_unit", AngleUnit);
    unit!(slope_unit, "slope_unit", SlopeUnit);
    for (name, value) in [
        ("length_precision", p.length_precision),
        ("area_precision", p.area_precision),
        ("volume_precision", p.volume_precision),
        ("angle_precision", p.angle_precision),
        ("slope_precision", p.slope_precision),
    ] {
        if let Some(v) = value {
            if i64::from(v) > MAX_PRECISION {
                return Err(Error::new(codes::VALUE_INVALID));
            }
            changes.retain(|(n, _)| *n != name);
            changes.push((name, Some(Value::I64(i64::from(v)))));
        }
    }
    for (name, value) in [
        ("length_rounding", p.length_rounding),
        ("area_rounding", p.area_rounding),
        ("volume_rounding", p.volume_rounding),
        ("angle_rounding", p.angle_rounding),
        ("slope_rounding", p.slope_rounding),
    ] {
        if let Some(v) = value {
            if !v.is_finite() || v < ROUNDING_RANGE.0 || v > ROUNDING_RANGE.1 {
                return Err(Error::new(codes::VALUE_INVALID));
            }
            changes.retain(|(n, _)| *n != name);
            changes.push((name, Some(Value::F64(v))));
        }
    }
    // Nothing to change selects the project, whose properties list the same fields.
    if changes.is_empty() {
        ctx.set_selection(vec![project])?;
        return Ok(describe(ctx.snapshot()));
    }
    let txn = ctx.txn()?;
    for (field, value) in changes {
        txn.modify(project, FieldPath::new(field), value)?;
    }
    crate::structure::ensure_valid(ctx)?;
    Ok(describe(ctx.snapshot()))
}

/// Registers the project commands.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(CommandSpec::new(
        PROJECT_SET_INFO,
        Msg {
            message_id: "ab-cmd-project-set-info-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-project-set-info-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: SetInfo| set_info(ctx, p),
    ))?;
    registry.register(CommandSpec::new(
        PROJECT_SET_UNITS,
        Msg {
            message_id: "ab-cmd-project-set-units-label",
        }
        .message_id,
        Msg {
            message_id: "ab-cmd-project-set-units-description",
        }
        .message_id,
        |ctx: &mut Context<'_>, p: SetUnits| set_units(ctx, p),
    ))
}
