//! The commands of curtain walls: `archbuilder.curtain.type` (make or edit a curtain wall type:
//! grid rules, system panel, mullions), `archbuilder.curtain.grid_add`, `grid_remove` and
//! `grid_move` (per-wall grid edits; a move with no destination pins the line where it is) and
//! `archbuilder.curtain.panel` (a per-cell panel override, a door or window included). A curtain
//! wall itself is drawn with `archbuilder.wall.create` and a curtain type.
//!
//! The edits are authored data of the wall record, keyed by line and cell keys that do not depend
//! on positions; an override whose cell an edit dissolved is dropped in the same transaction.

use arc_base::{Dimension, Error, Result};
use arc_command::{CommandSpec, Context, IdParam, QuantityParam, Registry};
use arc_doc::FieldPath;
use archbuilder_model::curtain::{
    CellOverride, CurtainType, Derived, Direction, EditKind, GridEdit, Justify, Layout, MAX_EDITS,
    MAX_OVERRIDES, MullionSpec, PanelKind, Priority, Profile, RuleKind,
};
use archbuilder_model::error::codes as model;
use archbuilder_model::spatial;
use archbuilder_model::types::{self, DOOR_TYPE, WALL, WINDOW_TYPE};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::element_type::{create_curtain_type, set_curtain_type};
use crate::structure::{ensure_valid, expect};

/// The identifier of the command that makes or edits a curtain wall type.
pub const CURTAIN_TYPE: &str = "archbuilder.curtain.type";
/// The identifier of the command that adds a grid line.
pub const CURTAIN_GRID_ADD: &str = "archbuilder.curtain.grid_add";
/// The identifier of the command that removes a grid line or a segment of one.
pub const CURTAIN_GRID_REMOVE: &str = "archbuilder.curtain.grid_remove";
/// The identifier of the command that moves or pins a grid line.
pub const CURTAIN_GRID_MOVE: &str = "archbuilder.curtain.grid_move";
/// The identifier of the command that sets or clears the panel of a cell.
pub const CURTAIN_PANEL: &str = "archbuilder.curtain.panel";

fn invalid() -> Error {
    Error::new(arc_command::error::codes::PARAMS_INVALID)
}

fn pick<T: Copy>(all: &[(&str, T)], name: &str) -> Result<T> {
    all.iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| *v)
        .ok_or_else(invalid)
}

fn rule_kinds() -> Vec<(&'static str, RuleKind)> {
    RuleKind::ALL.iter().map(|k| (k.name(), *k)).collect()
}

/// A grid rule as a command parameter; fields left out keep their value.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleParam {
    /// `fixed_distance`, `fixed_number`, `max_spacing` or `min_spacing`.
    #[serde(default)]
    pub kind: Option<String>,
    /// A distance in meters, or the number of interior lines for `fixed_number`.
    #[serde(default)]
    pub value: Option<f64>,
    /// `start`, `center` or `end`: where a fixed distance starts counting.
    #[serde(default)]
    pub justify: Option<String>,
    /// The shift of the origin of a fixed distance.
    #[serde(default)]
    pub offset: Option<QuantityParam>,
}

/// A mullion specification as a command parameter; fields left out keep their value.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MullionParam {
    /// `none`, `rectangular` or `circular`.
    #[serde(default)]
    pub profile: Option<String>,
    /// The width across the grid line, or the diameter.
    #[serde(default)]
    pub width: Option<QuantityParam>,
    /// The depth across the wall.
    #[serde(default)]
    pub depth: Option<QuantityParam>,
}

/// Parameters of `archbuilder.curtain.type`: without `type` a new type is made from the standard
/// one with the given changes; with it that type changes, and every curtain wall of it with it.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CurtainTypeParams {
    /// The wall type to edit; omit it to make a new curtain wall type.
    #[serde(default, rename = "type")]
    pub element_type: Option<IdParam>,
    /// The name.
    #[serde(default)]
    #[schemars(length(min = 1, max = 256))]
    pub name: Option<String>,
    /// The rule of the vertical lines.
    #[serde(default)]
    pub vertical: Option<RuleParam>,
    /// The rule of the horizontal lines.
    #[serde(default)]
    pub horizontal: Option<RuleParam>,
    /// The system panel: `glazed`, `solid` or `empty`.
    #[serde(default)]
    pub panel: Option<String>,
    /// The thickness of a glazed or solid panel.
    #[serde(default)]
    pub panel_thickness: Option<QuantityParam>,
    /// The material of the panel.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub material: Option<String>,
    /// The mullion on the lines between cells.
    #[serde(default)]
    pub interior: Option<MullionParam>,
    /// The mullion on the borders.
    #[serde(default)]
    pub border: Option<MullionParam>,
    /// The mullion where two curtain walls meet at a corner.
    #[serde(default)]
    pub corner: Option<MullionParam>,
    /// The continuous direction at a crossing: `vertical` or `horizontal`.
    #[serde(default)]
    pub priority: Option<String>,
}

/// What a curtain command returns.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct CurtainChanged {
    /// The type made or changed.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub element_type: Option<IdParam>,
    /// The curtain walls that changed with a type.
    pub walls: usize,
    /// The grid edits the wall holds now.
    pub edits: usize,
    /// The panel overrides the wall holds now.
    pub overrides: usize,
    /// The key of the line or cell the command made or changed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

fn meters(ctx: &Context<'_>, p: &QuantityParam) -> Result<f64> {
    ctx.quantity(p, Dimension::LENGTH).map(|q| q.value())
}

fn apply_rule(
    ctx: &Context<'_>,
    rule: &mut archbuilder_model::curtain::GridRule,
    p: &RuleParam,
) -> Result<()> {
    if let Some(k) = &p.kind {
        rule.kind = pick(&rule_kinds(), k)?;
    }
    if let Some(v) = p.value {
        rule.value = v;
    }
    if let Some(j) = &p.justify {
        let all: Vec<_> = Justify::ALL.iter().map(|k| (k.name(), *k)).collect();
        rule.justify = pick(&all, j)?;
    }
    if let Some(o) = &p.offset {
        rule.offset = meters(ctx, o)?;
    }
    Ok(())
}

fn apply_mullion(ctx: &Context<'_>, spec: &mut MullionSpec, p: &MullionParam) -> Result<()> {
    if let Some(k) = &p.profile {
        let all: Vec<_> = Profile::ALL.iter().map(|k| (k.name(), *k)).collect();
        spec.profile = pick(&all, k)?;
    }
    if let Some(w) = &p.width {
        spec.width = meters(ctx, w)?;
    }
    if let Some(d) = &p.depth {
        spec.depth = meters(ctx, d)?;
    }
    Ok(())
}

fn curtain_type(ctx: &mut Context<'_>, p: CurtainTypeParams) -> Result<CurtainChanged> {
    let existing = match &p.element_type {
        Some(IdParam(id)) => {
            let object = expect(ctx.snapshot(), *id, types::WALL_TYPE)?;
            Some((
                *id,
                CurtainType::from_values(&object.values).ok_or_else(|| {
                    Error::new(model::CURTAIN_NOT_CURTAIN).with_arg("name", id.to_string())
                })?,
            ))
        }
        None => None,
    };
    let mut ty = existing
        .as_ref()
        .map_or_else(CurtainType::standard, |(_, t)| t.clone());
    if let Some(r) = &p.vertical {
        apply_rule(ctx, &mut ty.vertical, r)?;
    }
    if let Some(r) = &p.horizontal {
        apply_rule(ctx, &mut ty.horizontal, r)?;
    }
    if let Some(k) = &p.panel {
        let all: Vec<_> = PanelKind::ALL.iter().map(|k| (k.name(), *k)).collect();
        ty.panel = pick(&all, k)?;
    }
    if let Some(t) = &p.panel_thickness {
        ty.panel_thickness = meters(ctx, t)?;
    }
    if let Some(m) = &p.material {
        ty.material.clone_from(m);
    }
    for (spec, param) in [
        (&mut ty.interior, &p.interior),
        (&mut ty.border, &p.border),
        (&mut ty.corner, &p.corner),
    ] {
        if let Some(m) = param {
            apply_mullion(ctx, spec, m)?;
        }
    }
    if let Some(k) = &p.priority {
        let all: Vec<_> = Priority::ALL.iter().map(|k| (k.name(), *k)).collect();
        ty.priority = pick(&all, k)?;
    }
    let (id, walls) = match existing {
        Some((id, _)) => (id, set_curtain_type(ctx, id, &ty, p.name.as_deref())?),
        None => (create_curtain_type(ctx, &ty, p.name.as_deref())?, 0),
    };
    ensure_valid(ctx)?;
    Ok(CurtainChanged {
        element_type: Some(IdParam(id)),
        walls,
        ..CurtainChanged::default()
    })
}

/// The curtain wall `id` with what it holds now.
struct Held {
    ty: CurtainType,
    length: f64,
    height: f64,
    edits: Vec<GridEdit>,
    overrides: Vec<CellOverride>,
}

fn held(ctx: &Context<'_>, id: arc_base::ObjectId) -> Result<Held> {
    let object = expect(ctx.snapshot(), id, WALL)?;
    let d = Derived::of(ctx.snapshot(), &object).ok_or_else(|| {
        Error::new(model::CURTAIN_NOT_CURTAIN).with_arg(
            "name",
            types::text(&object.values, 0)
                .unwrap_or_default()
                .to_owned(),
        )
    })?;
    Ok(Held {
        ty: d.ty,
        length: d.wall.length(),
        height: d.wall.height,
        edits: GridEdit::list(&object.values),
        overrides: CellOverride::list(&object.values),
    })
}

fn unknown(what: &str) -> Error {
    Error::new(model::CURTAIN_UNKNOWN).with_arg("name", what.to_owned())
}

/// Writes `edits` and, with them, the overrides whose cell the edits dissolved are dropped.
fn store(
    ctx: &mut Context<'_>,
    id: arc_base::ObjectId,
    old: &Held,
    edits: Vec<GridEdit>,
    key: Option<String>,
) -> Result<CurtainChanged> {
    if edits.len() > MAX_EDITS {
        return Err(Error::new(model::CURTAIN_LIMIT).with_arg("name", id.to_string()));
    }
    let before = Layout::of(&old.ty, &old.edits, old.length, old.height);
    let after = Layout::of(&old.ty, &edits, old.length, old.height);
    let overrides: Vec<CellOverride> = old
        .overrides
        .iter()
        .filter(|o| {
            after.cells.iter().any(|c| c.key == o.cell)
                || !before.cells.iter().any(|c| c.key == o.cell)
        })
        .cloned()
        .collect();
    let txn = ctx.txn()?;
    txn.modify(
        id,
        FieldPath::new("grid_edits"),
        Some(GridEdit::list_value(&edits)),
    )?;
    txn.modify(
        id,
        FieldPath::new("cell_overrides"),
        Some(CellOverride::list_value(&overrides)),
    )?;
    ensure_valid(ctx)?;
    Ok(CurtainChanged {
        edits: edits.len(),
        overrides: overrides.len(),
        key,
        ..CurtainChanged::default()
    })
}

fn next_key(edits: &[GridEdit]) -> i64 {
    edits.iter().map(|e| e.key).max().map_or(1, |k| k + 1)
}

fn direction(name: &str) -> Result<Direction> {
    let all: Vec<_> = Direction::ALL.iter().map(|k| (k.name(), *k)).collect();
    pick(&all, name)
}

fn span(
    ctx: &Context<'_>,
    from: &Option<QuantityParam>,
    until: &Option<QuantityParam>,
) -> Result<Option<(f64, f64)>> {
    match (from, until) {
        (None, None) => Ok(None),
        (Some(a), Some(b)) => {
            let (a, b) = (meters(ctx, a)?, meters(ctx, b)?);
            Ok(Some((a.min(b), a.max(b))))
        }
        _ => Err(invalid()),
    }
}

/// Parameters of `archbuilder.curtain.grid_add`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GridAdd {
    /// The curtain wall.
    pub wall: IdParam,
    /// `vertical` (divides the baseline) or `horizontal` (divides the height).
    pub direction: String,
    /// The position: along the baseline from its start, or above the base.
    pub at: QuantityParam,
    /// With `until`: the segment along the other axis the line is limited to; else it spans the
    /// whole wall.
    #[serde(default)]
    pub from: Option<QuantityParam>,
    /// The end of the segment.
    #[serde(default)]
    pub until: Option<QuantityParam>,
}

fn grid_add(ctx: &mut Context<'_>, p: GridAdd) -> Result<CurtainChanged> {
    let IdParam(id) = p.wall;
    let old = held(ctx, id)?;
    let dir = direction(&p.direction)?;
    let at = meters(ctx, &p.at)?;
    let extent = if dir == Direction::Vertical {
        old.length
    } else {
        old.height
    };
    if !(at.is_finite() && at > 0.0 && at < extent) {
        return Err(invalid());
    }
    let key = next_key(&old.edits);
    let mut edits = old.edits.clone();
    edits.push(GridEdit {
        key,
        direction: dir,
        kind: EditKind::Add,
        line: String::new(),
        at,
        span: span(ctx, &p.from, &p.until)?,
    });
    store(ctx, id, &old, edits, Some(format!("a{key}")))
}

/// Parameters of `archbuilder.curtain.grid_remove`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GridRemove {
    /// The curtain wall.
    pub wall: IdParam,
    /// `vertical` or `horizontal`.
    pub direction: String,
    /// The key of the line, as the property grid and `inspect` list it.
    pub line: String,
    /// With `until`: remove only the segment of the line between them, along the other axis.
    #[serde(default)]
    pub from: Option<QuantityParam>,
    /// The end of the segment.
    #[serde(default)]
    pub until: Option<QuantityParam>,
}

fn line_exists(old: &Held, dir: Direction, line: &str) -> bool {
    let layout = Layout::of(&old.ty, &old.edits, old.length, old.height);
    let lines = if dir == Direction::Vertical {
        &layout.vertical
    } else {
        &layout.horizontal
    };
    lines.iter().any(|l| !l.border && l.key == line)
}

fn grid_remove(ctx: &mut Context<'_>, p: GridRemove) -> Result<CurtainChanged> {
    let IdParam(id) = p.wall;
    let old = held(ctx, id)?;
    let dir = direction(&p.direction)?;
    if !line_exists(&old, dir, &p.line) {
        return Err(unknown(&p.line));
    }
    let key = next_key(&old.edits);
    let mut edits = old.edits.clone();
    edits.push(GridEdit {
        key,
        direction: dir,
        kind: EditKind::Remove,
        line: p.line.clone(),
        at: 0.0,
        span: span(ctx, &p.from, &p.until)?,
    });
    store(ctx, id, &old, edits, Some(p.line))
}

/// Parameters of `archbuilder.curtain.grid_move`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GridMove {
    /// The curtain wall.
    pub wall: IdParam,
    /// `vertical` or `horizontal`.
    pub direction: String,
    /// The key of the line.
    pub line: String,
    /// The new position; omit it to pin the line where it is, so it stays when the rule changes.
    #[serde(default)]
    pub to: Option<QuantityParam>,
}

fn grid_move(ctx: &mut Context<'_>, p: GridMove) -> Result<CurtainChanged> {
    let IdParam(id) = p.wall;
    let old = held(ctx, id)?;
    let dir = direction(&p.direction)?;
    let layout = Layout::of(&old.ty, &old.edits, old.length, old.height);
    let lines = if dir == Direction::Vertical {
        &layout.vertical
    } else {
        &layout.horizontal
    };
    let current = lines
        .iter()
        .find(|l| !l.border && l.key == p.line)
        .ok_or_else(|| unknown(&p.line))?
        .at;
    let to = match &p.to {
        Some(q) => meters(ctx, q)?,
        None => current,
    };
    let extent = if dir == Direction::Vertical {
        old.length
    } else {
        old.height
    };
    if !(to.is_finite() && to > 0.0 && to < extent) {
        return Err(invalid());
    }
    let key = next_key(&old.edits);
    // A line added by an edit moves by changing that edit; others get a move edit.
    let mut edits = old.edits.clone();
    edits.push(GridEdit {
        key,
        direction: dir,
        kind: EditKind::Move,
        line: p.line.clone(),
        at: to,
        span: None,
    });
    store(ctx, id, &old, edits, Some(p.line))
}

/// Parameters of `archbuilder.curtain.panel`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PanelSet {
    /// The curtain wall.
    pub wall: IdParam,
    /// The key of the cell, as the property grid and `inspect` list it.
    pub cell: String,
    /// `glazed`, `solid`, `empty`, `door`, `window`, or `default` to clear the override.
    pub panel: String,
    /// For `door` and `window`: the door or window type; else a plain one.
    #[serde(default, rename = "type")]
    pub element_type: Option<IdParam>,
}

fn panel_set(ctx: &mut Context<'_>, p: PanelSet) -> Result<CurtainChanged> {
    let IdParam(id) = p.wall;
    let old = held(ctx, id)?;
    let layout = Layout::of(&old.ty, &old.edits, old.length, old.height);
    if !layout.cells.iter().any(|c| c.key == p.cell) {
        return Err(unknown(&p.cell));
    }
    let mut overrides: Vec<CellOverride> = old
        .overrides
        .iter()
        .filter(|o| o.cell != p.cell)
        .cloned()
        .collect();
    if p.panel != "default" {
        let all: Vec<_> = PanelKind::ALL.iter().map(|k| (k.name(), *k)).collect();
        let kind = pick(&all, &p.panel)?;
        let type_id = match (&p.element_type, kind) {
            (Some(IdParam(t)), PanelKind::Door | PanelKind::Window) => {
                let want = if kind == PanelKind::Door {
                    DOOR_TYPE
                } else {
                    WINDOW_TYPE
                };
                if spatial::type_of(ctx.snapshot(), *t).as_deref() != Some(want) {
                    return Err(unknown(&t.to_string()));
                }
                Some(*t)
            }
            (Some(_), _) => return Err(invalid()),
            (None, _) => None,
        };
        overrides.push(CellOverride {
            cell: p.cell.clone(),
            panel: kind,
            type_id,
        });
    }
    if overrides.len() > MAX_OVERRIDES {
        return Err(Error::new(model::CURTAIN_LIMIT).with_arg("name", id.to_string()));
    }
    ctx.txn()?.modify(
        id,
        FieldPath::new("cell_overrides"),
        Some(CellOverride::list_value(&overrides)),
    )?;
    ensure_valid(ctx)?;
    Ok(CurtainChanged {
        edits: old.edits.len(),
        overrides: overrides.len(),
        key: Some(p.cell),
        ..CurtainChanged::default()
    })
}

fn spec<P, R, F>(id: &str, label: &'static str, description: &'static str, run: F) -> CommandSpec
where
    P: serde::de::DeserializeOwned + JsonSchema,
    R: Serialize + JsonSchema,
    F: Fn(&mut Context<'_>, P) -> Result<R> + Send + Sync + 'static,
{
    CommandSpec::new(id, label, description, run)
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    for s in [
        spec(
            CURTAIN_TYPE,
            "ab-cmd-curtain-type-label",
            "ab-cmd-curtain-type-description",
            curtain_type,
        ),
        spec(
            CURTAIN_GRID_ADD,
            "ab-cmd-curtain-grid-add-label",
            "ab-cmd-curtain-grid-add-description",
            grid_add,
        ),
        spec(
            CURTAIN_GRID_REMOVE,
            "ab-cmd-curtain-grid-remove-label",
            "ab-cmd-curtain-grid-remove-description",
            grid_remove,
        ),
        spec(
            CURTAIN_GRID_MOVE,
            "ab-cmd-curtain-grid-move-label",
            "ab-cmd-curtain-grid-move-description",
            grid_move,
        ),
        spec(
            CURTAIN_PANEL,
            "ab-cmd-curtain-panel-label",
            "ab-cmd-curtain-panel-description",
            panel_set,
        ),
    ] {
        registry.register(s)?;
    }
    Ok(())
}
