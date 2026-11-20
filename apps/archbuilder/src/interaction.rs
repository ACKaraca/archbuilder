//! The interaction of one tab: the wall tool behind the viewport's pointer, the command panel and
//! the toolbar. It translates the shell's events for [`crate::wall_tool::Tool`] and its state into
//! what the shell shows (the prompt, the option bar, the rubber band and the cursor).
//!
//! A click on the viewport answers the tool's point prompt at the point the shell resolved on the
//! plane of the storey's level ([`work_plane`]); cursor positions snap to a 10 mm grid. With no
//! prompt active the click is left to the viewport, which selects. The shell carries out its own
//! commands (file, undo, selection) itself; every other command a menu item, a key or a toolbar
//! button starts arrives as an invoked intent, and the wall command starts the tool.
//!
//! Typed numbers follow the interface language: where the prompt cannot take a point, a decimal
//! comma is read as the decimal point in a language that writes one. A point's coordinates are
//! separated by a comma in the foundation's point entry, so a point is typed `3.5,2` in every
//! language.

use std::sync::atomic::{AtomicU64, Ordering};

use arc_app::application::{Application, TabId};
use arc_app::shell::{Interaction, InteractionContext, InteractionOutput};
use arc_base::math::{Point3, Vector3};
use arc_base::{ObjectId, Result};
use arc_command::prompt::Accept;
use arc_command::{Invocation, Registry, Session};
use arc_ui::egui::PointerButton;
use arc_ui::{
    DynamicEvent, Intent, LineAction, LineStatus, OptionControl, PointerEvent, PointerKind,
    ProductOverlay, PromptControls, WorkPlane,
};
use archbuilder_commands::structure::LANGUAGE_SETTING;
use archbuilder_commands::wall::{OBJECT_DELETE, WALL_CREATE};
use archbuilder_model::spatial;
use archbuilder_model::wall::LocationLine;

use crate::wall_tool::{Env, Tool, storey_for};

/// The snap step of the cursor on the level plane, in meters.
const GRID: f64 = 0.01;

/// The plane point input lands on: the level plane of the storey the wall tool draws on (the
/// selection's storey, else the lowest), for [`arc_app::shell::ShellConfig::work_plane`].
#[must_use]
pub fn work_plane(app: &Application, tab: TabId, selection: &[ObjectId]) -> Option<WorkPlane> {
    let snapshot = app.tab(tab).ok()?.document().snapshot();
    let (_, elevation) = storey_for(&snapshot, selection)?;
    Some(WorkPlane {
        origin: Point3::new(0.0, 0.0, elevation),
        normal: Vector3::z(),
    })
}

/// How far from the pointer an existing end point attracts it, in pixels.
const APERTURE_PIXELS: f64 = 10.0;

/// The pointer on the work plane: on the nearest `ends` point within the aperture, else on the
/// grid. The flag says whether it snapped to an end point.
fn snapped(event: &PointerEvent, ends: &[[f64; 2]]) -> Option<([f64; 2], bool)> {
    let point = event.model?;
    if !(point.x.is_finite() && point.y.is_finite()) {
        return None;
    }
    let reach = event.pixels_per_unit.map_or(0.0, |p| APERTURE_PIXELS / p);
    let near = ends
        .iter()
        .map(|e| ((e[0] - point.x).hypot(e[1] - point.y), *e))
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, end)) = near {
        return Some((end, true));
    }
    let snap = |v: f64| (v / GRID).round() * GRID;
    Some(([snap(point.x), snap(point.y)], false))
}

/// How far from a 15 degree step the pointer direction snaps, in degrees.
const ANGLE_WINDOW: f64 = 4.0;

/// `to` moved onto the nearest 15 degree direction from `from` when the pointer is within the
/// window of one (exactly horizontal or vertical for the four right angles), with the length
/// snapped to the grid; the angle in degrees when it snapped.
#[must_use]
pub fn snap_direction(from: [f64; 2], to: [f64; 2]) -> Option<([f64; 2], i32)> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = dx.hypot(dy);
    if length < GRID {
        return None;
    }
    let degrees = dy.atan2(dx).to_degrees();
    let step = (degrees / 15.0).round();
    if (degrees - step * 15.0).abs() > ANGLE_WINDOW {
        return None;
    }
    // `step` is a whole number from -12 to 12, so the conversion is exact.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "a whole number from -12 to 12"
    )]
    let k = (step as i32).rem_euclid(24);
    let (c, s) = match k {
        0 => (1.0, 0.0),
        6 => (0.0, 1.0),
        12 => (-1.0, 0.0),
        18 => (0.0, -1.0),
        _ => {
            let r = (f64::from(k) * 15.0).to_radians();
            (r.cos(), r.sin())
        }
    };
    let length = (length / GRID).round() * GRID;
    Some(([from[0] + c * length, from[1] + s * length], k * 15))
}

/// The end points of the walls of `storey`, which a new wall closes on.
fn end_points(cx: &InteractionContext<'_>, storey: ObjectId) -> Vec<[f64; 2]> {
    let Ok(document) = cx.document() else {
        return Vec::new();
    };
    spatial::walls(&document.snapshot(), storey)
        .iter()
        .filter_map(|w| archbuilder_model::wall::Wall::from_values(&w.values))
        .flat_map(|w| [w.start, w.end])
        .collect()
}

/// Whether `text` asks to delete the selection.
fn is_erase_word(text: &str) -> bool {
    ["erase", "delete", "sil"]
        .iter()
        .any(|w| text.eq_ignore_ascii_case(w))
}

/// The controller of a tab.
pub struct WallInteraction {
    tool: Tool,
    registry: Option<Registry>,
    /// The seed of the next invocation, advanced with each one so a reopened document does not
    /// repeat the identifiers of an earlier run.
    seed: u64,
    /// The pointer on the level plane, for the rubber band and the status bar.
    cursor: Option<[f64; 2]>,
    /// The end point the pointer snapped to, for its marker.
    snap: Option<[f64; 2]>,
    /// The point and the direction in degrees when the pointer snapped to a 15 degree direction.
    angle: Option<([f64; 2], i32)>,
    /// The messages of the interface language, for the keywords typed in it.
    messages: Option<(String, arc_app::locale::Localizer)>,
}

impl WallInteraction {
    /// The interaction of a new tab.
    #[must_use]
    pub fn new(seed: &AtomicU64) -> Self {
        Self {
            tool: Tool::new(),
            registry: archbuilder_commands::registry().ok(),
            seed: seed.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed),
            cursor: None,
            snap: None,
            angle: None,
            messages: None,
        }
    }

    /// The shell's session (it carries the interface language, `app.language`) with the project's
    /// length unit and degrees as the context units of typed numbers.
    fn session(cx: &InteractionContext<'_>) -> Session {
        let mut session = cx.session().clone();
        if session.setting(LANGUAGE_SETTING).is_none() {
            let _ = session.set_setting(LANGUAGE_SETTING, serde_json::Value::from(cx.language()));
        }
        if let Ok(document) = cx.document() {
            let unit = archbuilder_model::spatial::length_unit(&document.snapshot());
            session.units = unit
                .unit()
                .copied()
                .into_iter()
                .chain(arc_base::Unit::by_name("degree").ok().copied())
                .collect();
        }
        session
    }

    /// Runs `step` of the tool with the pieces the context gives.
    fn with_tool(
        &mut self,
        cx: &InteractionContext<'_>,
        step: impl FnOnce(
            &mut Tool,
            &Env<'_>,
            &mut dyn FnMut(Invocation) -> Result<arc_command::Outcome>,
        ) -> Result<()>,
    ) -> Result<()> {
        let (Some(registry), Ok(document)) = (self.registry.as_ref(), cx.document()) else {
            return Ok(());
        };
        if cx.read_only() {
            return Ok(());
        }
        self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let session = Self::session(cx);
        let env = Env {
            registry,
            document,
            session: &session,
            selection: cx.selection(),
            seed: self.seed,
        };
        step(&mut self.tool, &env, &mut |invocation| {
            cx.invoke(invocation)
        })
    }

    /// The next invocation's seed.
    fn next_seed(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        self.seed
    }

    /// `text` as the active prompt reads it: a decimal comma becomes a point where the language
    /// writes one and the prompt takes no point (a point's coordinates are comma-separated).
    fn typed(&mut self, cx: &InteractionContext<'_>, text: &str) -> String {
        if let Some(id) = self.keyword_named(cx, text) {
            return id;
        }
        let takes_point = self
            .tool
            .line()
            .active()
            .and_then(|s| s.prompt())
            .is_some_and(|p| p.accepts.iter().any(|a| matches!(a, Accept::Point { .. })));
        if cx.decimal_separator() == ',' && !takes_point {
            text.replace(',', ".")
        } else {
            text.to_owned()
        }
    }
}

/// The English shortcut letter of each keyword of the wall prompts.
const ENGLISH_SHORTCUTS: [(&str, char); 8] = [
    ("thickness", 't'),
    ("height", 'h'),
    ("location", 'l'),
    ("flip", 'f'),
    ("close", 'c'),
    ("center", 'c'),
    ("exterior", 'e'),
    ("interior", 'i'),
];

impl WallInteraction {
    /// The keyword of the active prompt that `text` names in the interface language (its label
    /// or the English shortcut letter), as the identifier the command line takes. The command
    /// line itself takes the identifier and the prompt's own shortcut letter.
    fn keyword_named(&mut self, cx: &InteractionContext<'_>, text: &str) -> Option<String> {
        let typed = text.trim();
        let prompt = self.tool.line().active().and_then(|s| s.prompt())?.clone();
        if typed.is_empty() || prompt.keyword_options().next().is_none() {
            return None;
        }
        let language = cx.language();
        if self.messages.as_ref().is_none_or(|(l, _)| l != language) {
            let localizer = crate::localizer(&[language.to_owned()]).ok()?;
            self.messages = Some((language.to_owned(), localizer));
        }
        let (_, localizer) = self.messages.as_ref()?;
        let fold = |t: &str| arc_ui::fold_case(t, language);
        let wanted = fold(typed);
        let single = {
            let mut chars = wanted.chars();
            chars.next().filter(|_| chars.next().is_none())
        };
        prompt.keyword_options().find_map(|o| {
            let label = fold(&localizer.format(&o.label, &[]).text);
            let english = ENGLISH_SHORTCUTS
                .iter()
                .any(|(id, c)| *id == o.id && Some(*c) == single);
            (label == wanted || english).then(|| o.id.clone())
        })
    }
}

impl Interaction for WallInteraction {
    fn pointer(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        let Some(chain) = self.tool.chain().cloned() else {
            return false;
        };
        let mut ends = end_points(cx, chain.storey);
        ends.extend(chain.start.filter(|_| chain.walls >= 1));
        let mut hit = snapped(event, &ends);
        // A pointer that is not on an end point follows the 15 degree directions from the
        // segment's start, as a typed length does.
        self.angle = None;
        let from = self
            .tool
            .line()
            .base_point()
            .map(|p| [p.x, p.y])
            .or(chain.end);
        if let (Some((to, false)), Some(from)) = (hit, from)
            && self.tool.line().active().is_some()
            && let Some((snapped_to, degrees)) = snap_direction(from, to)
        {
            hit = Some((snapped_to, false));
            self.angle = Some((snapped_to, degrees));
        }
        let at = hit.map(|(p, _)| p);
        match event.kind {
            PointerKind::Move => {
                self.snap = hit.filter(|(_, on_end)| *on_end).map(|(p, _)| p);
                if at.is_some() {
                    self.cursor = at;
                }
                if let Some(at) = at {
                    self.tool.aim(at);
                }
                false
            }
            PointerKind::Click if event.button == Some(PointerButton::Primary) => {
                if let Some(at) = at {
                    self.cursor = Some(at);
                    // A click that the command refuses is kept for the next line action.
                    let _ = self.with_tool(cx, |tool, env, run| tool.click(env, run, at));
                }
                true
            }
            // The secondary button is Enter.
            PointerKind::Press if event.button == Some(PointerButton::Secondary) => {
                self.tool.stop();
                true
            }
            _ => false,
        }
    }

    fn dynamic(&mut self, _: &InteractionContext<'_>, _: &DynamicEvent) {}

    fn line(&mut self, cx: &InteractionContext<'_>, action: LineAction) -> Result<()> {
        // An error a click left comes first; one this action left (a rejected answer) is shown now.
        let pending = self.tool.take_error();
        let result = match action {
            LineAction::ClearInput => Ok(()),
            LineAction::Cancel => self.with_tool(cx, |tool, env, run| tool.escape(env, run)),
            LineAction::Back => self.with_tool(cx, |tool, env, run| tool.back(env, run)),
            // A product command named at a prompt (a toolbar button) ends the running tool first.
            LineAction::Submit(text) if self.tool.active() && self.names_command(&text) => {
                self.tool.stop();
                self.with_tool(cx, |tool, env, run| tool.text(env, run, &text))
            }
            LineAction::Submit(text) if !self.tool.active() && is_erase_word(text.trim()) => {
                self.intent(cx, &Intent::new(OBJECT_DELETE))
            }
            LineAction::Submit(text) => {
                let text = self.typed(cx, &text);
                self.with_tool(cx, |tool, env, run| tool.text(env, run, &text))
            }
            LineAction::Repeat => self.with_tool(cx, |tool, env, run| tool.text(env, run, "wall")),
            LineAction::Invoke(intent) => self.intent(cx, &intent),
            LineAction::CancelThen(intent) => {
                self.tool.stop();
                self.intent(cx, &intent)
            }
        };
        let fresh = self.tool.take_error();
        result
            .and(pending.map_or(Ok(()), Err))
            .and(fresh.map_or(Ok(()), Err))
    }

    fn output(&self) -> InteractionOutput {
        let line = self.tool.line();
        let prompt = line.active().and_then(|s| s.prompt()).cloned();
        let options = prompt
            .as_ref()
            .map_or_else(PromptControls::default, |p| PromptControls {
                options: p
                    .keyword_options()
                    .map(|o| OptionControl {
                        id: o.id.clone(),
                        label: o.label.clone(),
                        shortcut: o.shortcut,
                    })
                    .collect(),
                parameters: Vec::new(),
            });
        // The selection is the tab's: the shell fills `has_selection` and the highlight from it.
        InteractionOutput {
            status: LineStatus {
                active: self.tool.active(),
                answers: line.active().map_or(0, |s| s.answers().len()),
                has_selection: false,
                has_last: line.last().is_some(),
            },
            prompt,
            options,
            overlays: self
                .preview()
                .into_iter()
                .chain(self.marker())
                .chain(self.angle_label())
                .collect(),
            cursor: self.tool.chain().and(self.cursor),
            // An idle line takes inline arguments (`app.file.open path=<file>`); answers end at Enter.
            inline_arguments: !self.tool.active(),
            ..InteractionOutput::default()
        }
    }
}

impl WallInteraction {
    /// Whether `text` is the identifier of a command of the product.
    fn names_command(&self, text: &str) -> bool {
        self.registry
            .as_ref()
            .is_some_and(|r| r.get(text.trim()).is_some())
    }

    /// A command a menu, a key or a toolbar button started. The wall command without parameters
    /// starts the tool; any other command of the product runs with the selection and its
    /// parameters. Shell commands never arrive here.
    fn intent(&mut self, cx: &InteractionContext<'_>, intent: &Intent) -> Result<()> {
        let command = intent.command.as_str();
        if command == WALL_CREATE && intent.params.is_empty() {
            return self.with_tool(cx, |tool, env, run| tool.begin(env, run));
        }
        if !self.names_command(command) {
            return Ok(());
        }
        self.tool.stop();
        let mut invocation = Invocation::new(
            command.to_owned(),
            serde_json::Value::Object(intent.params.clone()),
        );
        invocation.seed = self.next_seed();
        cx.invoke(invocation).map(drop)
    }

    /// The angle of the snapped direction, beside the rubber band's end.
    fn angle_label(&self) -> Option<ProductOverlay> {
        let ([x, y], degrees) = self.angle?;
        let z = self.tool.chain()?.elevation;
        Some(ProductOverlay::Label {
            at: Point3::new(x, y, z),
            text: format!("{degrees}\u{b0}"),
        })
    }

    /// The marker of the end point the pointer snapped to.
    fn marker(&self) -> Option<ProductOverlay> {
        let at = self.snap?;
        let z = self.tool.chain()?.elevation;
        Some(ProductOverlay::Snap {
            kind: arc_ui::SnapKind::Endpoint,
            at: Point3::new(at[0], at[1], z),
        })
    }

    /// The rubber band: the baseline from the last point to the cursor, and the outline of the
    /// wall's footprint with the thickness and location line in effect.
    fn preview(&self) -> Vec<ProductOverlay> {
        let (Some(chain), Some(to)) = (self.tool.chain(), self.cursor) else {
            return Vec::new();
        };
        let prompt = self.tool.line().active().and_then(|s| s.prompt());
        let accepts_point =
            prompt.is_some_and(|p| p.accepts.iter().any(|a| matches!(a, Accept::Point { .. })));
        // The segment starts at the last point: the answered start, else the end of the last wall
        // (a wall typed in has no pointer answer).
        let from = self
            .tool
            .line()
            .base_point()
            .map(|p| [p.x, p.y])
            .or(chain.end)
            .filter(|_| accepts_point);
        let Some(from) = from else {
            return Vec::new();
        };
        let z = chain.elevation;
        let length = (to[0] - from[0]).hypot(to[1] - from[1]);
        let at = |p: [f64; 2]| Point3::new(p[0], p[1], z);
        let mut overlays = vec![ProductOverlay::Line {
            from: at(from),
            to: at(to),
        }];
        if length > 0.0 {
            // The values in effect now, which the prompt carries, else those of the chain.
            let arg = |name: &str| prompt.and_then(|p| p.message.args.get(name));
            let thickness = arg("thickness_m")
                .and_then(|t| t.parse::<f64>().ok())
                .unwrap_or(chain.thickness);
            let location = match arg("location")
                .map(String::as_str)
                .or(chain.location.as_deref())
            {
                Some("exterior") => LocationLine::Exterior,
                Some("interior") => LocationLine::Interior,
                _ => LocationLine::Center,
            };
            let flip = arg("flip").map_or(chain.flip, |f| f == "true");
            // A flipped wall has its baseline reversed.
            let (start, end) = if flip { (to, from) } else { (from, to) };
            let wall = archbuilder_model::wall::Wall {
                start,
                end,
                thickness,
                height: 1.0,
                base_offset: 0.0,
                location,
            };
            let (low, high) = wall.extent();
            let (nx, ny) = (-(end[1] - start[1]) / length, (end[0] - start[0]) / length);
            let corner = |p: [f64; 2], side: f64| at([p[0] + nx * side, p[1] + ny * side]);
            overlays.push(ProductOverlay::Path {
                points: vec![
                    corner(start, low),
                    corner(end, low),
                    corner(end, high),
                    corner(start, high),
                    corner(start, low),
                ],
                dashed: true,
            });
        }
        overlays
    }
}
