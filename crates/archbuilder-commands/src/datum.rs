//! The datum commands: `archbuilder.grid.create`, `archbuilder.reference_plane.create`,
//! `archbuilder.work_plane.set`, `archbuilder.work_plane.show` and `archbuilder.datum.set_extent`.
//!
//! A grid is a labeled line, drawn from two points, an arc (`bulge`) or the baseline of a wall
//! (`from_wall`); without a label it gets the one after the last (`after`, else the highest in
//! the building): `1, 2, 3`, `A, B, C` or `A.1, A.2`. Its extent runs between two levels and
//! each end shows a head or not (`set_extent`). A reference plane is a named vertical plane with
//! a strength. A work plane records where placement works: from a level, grid or reference plane
//! (it then follows that datum) or three points; at most one is active. Moving, copying,
//! rotating and mirroring grids and reference planes is done by the modify commands
//! ([`crate::modify`]).

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::{Accept, PointDims, PromptSequence, PromptStep};
use arc_command::{CommandSpec, Context, IdParam, Registry};
use arc_doc::{FieldPath, Snapshot, Value};
use archbuilder_model::datum::{self, Source, Strength};
use archbuilder_model::types::{self, GRID, LEVEL, REFERENCE_PLANE, WALL, WORK_PLANE};
use archbuilder_model::{defaults, names, spatial};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::{building_for, create, ensure_valid, expect, language, named};

/// The identifier of the create-grid command.
pub const GRID_CREATE: &str = "archbuilder.grid.create";
/// The identifier of the create-reference-plane command.
pub const REFERENCE_PLANE_CREATE: &str = "archbuilder.reference_plane.create";
/// The identifier of the set-work-plane command.
pub const WORK_PLANE_SET: &str = "archbuilder.work_plane.set";
/// The identifier of the show-work-plane command.
pub const WORK_PLANE_SHOW: &str = "archbuilder.work_plane.show";
/// The identifier of the datum extent command.
pub const DATUM_SET_EXTENT: &str = "archbuilder.datum.set_extent";

/// The most grids one extent command works on.
const MAX_OBJECTS: usize = 5_000;

/// Parameters of `archbuilder.grid.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateGrid {
    /// The start `[x, y]` in meters; with `end`, or else `from_wall`.
    #[serde(default)]
    pub start: Option<[f64; 2]>,
    /// The end `[x, y]` in meters.
    #[serde(default)]
    pub end: Option<[f64; 2]>,
    /// A wall whose baseline the grid takes: a grid drawn from an existing curve.
    #[serde(default)]
    pub from_wall: Option<IdParam>,
    /// Walls that follow one another, end to start within a millimeter, in order: the grid runs
    /// along their baselines as one multi-segment line.
    #[serde(default)]
    #[schemars(length(max = 1000))]
    pub from_walls: Option<Vec<IdParam>>,
    /// The points `[x, y]` of a multi-segment grid, in meters: the first is the start, the last
    /// the end, the others vertices; at least two.
    #[serde(default)]
    #[schemars(length(max = 1000))]
    pub points: Option<Vec<[f64; 2]>>,
    /// The bulge of each segment of `points` (one fewer than the points; 0 is straight); the
    /// last segment takes `bulge` when this is absent.
    #[serde(default)]
    #[schemars(length(max = 1000))]
    pub bulges: Option<Vec<f64>>,
    /// The bulge of an arc grid: the tangent of a quarter of its included angle; straight when
    /// absent or 0.
    #[serde(default)]
    pub bulge: Option<f64>,
    /// The label; else the one after `after`, else after the last label of the building.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub label: Option<String>,
    /// The label the automatic one follows.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub after: Option<String>,
    /// The building; else the one of the selection, else the first.
    #[serde(default)]
    pub building: Option<IdParam>,
    /// The level the grid starts at (identifier or name); the lowest when absent.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub bottom_level: Option<String>,
    /// The level the grid ends at (identifier or name); the highest when absent.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub top_level: Option<String>,
}

/// What `archbuilder.grid.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GridCreated {
    /// The new grid.
    pub grid: IdParam,
    /// Its label.
    pub label: String,
}

/// Parameters of `archbuilder.reference_plane.create`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateReferencePlane {
    /// The first point `[x, y]` in meters.
    pub start: [f64; 2],
    /// The second point `[x, y]` in meters.
    pub end: [f64; 2],
    /// The name; else the first free `Reference plane <n>`.
    #[serde(default)]
    pub name: Option<String>,
    /// `strong`, `weak` (the default) or `none`.
    #[serde(default)]
    pub strength: Option<String>,
    /// Whether the plane ends at its points; true when absent.
    #[serde(default)]
    pub bounded: Option<bool>,
    /// The building; else the one of the selection, else the first.
    #[serde(default)]
    pub building: Option<IdParam>,
}

/// What `archbuilder.reference_plane.create` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReferencePlaneCreated {
    /// The new reference plane.
    pub reference_plane: IdParam,
    /// Its name.
    pub name: String,
}

/// Parameters of `archbuilder.work_plane.set`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetWorkPlane {
    /// `level`, `grid`, `reference_plane` or `points`; else what `target` or the selection is.
    #[serde(default)]
    pub source: Option<String>,
    /// The level, grid or reference plane (identifier or name); else the selected one.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub target: Option<String>,
    /// Three points `[x, y, z]` in meters, for a plane from points.
    #[serde(default)]
    pub points: Option<[[f64; 3]; 3]>,
    /// The name of a new plane; else the first free `Work plane <n>`.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether the plane is displayed; true when absent.
    #[serde(default)]
    pub shown: Option<bool>,
    /// The building; else the one of the selection, else the first.
    #[serde(default)]
    pub building: Option<IdParam>,
}

/// What `archbuilder.work_plane.set` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WorkPlaneSet {
    /// The active work plane.
    pub work_plane: IdParam,
    /// Its name.
    pub name: String,
}

/// Parameters of `archbuilder.work_plane.show`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShowWorkPlane {
    /// Whether to display the active work plane; the opposite of now when absent.
    #[serde(default)]
    pub shown: Option<bool>,
    /// The building; else the one of the selection, else the first.
    #[serde(default)]
    pub building: Option<IdParam>,
}

/// Parameters of `archbuilder.datum.set_extent`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetExtent {
    /// The grids; else the selected ones.
    #[serde(default)]
    pub objects: Option<Vec<IdParam>>,
    /// The level the extent starts at (identifier or name); `auto` for the lowest level.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub bottom_level: Option<String>,
    /// The level the extent ends at (identifier or name); `auto` for the highest level.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub top_level: Option<String>,
    /// Whether the start end shows its head.
    #[serde(default)]
    pub head_start: Option<bool>,
    /// Whether the end shows its head.
    #[serde(default)]
    pub head_end: Option<bool>,
}

/// What `archbuilder.datum.set_extent` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ExtentSet {
    /// The grids changed.
    pub objects: usize,
}

fn level_named(snapshot: &Snapshot, building: ObjectId, text: &str) -> Result<ObjectId> {
    spatial::levels(snapshot, building)
        .iter()
        .find(|l| {
            l.id.to_string() == text.trim()
                || types::text(&l.values, types::level::NAME).is_some_and(|n| names::same(n, text))
        })
        .map(|l| l.id)
        .ok_or_else(|| Error::new(codes::VALUE_INVALID))
}

/// A level given as text: `auto` (or nothing) is none.
fn optional_level(
    snapshot: &Snapshot,
    building: ObjectId,
    text: Option<&str>,
) -> Result<Option<ObjectId>> {
    match text.map(str::trim) {
        None | Some("" | "auto") => Ok(None),
        Some(t) => level_named(snapshot, building, t).map(Some),
    }
}

fn point_ok(p: [f64; 2]) -> bool {
    let (lo, hi) = types::COORDINATE_RANGE;
    p.iter().all(|v| v.is_finite() && *v >= lo && *v <= hi)
}

fn create_grid(ctx: &mut Context<'_>, mut p: CreateGrid) -> Result<GridCreated> {
    let building = building_for(ctx, p.building)?;
    let snapshot = ctx.snapshot().clone();
    let baseline = |id: ObjectId| -> Result<[[f64; 2]; 2]> {
        let wall = expect(&snapshot, id, WALL)?;
        archbuilder_model::wall::Wall::from_values(&wall.values)
            .map(|w| [w.start, w.end])
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))
    };
    // The control points of the line and the bulge of each segment.
    let (points, mut bulges): (Vec<[f64; 2]>, Option<Vec<f64>>) =
        match (p.from_walls, p.from_wall, p.points, p.start, p.end) {
            (Some(walls), ..) => {
                if walls.is_empty() || walls.len() > datum::MAX_PATH {
                    return Err(Error::new(codes::NO_LINE));
                }
                let mut points: Vec<[f64; 2]> = Vec::new();
                for IdParam(id) in walls {
                    let [a, b] = baseline(id)?;
                    let joined =
                        |x: [f64; 2], y: [f64; 2]| (x[0] - y[0]).hypot(x[1] - y[1]) < 1.0e-3;
                    // The first wall runs backwards when the second meets its start.
                    if points.len() == 2
                        && let Some(first) = points.first().copied()
                        && (joined(first, a) || joined(first, b))
                        && !points
                            .last()
                            .is_some_and(|l| joined(*l, a) || joined(*l, b))
                    {
                        points.reverse();
                    }
                    match points.last().copied() {
                        None => points.extend([a, b]),
                        Some(last) if joined(last, a) => points.push(b),
                        Some(last) if joined(last, b) => points.push(a),
                        Some(_) => return Err(Error::new(codes::VALUE_INVALID)),
                    }
                }
                (points, None)
            }
            (None, Some(IdParam(id)), ..) => (baseline(id)?.to_vec(), None),
            (None, None, Some(points), ..) => (points, p.bulges.take()),
            (None, None, None, Some(a), Some(b)) => (vec![a, b], None),
            _ => return Err(Error::new(codes::NO_LINE)),
        };
    if points.len() < 2
        || points.len() > datum::MAX_PATH + 2
        || !points.iter().all(|q| point_ok(*q))
    {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    let segments = points.len() - 1;
    let bulges = match bulges.take() {
        Some(list) if list.len() == segments => list,
        Some(_) => return Err(Error::new(codes::VALUE_INVALID)),
        None => {
            let mut list = vec![0.0; segments];
            if let Some(last) = list.last_mut() {
                *last = p.bulge.unwrap_or(0.0);
            }
            list
        }
    };
    if bulges
        .iter()
        .any(|b| !b.is_finite() || b.abs() > archbuilder_model::floor::MAX_BULGE)
    {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    let (start, end, bulge) = match (points.first(), points.last(), bulges.last()) {
        (Some(a), Some(b), Some(k)) => (*a, *b, *k),
        _ => return Err(Error::new(codes::NO_LINE)),
    };
    let via: Vec<([f64; 2], f64)> = points
        .iter()
        .skip(1)
        .take(segments - 1)
        .zip(&bulges)
        .map(|(q, k)| (*q, *k))
        .collect();
    let bottom = optional_level(&snapshot, building, p.bottom_level.as_deref())?;
    let top = optional_level(&snapshot, building, p.top_level.as_deref())?;
    let label = match p.label.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        Some(label) => label.to_owned(),
        None => {
            let used: Vec<String> = datum::grids(&snapshot, building)
                .iter()
                .filter_map(|g| types::text(&g.values, types::grid::NAME).map(str::to_owned))
                .collect();
            let used: Vec<&str> = used.iter().map(String::as_str).collect();
            datum::next_label(&used, p.after.as_deref())
        }
    };
    let reference = |id: Option<ObjectId>| id.map(Value::Reference);
    let id = create(
        ctx,
        GRID,
        Some(building),
        vec![
            Some(named(&label)),
            Some(types::length(start[0])),
            Some(types::length(start[1])),
            Some(types::length(end[0])),
            Some(types::length(end[1])),
            Some(Value::F64(bulge)),
            Some(Value::Bool(true)),
            Some(Value::Bool(true)),
            reference(bottom),
            reference(top),
            None,
            datum::path_value(&via),
        ],
    )?;
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    Ok(GridCreated {
        grid: IdParam(id),
        label,
    })
}

/// The first free `Reference plane <n>` or `Work plane <n>` of a building.
fn next_name(snapshot: &Snapshot, key: &str, building: ObjectId, tongue: &str) -> String {
    let used = spatial::children(snapshot, key, building);
    let taken = |name: &str| {
        used.iter()
            .any(|o| types::text(&o.values, 0).is_some_and(|n| names::same(n, name)))
    };
    (used.len() + 1..)
        .map(|n| {
            if key == REFERENCE_PLANE {
                defaults::reference_plane(tongue, n)
            } else {
                defaults::work_plane(tongue, n)
            }
        })
        .find(|name| !taken(name))
        .unwrap_or_default()
}

fn create_reference_plane(
    ctx: &mut Context<'_>,
    p: CreateReferencePlane,
) -> Result<ReferencePlaneCreated> {
    let building = building_for(ctx, p.building)?;
    if !point_ok(p.start) || !point_ok(p.end) {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    let strength = match p.strength.as_deref().map(str::trim) {
        None | Some("") => Strength::Weak,
        Some(t) => Strength::ALL
            .into_iter()
            .find(|s| s.name() == t)
            .ok_or_else(|| Error::new(codes::VALUE_INVALID))?,
    };
    let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_owned(),
        None => next_name(ctx.snapshot(), REFERENCE_PLANE, building, &language(ctx)),
    };
    let id = create(
        ctx,
        REFERENCE_PLANE,
        Some(building),
        vec![
            Some(named(&name)),
            Some(types::length(p.start[0])),
            Some(types::length(p.start[1])),
            Some(types::length(p.end[0])),
            Some(types::length(p.end[1])),
            Some(Value::Enum(strength.code())),
            Some(Value::Bool(p.bounded.unwrap_or(true))),
            None,
        ],
    )?;
    ensure_valid(ctx)?;
    ctx.set_selection_after(vec![id]);
    Ok(ReferencePlaneCreated {
        reference_plane: IdParam(id),
        name,
    })
}

/// The source and target a work plane command means: from `source` and `target`, else from the
/// first selected level, grid or reference plane.
fn resolve_target(
    ctx: &Context<'_>,
    building: ObjectId,
    p: &SetWorkPlane,
) -> Result<(Source, Option<ObjectId>)> {
    let snapshot = ctx.snapshot();
    if p.points.is_some() || p.source.as_deref() == Some("points") {
        return Ok((Source::Points, None));
    }
    let source_of = |key: &str| match key {
        LEVEL => Some(Source::Level),
        GRID => Some(Source::Grid),
        REFERENCE_PLANE => Some(Source::ReferencePlane),
        _ => None,
    };
    let found =
        match p.target.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            Some(text) => {
                let datums = [GRID, REFERENCE_PLANE]
                    .into_iter()
                    .flat_map(|k| spatial::children(snapshot, k, building));
                let by_name = spatial::levels(snapshot, building)
                    .into_iter()
                    .chain(datums)
                    .find(|o| {
                        o.id.to_string() == text
                            || types::text(&o.values, 0).is_some_and(|n| names::same(n, text))
                    });
                by_name.map(|o| o.id)
            }
            None => ctx.selection().iter().copied().find(|id| {
                spatial::type_of(snapshot, *id).is_some_and(|k| source_of(&k).is_some())
            }),
        };
    let id = found.ok_or_else(|| Error::new(codes::NO_WORK_PLANE))?;
    let source = spatial::type_of(snapshot, id)
        .and_then(|k| source_of(&k))
        .ok_or_else(|| Error::new(codes::NO_WORK_PLANE))?;
    if let Some(named) = p.source.as_deref()
        && named != source.name()
    {
        return Err(Error::new(codes::KIND_MISMATCH));
    }
    Ok((source, Some(id)))
}

fn set_work_plane(ctx: &mut Context<'_>, p: SetWorkPlane) -> Result<WorkPlaneSet> {
    let building = building_for(ctx, p.building)?;
    let (source, target) = resolve_target(ctx, building, &p)?;
    let snapshot = ctx.snapshot().clone();
    let shown = p.shown.unwrap_or(true);
    // A plane that follows a datum is reused; three points always make a new one.
    let existing = (source != Source::Points)
        .then(|| {
            spatial::children(&snapshot, WORK_PLANE, building)
                .into_iter()
                .find(|o| {
                    Source::of_values(&o.values) == source
                        && matches!(
                            o.values.get(types::work_plane::TARGET),
                            Some(Some(Value::Reference(t))) if Some(*t) == target
                        )
                })
        })
        .flatten();
    // The others stop being active.
    for other in spatial::children(&snapshot, WORK_PLANE, building) {
        if existing.as_ref().is_none_or(|e| e.id != other.id) {
            ctx.txn()?
                .modify(other.id, FieldPath::new("active"), Some(Value::Bool(false)))?;
        }
    }
    let (id, name) = if let Some(found) = existing {
        let txn = ctx.txn()?;
        txn.modify(found.id, FieldPath::new("active"), Some(Value::Bool(true)))?;
        txn.modify(found.id, FieldPath::new("shown"), Some(Value::Bool(shown)))?;
        (
            found.id,
            types::text(&found.values, 0).unwrap_or_default().to_owned(),
        )
    } else {
        let name = match p.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => n.to_owned(),
            None => next_name(&snapshot, WORK_PLANE, building, &language(ctx)),
        };
        let mut values: Vec<Option<Value>> = vec![
            Some(named(&name)),
            Some(Value::Enum(source.code())),
            target.map(Value::Reference),
        ];
        let points = p.points.unwrap_or([[0.0; 3]; 3]);
        for point in points {
            for v in point {
                if !point_ok([v, 0.0]) {
                    return Err(Error::new(codes::VALUE_INVALID));
                }
                values.push((source == Source::Points).then(|| types::length(v)));
            }
        }
        values.push(Some(Value::Bool(true)));
        values.push(Some(Value::Bool(shown)));
        (create(ctx, WORK_PLANE, Some(building), values)?, name)
    };
    ensure_valid(ctx)?;
    Ok(WorkPlaneSet {
        work_plane: IdParam(id),
        name,
    })
}

fn show_work_plane(ctx: &mut Context<'_>, p: ShowWorkPlane) -> Result<WorkPlaneSet> {
    let building = building_for(ctx, p.building)?;
    let (object, _) =
        datum::active(ctx.snapshot(), building).ok_or_else(|| Error::new(codes::NO_WORK_PLANE))?;
    let now = matches!(
        object.values.get(types::work_plane::SHOWN),
        Some(Some(Value::Bool(true)))
    );
    let shown = p.shown.unwrap_or(!now);
    ctx.txn()?
        .modify(object.id, FieldPath::new("shown"), Some(Value::Bool(shown)))?;
    ensure_valid(ctx)?;
    Ok(WorkPlaneSet {
        work_plane: IdParam(object.id),
        name: types::text(&object.values, 0)
            .unwrap_or_default()
            .to_owned(),
    })
}

fn set_extent(ctx: &mut Context<'_>, p: SetExtent) -> Result<ExtentSet> {
    let ids: Vec<ObjectId> = match p.objects {
        Some(list) if list.len() > MAX_OBJECTS => {
            return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
        }
        Some(list) => list.into_iter().map(|IdParam(id)| id).collect(),
        None => ctx.selection().iter().copied().take(MAX_OBJECTS).collect(),
    };
    let snapshot = ctx.snapshot().clone();
    let grids: Vec<_> = ids
        .iter()
        .filter_map(|id| spatial::get(&snapshot, *id))
        .filter(|o| spatial::type_of(&snapshot, o.id).as_deref() == Some(GRID))
        .collect();
    if grids.is_empty() {
        return Err(Error::new(codes::NOTHING_SELECTED));
    }
    for g in &grids {
        let building = g
            .header
            .owner
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let mut changes: Vec<(&str, Option<Value>)> = Vec::new();
        for (field, text) in [
            ("bottom_level", &p.bottom_level),
            ("top_level", &p.top_level),
        ] {
            if text.is_some() {
                let level = optional_level(&snapshot, building, text.as_deref())?;
                changes.push((field, level.map(Value::Reference)));
            }
        }
        for (field, flag) in [("head_start", p.head_start), ("head_end", p.head_end)] {
            if let Some(on) = flag {
                changes.push((field, Some(Value::Bool(on))));
            }
        }
        let txn = ctx.txn()?;
        for (field, value) in changes {
            txn.modify(g.id, FieldPath::new(field), value)?;
        }
    }
    ensure_valid(ctx)?;
    Ok(ExtentSet {
        objects: grids.len(),
    })
}

/// A message of the catalog, named by identifier.
struct Msg {
    message_id: &'static str,
}

fn two_points(first: Msg, second: Msg) -> PromptSequence {
    let point = || Accept::Point {
        dims: PointDims::Two,
    };
    PromptSequence::new()
        .step(PromptStep::new("start", first.message_id, point()))
        .step(PromptStep::new("end", second.message_id, point()))
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    let spec = |id, label: Msg, description: Msg| (id, label.message_id, description.message_id);
    let (id, label, description) = spec(
        GRID_CREATE,
        Msg {
            message_id: "ab-cmd-grid-create-label",
        },
        Msg {
            message_id: "ab-cmd-grid-create-description",
        },
    );
    registry.register(
        CommandSpec::new(
            id,
            label,
            description,
            |ctx: &mut Context<'_>, p: CreateGrid| create_grid(ctx, p),
        )
        .with_prompts(two_points(
            Msg {
                message_id: "ab-prompt-grid-start",
            },
            Msg {
                message_id: "ab-prompt-grid-end",
            },
        )),
    )?;
    let (id, label, description) = spec(
        REFERENCE_PLANE_CREATE,
        Msg {
            message_id: "ab-cmd-reference-plane-create-label",
        },
        Msg {
            message_id: "ab-cmd-reference-plane-create-description",
        },
    );
    registry.register(
        CommandSpec::new(
            id,
            label,
            description,
            |ctx: &mut Context<'_>, p: CreateReferencePlane| create_reference_plane(ctx, p),
        )
        .with_prompts(two_points(
            Msg {
                message_id: "ab-prompt-reference-plane-start",
            },
            Msg {
                message_id: "ab-prompt-reference-plane-end",
            },
        )),
    )?;
    let (id, label, description) = spec(
        WORK_PLANE_SET,
        Msg {
            message_id: "ab-cmd-work-plane-set-label",
        },
        Msg {
            message_id: "ab-cmd-work-plane-set-description",
        },
    );
    registry.register(CommandSpec::new(
        id,
        label,
        description,
        |ctx: &mut Context<'_>, p: SetWorkPlane| set_work_plane(ctx, p),
    ))?;
    let (id, label, description) = spec(
        WORK_PLANE_SHOW,
        Msg {
            message_id: "ab-cmd-work-plane-show-label",
        },
        Msg {
            message_id: "ab-cmd-work-plane-show-description",
        },
    );
    registry.register(CommandSpec::new(
        id,
        label,
        description,
        |ctx: &mut Context<'_>, p: ShowWorkPlane| show_work_plane(ctx, p),
    ))?;
    let (id, label, description) = spec(
        DATUM_SET_EXTENT,
        Msg {
            message_id: "ab-cmd-datum-set-extent-label",
        },
        Msg {
            message_id: "ab-cmd-datum-set-extent-description",
        },
    );
    registry.register(CommandSpec::new(
        id,
        label,
        description,
        |ctx: &mut Context<'_>, p: SetExtent| set_extent(ctx, p),
    ))
}

/// Moves the grids and reference planes `ids` by `xf` and returns how many.
pub(crate) fn move_datums(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ids: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
) -> Result<()> {
    for id in ids {
        let (key, start, end, bulge) = line_of(snapshot, *id)?;
        if key == GRID
            && let Some(g) =
                spatial::get(snapshot, *id).and_then(|o| datum::Grid::from_values(&o.values))
            && !g.via.is_empty()
        {
            let sign = if xf.reflects() { -1.0 } else { 1.0 };
            let via: Vec<([f64; 2], f64)> = g
                .via
                .iter()
                .map(|(q, k)| (xf.point(*q), sign * k))
                .collect();
            ctx.txn()?
                .modify(*id, FieldPath::new("path"), datum::path_value(&via))?;
        }
        let (a, b) = (xf.point(start), xf.point(end));
        for (field, v) in [
            ("start_x", a[0]),
            ("start_y", a[1]),
            ("end_x", b[0]),
            ("end_y", b[1]),
        ] {
            crate::modify::set_length(ctx, *id, field, v)?;
        }
        if xf.reflects() && bulge != 0.0 {
            ctx.txn()?
                .modify(*id, FieldPath::new("bulge"), Some(Value::F64(-bulge)))?;
        }
    }
    Ok(())
}

/// The kind, ends and bulge of the grid or reference plane `id`.
fn line_of(snapshot: &Snapshot, id: ObjectId) -> Result<(String, [f64; 2], [f64; 2], f64)> {
    let object = spatial::get(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    let key = spatial::type_of(snapshot, id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
    match key.as_str() {
        GRID => datum::Grid::from_values(&object.values).map(|g| (key, g.start, g.end, g.bulge)),
        REFERENCE_PLANE => {
            datum::RefPlane::from_values(&object.values).map(|r| (key, r.start, r.end, 0.0))
        }
        _ => None,
    }
    .ok_or_else(|| Error::new(codes::KIND_MISMATCH))
}

/// Copies the grids and reference planes `ids` moved by `xf`: a grid gets the label after its
/// original's, a reference plane the next free name.
pub(crate) fn copy_datums(
    ctx: &mut Context<'_>,
    snapshot: &Snapshot,
    ids: &[ObjectId],
    xf: &archbuilder_model::transform::Xform,
) -> Result<Vec<ObjectId>> {
    let mut out = Vec::new();
    for id in ids {
        let (key, ..) = line_of(snapshot, *id)?;
        let object = spatial::get(snapshot, *id).ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        let owner = object
            .header
            .owner
            .ok_or_else(|| Error::new(codes::KIND_MISMATCH))?;
        out.push(copy_datum(ctx, &key, object.values.clone(), owner, xf)?);
    }
    Ok(out)
}

/// Creates in `building` a copy of the grid or reference plane (`key`) with the field values
/// `values`, moved by `xf`: a grid gets the label after the original's, a reference plane the next
/// free name. Levels the grid reaches that are not levels of `building` are dropped (the grid then
/// reaches the building's lowest or highest level), as a pasted copy from another document has.
pub(crate) fn copy_datum(
    ctx: &mut Context<'_>,
    key: &str,
    mut values: Vec<Option<Value>>,
    building: ObjectId,
    xf: &archbuilder_model::transform::Xform,
) -> Result<ObjectId> {
    let invalid = || Error::new(codes::KIND_MISMATCH);
    let (start, end, bulge) = match key {
        GRID => datum::Grid::from_values(&values).map(|g| (g.start, g.end, g.bulge)),
        REFERENCE_PLANE => datum::RefPlane::from_values(&values).map(|r| (r.start, r.end, 0.0)),
        _ => None,
    }
    .ok_or_else(invalid)?;
    let tongue = language(ctx);
    let seen = ctx.txn()?.snapshot().clone();
    let (a, b) = (xf.point(start), xf.point(end));
    let (x0, y0, x1, y1) = (
        types::grid::START_X,
        types::grid::START_Y,
        types::grid::END_X,
        types::grid::END_Y,
    );
    for (at, v) in [(x0, a[0]), (y0, a[1]), (x1, b[0]), (y1, b[1])] {
        if let Some(slot) = values.get_mut(at) {
            *slot = Some(types::length(v));
        }
    }
    let name = if key == GRID {
        if let Some(slot) = values.get_mut(types::grid::BULGE) {
            *slot = Some(Value::F64(if xf.reflects() { -bulge } else { bulge }));
        }
        if let Some(g) = datum::Grid::from_values(&values) {
            let sign = if xf.reflects() { -1.0 } else { 1.0 };
            let via: Vec<([f64; 2], f64)> = g
                .via
                .iter()
                .map(|(q, k)| (xf.point(*q), sign * k))
                .collect();
            if let Some(slot) = values.get_mut(types::grid::PATH) {
                *slot = datum::path_value(&via);
            }
        }
        let ours = spatial::children(&seen, LEVEL, building);
        for at in [types::grid::BOTTOM_LEVEL, types::grid::TOP_LEVEL] {
            if let Some(slot) = values.get_mut(at)
                && let Some(Value::Reference(level)) = slot
                && !ours.iter().any(|l| l.id == *level)
            {
                *slot = None;
            }
        }
        let used: Vec<String> = datum::grids(&seen, building)
            .iter()
            .filter_map(|g| types::text(&g.values, 0).map(str::to_owned))
            .collect();
        let used: Vec<&str> = used.iter().map(String::as_str).collect();
        let label = types::text(&values, 0).unwrap_or_default().to_owned();
        datum::next_label(&used, Some(&label))
    } else {
        next_name(&seen, REFERENCE_PLANE, building, &tongue)
    };
    if let Some(slot) = values.get_mut(0) {
        *slot = Some(Value::String(name));
    }
    create(ctx, key, Some(building), values)
}
