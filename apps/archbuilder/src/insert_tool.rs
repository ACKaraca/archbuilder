//! The insert tool: the prompts of `archbuilder.door.place` and `archbuilder.window.place` behind the
//! viewport and the command line, without any windowing, and the pointer's way to a wall: which
//! wall the pointer is on and where along it the opening would sit ([`hover`]).
//!
//! The tool places inserts in a row: each placement is one command (one undo step), and when one is
//! placed the next starts with the same wall, width, height, sill, hand and facing. The chain ends on
//! Enter or Escape; the inserts already placed stay. A click answers the prompt with the wall under
//! the pointer and the distance along it; a typed distance goes to the wall under the pointer, else
//! to the wall of the selection.

use archbuilder_model::element_type::TypeKind;
use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::PromptInput;
use arc_command::{CommandLine, Invocation, LineStep, Outcome, Submitted, Target};
use arc_doc::Snapshot;
use archbuilder_commands::insert::{DOOR_PLACE, WINDOW_PLACE};
use archbuilder_model::insert::{self, Kind, SIDE_CLEARANCE};
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, WALL, WINDOW};
use archbuilder_model::wall::Wall;
use serde_json::{Map, Value};

/// The step the center of an opening snaps to along a wall under the pointer, in meters.
pub const STEP: f64 = 0.05;
/// How far from a wall the pointer still points at it, in pixels.
const APERTURE: f64 = 14.0;

/// What the tool reads from the outside; the same pieces as the wall tool's.
pub use crate::wall_tool::Env;
/// Runs an invocation of a document command.
pub type Runner<'r> = &'r mut dyn FnMut(Invocation) -> Result<Outcome>;

/// The command a kind places.
#[must_use]
pub const fn command(kind: Kind) -> &'static str {
    match kind {
        Kind::Door => DOOR_PLACE,
        Kind::Window => WINDOW_PLACE,
        Kind::Opening => archbuilder_commands::insert::OPENING_PLACE,
    }
}

/// The options and position of the chain being placed.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// Door or window.
    pub kind: Kind,
    /// The storey whose walls the pointer picks from.
    pub storey: ObjectId,
    /// The elevation of the storey's level, where clicks land.
    pub elevation: f64,
    /// The wall the prompt was started with: the selected one, then the last one placed in.
    pub wall: Option<ObjectId>,
    /// The width of the next insert, in meters, once one carried it.
    pub width: Option<f64>,
    /// The height of the next insert, once one carried it.
    pub height: Option<f64>,
    /// The sill height of the next window, once one carried it.
    pub sill: Option<f64>,
    /// The arch rise of the next wall opening, once one carried it.
    pub rise: Option<f64>,
    /// Whether the hand of the next insert is flipped.
    pub flip_hand: bool,
    /// Whether the facing of the next insert is flipped.
    pub flip_facing: bool,
    /// The inserts placed so far.
    pub placed: usize,
    /// The type of the next insert: the type picked in the type selector, or the type the last
    /// insert got.
    pub insert_type: Option<String>,
    /// The type picked in the type selector when the chain last looked; a new pick replaces
    /// `insert_type` and the size and sill carried so far.
    pub picked: Option<String>,
    /// The family type of a hosted family instance: the chain places instances of it
    /// (`archbuilder.family_insert.place`) instead of doors and windows.
    pub family: Option<String>,
}

/// The state of the insert tool of one tab.
#[derive(Debug, Default)]
pub struct InsertTool {
    line: CommandLine,
    state: Option<State>,
    error: Option<Error>,
    /// The hand and facing flips the person chose, while a click places with those the pointer
    /// gave; they carry on to the next insert.
    pending: Option<(bool, bool)>,
}

fn target<'a>(env: &'a Env<'_>, localized: &'a BTreeMap<String, String>) -> Target<'a> {
    Target {
        registry: env.registry,
        document: env.document,
        session: env.session,
        selection: env.selection,
        localized,
        seed: env.seed,
    }
}

/// The storey and the wall the selection names: a selected wall, the wall of a selected insert,
/// else a selected storey or the lowest one.
fn context(
    snapshot: &Snapshot,
    selection: &[ObjectId],
) -> Option<(ObjectId, f64, Option<ObjectId>)> {
    let wall = selection.iter().find_map(|id| {
        let object = spatial::get(snapshot, *id)?;
        match spatial::type_of(snapshot, *id)?.as_str() {
            WALL => Some(object),
            DOOR | WINDOW => spatial::get(snapshot, object.header.owner?),
            _ => None,
        }
    });
    if let Some(wall) = wall {
        let storey = spatial::get(snapshot, wall.header.owner?)?;
        let elevation = spatial::storey_elevation(snapshot, &storey);
        return Some((storey.id, elevation, Some(wall.id)));
    }
    let (storey, elevation) = crate::wall_tool::storey_for(snapshot, selection)?;
    Some((storey, elevation, None))
}

impl InsertTool {
    /// An idle tool.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The command line, for the prompt and the options it shows.
    #[must_use]
    pub fn line(&self) -> &CommandLine {
        &self.line
    }

    /// The chain being placed.
    #[must_use]
    pub fn state(&self) -> Option<&State> {
        self.state.as_ref()
    }

    /// Whether a chain is running.
    #[must_use]
    pub fn running(&self) -> bool {
        self.state.is_some()
    }

    /// The error a rejected answer or a refused placement left, taken once.
    pub fn take_error(&mut self) -> Option<Error> {
        self.error.take()
    }

    /// Starts a chain of `kind`: the prompt of the first insert, on the wall of the selection if
    /// it names one.
    ///
    /// # Errors
    /// `ab.cmd.no_building` without a storey; the errors of the command line.
    pub fn begin(&mut self, env: &Env<'_>, run: Runner<'_>, kind: Kind) -> Result<()> {
        self.start_chain(env, run, kind, None)
    }

    /// Starts the chain that places hosted instances of the family type `family` (an identifier
    /// or a name) in the walls the pointer picks.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn begin_family(&mut self, env: &Env<'_>, run: Runner<'_>, family: String) -> Result<()> {
        self.start_chain(env, run, Kind::Door, Some(family))
    }

    fn start_chain(
        &mut self,
        env: &Env<'_>,
        run: Runner<'_>,
        kind: Kind,
        family: Option<String>,
    ) -> Result<()> {
        self.line.cancel();
        self.state = None;
        let (storey, elevation, wall) = context(&env.document.snapshot(), env.selection)
            .ok_or_else(|| Error::new(archbuilder_commands::error::codes::NO_BUILDING))?;
        self.state = Some(State {
            kind,
            storey,
            elevation,
            wall,
            width: None,
            height: None,
            sill: None,
            rise: None,
            flip_hand: false,
            flip_facing: false,
            placed: 0,
            insert_type: family.clone(),
            picked: None,
            family,
        });
        self.next(env, run)
    }

    /// Starts the prompt of the next insert with the options the chain carries.
    fn start(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<Submitted> {
        let snapshot = env.document.snapshot();
        let Some(state) = &mut self.state else {
            return Ok(Submitted::Idle);
        };
        let kind = match state.kind {
            Kind::Door | Kind::Opening => TypeKind::Door,
            Kind::Window => TypeKind::Window,
        };
        // A bare opening has no type: the selector's pick is not its own.
        let picked = (state.kind != Kind::Opening && state.family.is_none())
            .then(|| crate::tool_type::current_param(&snapshot, kind))
            .flatten();
        if picked != state.picked || (state.insert_type.is_none() && state.kind != Kind::Opening) {
            state.insert_type.clone_from(&picked);
            state.picked = picked;
            (state.width, state.height, state.sill, state.rise) = (None, None, None, None);
        }
        let state = &*state;
        let mut preset = Map::new();
        if let Some(t) = &state.insert_type {
            preset.insert("type".into(), Value::from(t.as_str()));
        }
        if let Some(wall) = state.wall {
            // A hosted family instance names its host as `host`.
            let key = if state.family.is_some() { "host" } else { "wall" };
            preset.insert(key.into(), Value::from(wall.to_string()));
        }
        for (key, value) in [
            ("width", state.width),
            ("height", state.height),
            ("sill", state.sill),
            ("rise", state.rise),
        ] {
            if let Some(value) = value {
                preset.insert(key.into(), Value::from(value));
            }
        }
        for (key, flag) in [
            ("flip_hand", state.flip_hand),
            ("flip_facing", state.flip_facing),
        ] {
            if flag {
                preset.insert(key.into(), Value::from(true));
            }
        }
        let localized = BTreeMap::new();
        let step = LineStep::Start {
            command: match state.family {
                Some(_) => archbuilder_commands::family_project::FAMILY_INSERT_PLACE,
                None => command(state.kind),
            },
            preset,
        };
        self.line.drive(&target(env, &localized), step, run)
    }

    fn next(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        let result = self.start(env, run);
        self.follow(env, run, result)
    }

    /// Takes what a line step led to: an insert that was placed continues the chain, a rejected
    /// answer or a refused placement is kept as the pending error and the prompt starts again.
    fn follow(&mut self, env: &Env<'_>, run: Runner<'_>, result: Result<Submitted>) -> Result<()> {
        match result {
            Err(error) => {
                // A refused placement leaves the chain going at a new prompt; a start that
                // fails ends it.
                self.error = Some(error);
                self.line.cancel();
                if self.start(env, run).is_err() {
                    self.state = None;
                }
                Ok(())
            }
            Ok(Submitted::Rejected { error, .. }) => {
                self.error = Some(error);
                Ok(())
            }
            Ok(Submitted::Ran(outcome))
                if outcome.command == DOOR_PLACE
                    || outcome.command == WINDOW_PLACE
                    || outcome.command == archbuilder_commands::insert::OPENING_PLACE
                    || outcome.command == archbuilder_commands::family_project::FAMILY_INSERT_PLACE =>
            {
                self.placed(&outcome.value);
                self.next(env, run)
            }
            Ok(Submitted::Cancelled | Submitted::Idle) => {
                self.state = None;
                Ok(())
            }
            Ok(_) => Ok(()),
        }
    }

    /// An insert was placed: the chain carries its options and its wall.
    fn placed(&mut self, value: &Value) {
        let Some(state) = &mut self.state else {
            return;
        };
        let number = |key: &str| value.get(key).and_then(Value::as_f64);
        let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(false);
        state.wall = value
            .get("wall")
            .and_then(Value::as_str)
            .and_then(|w| w.parse().ok())
            .or(state.wall);
        state.width = number("width");
        state.height = number("height");
        state.sill = number("sill");
        state.rise = number("rise");
        if state.family.is_none() {
            state.insert_type = value.get("type").and_then(Value::as_str).map(str::to_owned);
        }
        // A click carries the options the person set; the flags it placed with followed the pointer.
        (state.flip_hand, state.flip_facing) = self
            .pending
            .take()
            .unwrap_or((flag("flip_hand"), flag("flip_facing")));
        state.placed += 1;
    }

    /// A type was picked in the type selector: the prompt starts again with it.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn retype(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        if self.state.is_none() {
            return Ok(());
        }
        self.line.cancel();
        let result = self.start(env, run);
        self.follow(env, run, result)
    }

    /// Keeps the options the active prompt shows (width, height, sill, flips), so a refused
    /// placement or a new prompt starts with them.
    fn remember(&mut self) {
        let Some(prompt) = self.line.active().and_then(|s| s.prompt()) else {
            return;
        };
        let arg = |name: &str| prompt.message.args.get(name).map(String::as_str);
        let number = |name: &str| arg(name).and_then(|v| v.parse::<f64>().ok());
        if let Some(state) = &mut self.state {
            state.width = number("width_m").or(state.width);
            state.height = number("height_m").or(state.height);
            state.sill = number("sill_m").or(state.sill);
            state.rise = number("rise_m").or(state.rise);
            if arg("hand").is_some() {
                state.flip_hand = arg("hand") == Some("right");
                state.flip_facing = arg("facing") == Some("interior");
            }
        }
    }

    /// Whether the active prompt asks for a placement (not for the value of an option).
    fn at_placement(&self) -> bool {
        self.line
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| p.key.starts_with("point"))
    }

    /// A click on `wall` at `distance` meters from its baseline start, with the pointer on its
    /// interior side or not. The pointer decides the facing: the leaf swings to the side the
    /// pointer is on. The hinge stays on the side it was (toward the wall's end by default, `flip hand`
    /// changes it), and the hand follows from the hinge and the facing. `flip facing` inverts what
    /// the pointer gives.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn click(
        &mut self,
        env: &Env<'_>,
        run: Runner<'_>,
        wall: ObjectId,
        distance: f64,
        interior: bool,
    ) -> Result<()> {
        if !self.at_placement() {
            return Ok(());
        }
        let Some(prompt) = self.line.active().and_then(|s| s.prompt()).cloned() else {
            return Ok(());
        };
        let arg = |name: &str| prompt.message.args.get(name).map(String::as_str);
        let number = |name: &str| arg(name).and_then(|v| v.parse::<f64>().ok());
        let (invert_hand, invert_facing) = (
            arg("hand") == Some("right"),
            arg("facing") == Some("interior"),
        );
        let flip_facing = interior != invert_facing;
        // Seen from the facing side, a hinge toward the end is on the left from the exterior.
        let (hinge, side) = (
            if invert_hand { -1.0 } else { 1.0 },
            if flip_facing { -1.0 } else { 1.0 },
        );
        if let Some(state) = &mut self.state {
            state.wall = Some(wall);
            state.width = number("width_m").or(state.width);
            state.height = number("height_m").or(state.height);
            state.sill = number("sill_m").or(state.sill);
            state.rise = number("rise_m").or(state.rise);
            state.flip_hand = hinge * side < 0.0;
            state.flip_facing = flip_facing;
        }
        self.pending = Some((invert_hand, invert_facing));
        self.line.cancel();
        let started = self.start(env, run);
        self.follow(env, run, started)?;
        let localized = BTreeMap::new();
        let result = self.line.drive(
            &target(env, &localized),
            LineStep::Answer(PromptInput::Distance(distance)),
            run,
        );
        self.follow(env, run, result)?;
        // A refused placement leaves the person's own choices as they were.
        if let (Some(state), Some((hand, facing))) = (&mut self.state, self.pending.take()) {
            (state.flip_hand, state.flip_facing) = (hand, facing);
        }
        Ok(())
    }

    /// A line the user typed: a distance along the wall under the pointer (`under`), else along
    /// the wall of the selection, or a keyword. Empty text (Enter) ends the chain.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn text(
        &mut self,
        env: &Env<'_>,
        run: Runner<'_>,
        text: &str,
        under: Option<ObjectId>,
    ) -> Result<()> {
        let text = text.trim();
        if text.is_empty() {
            self.stop();
            return Ok(());
        }
        let localized = BTreeMap::new();
        // A distance goes to the wall under the pointer when that is not the preset one.
        let numeric = text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+'));
        self.remember();
        if let (true, Some(wall), true) = (numeric, under, self.at_placement())
            && self.state.as_ref().and_then(|s| s.wall) != Some(wall)
        {
            let result = self.line.drive(
                &target(env, &localized),
                LineStep::Answer(PromptInput::Pick(wall)),
                run,
            );
            self.follow(env, run, result)?;
        }
        let result = self
            .line
            .drive(&target(env, &localized), LineStep::Submit(text), run);
        self.follow(env, run, result)
    }

    /// Steps back one answer of the active command.
    ///
    /// # Errors
    /// The errors of the command line.
    pub fn back(&mut self, env: &Env<'_>, run: Runner<'_>) -> Result<()> {
        if self.line.active().is_none() {
            return Ok(());
        }
        let localized = BTreeMap::new();
        let result = self
            .line
            .drive(&target(env, &localized), LineStep::Back, run);
        self.follow(env, run, result)
    }

    /// Enter or Escape: drops the prompt and ends the chain. The inserts already placed stay.
    pub fn stop(&mut self) {
        self.line.cancel();
        self.state = None;
    }
}

/// Where the pointer is on a wall and where the opening would sit.
#[derive(Debug, Clone, PartialEq)]
pub struct Hover {
    /// The wall under the pointer.
    pub wall: ObjectId,
    /// The distance of the opening's center from the baseline's start, snapped to [`STEP`] and
    /// kept inside the clear span.
    pub distance: f64,
    /// The length of the baseline.
    pub length: f64,
    /// The corners of the wall's joined footprint, in plan.
    pub footprint: [[f64; 2]; 4],
    /// The elevation of the wall's top, where the highlight is drawn.
    pub top: f64,
    /// The elevation of the wall's base.
    pub base: f64,
    /// The corners of the opening in plan, across the whole thickness, in the order of a loop.
    pub opening: [[f64; 2]; 4],
    /// Whether the pointer is on the interior side of the wall (the right of the baseline).
    pub interior: bool,
    /// The wall's baseline start and the unit direction along it.
    pub axes: ([f64; 2], [f64; 2]),
}

/// A ray in model coordinates: its origin and its direction.
pub type Ray = ([f64; 3], [f64; 3]);

use archbuilder_model::insert::enters;

/// The wall of `storey` that the pointer is on, and the opening of width `width` that follows it
/// along the wall. The pointer's `ray` (a view ray through the pixel) picks the wall it enters
/// first, so a wall is picked on its face or its top as the view shows it; without a hit, the
/// model point `point` on the storey's plane picks a wall it is on or within `pixels /
/// pixels_per_unit` of.
#[must_use]
pub fn hover(
    snapshot: &Snapshot,
    storey: ObjectId,
    point: [f64; 2],
    ray: Option<Ray>,
    pixels_per_unit: Option<f64>,
    width: f64,
) -> Option<Hover> {
    let aperture = APERTURE / pixels_per_unit.filter(|p| *p > 0.0).unwrap_or(100.0);
    let elevation =
        spatial::get(snapshot, storey).map_or(0.0, |s| spatial::storey_elevation(snapshot, &s));
    let mut best: Option<(f64, Hover)> = None;
    let mut by_ray = false;
    for object in spatial::walls(snapshot, storey) {
        let Some(wall) = Wall::of(snapshot, &object) else {
            continue;
        };
        let length = wall.length();
        if length < archbuilder_model::types::MIN_WALL_LENGTH {
            continue;
        }
        let u = [
            (wall.end[0] - wall.start[0]) / length,
            (wall.end[1] - wall.start[1]) / length,
        ];
        let n = [-u[1], u[0]];
        let rel = [point[0] - wall.start[0], point[1] - wall.start[1]];
        let (x, y) = (rel[0] * u[0] + rel[1] * u[1], rel[0] * n[0] + rel[1] * n[1]);
        let (low, high) = wall.extent();
        let level = elevation + wall.base_offset;
        // The ray in the wall's own axes: along the baseline, across it and up from its base.
        let hit = ray.and_then(|(o, d)| {
            let local = |v: [f64; 3], origin: bool| {
                let r = if origin {
                    [v[0] - wall.start[0], v[1] - wall.start[1], v[2] - level]
                } else {
                    v
                };
                [r[0] * u[0] + r[1] * u[1], r[0] * n[0] + r[1] * n[1], r[2]]
            };
            let (lo, ld) = (local(o, true), local(d, false));
            let t = enters(lo, ld, [0.0, low, 0.0], [length, high, wall.height])?;
            Some((t, lo[0] + ld[0] * t, lo[1] + ld[1] * t))
        });
        let (score, x, side) = match hit {
            Some((t, hx, hy)) => (t, hx.clamp(0.0, length), hy),
            None if by_ray => continue,
            None => {
                let across = (y - (low + high) / 2.0).abs() - wall.thickness / 2.0;
                if across > aperture || x < -aperture || x > length + aperture {
                    continue;
                }
                (f64::MAX / 2.0 + across, x, y)
            }
        };
        if hit.is_some() && !by_ray {
            // A ray hit beats every plan guess.
            by_ray = true;
            best = None;
        }
        let footprint = archbuilder_model::join::footprint_of(snapshot, &object)
            .unwrap_or_else(|| wall.plain_footprint());
        let (lo, hi) = insert::clear_span(&wall, footprint);
        let half = width / 2.0;
        let (min, max) = (lo + SIDE_CLEARANCE + half, hi - SIDE_CLEARANCE - half);
        let snapped = (x / STEP).round() * STEP;
        let distance = if min <= max {
            snapped.clamp(min, max)
        } else {
            snapped
        };
        let at = |d: f64, side: f64| {
            [
                wall.start[0] + u[0] * d + n[0] * side,
                wall.start[1] + u[1] * d + n[1] * side,
            ]
        };
        let found = Hover {
            wall: object.id,
            distance,
            length,
            footprint,
            top: elevation + wall.base_offset + wall.height,
            base: level,
            opening: [
                at(distance - half, low),
                at(distance + half, low),
                at(distance + half, high),
                at(distance - half, high),
            ],
            axes: (wall.start, u),
            interior: side < (low + high) / 2.0,
        };
        if best.as_ref().is_none_or(|(d, _)| score < *d) {
            best = Some((score, found));
        }
    }
    best.map(|(_, h)| h)
}

/// The kind named by a typed word, in either language.
#[must_use]
pub fn kind_named(text: &str) -> Option<Kind> {
    let t = text.trim();
    let is = |words: &[&str]| words.iter().any(|w| t.eq_ignore_ascii_case(w));
    if is(&[DOOR_PLACE, "door", "kapi", "kapı"]) {
        Some(Kind::Door)
    } else if is(&[WINDOW_PLACE, "window", "pencere"]) {
        Some(Kind::Window)
    } else if is(&[
        archbuilder_commands::insert::OPENING_PLACE,
        "opening",
        "boşluk",
        "bosluk",
    ]) {
        Some(Kind::Opening)
    } else {
        None
    }
}
