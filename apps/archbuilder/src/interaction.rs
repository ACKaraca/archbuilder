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
use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::Accept;
use arc_command::{Invocation, Registry, Session};
use arc_ui::egui::PointerButton;
use arc_ui::{
    DynamicEvent, Intent, LineAction, LineStatus, OptionControl, PointerEvent, PointerKind,
    ProductOverlay, PromptControls, WorkPlane,
};
use archbuilder_commands::structure::LANGUAGE_SETTING;
use archbuilder_commands::wall::{OBJECT_DELETE, WALL_CREATE, WALL_DELETE};
use archbuilder_model::spatial;
use archbuilder_model::wall::LocationLine;

use crate::insert_tool::{Hover, InsertTool, command as insert_command, kind_named};
use crate::wall_tool::{Env, Tool, storey_for};

/// A message of the application, named by identifier.
struct Message {
    message_id: &'static str,
}

/// What a delete says when doors and windows went with their wall.
const INSERTS_DELETED: Message = Message {
    message_id: "ab-notice-inserts-deleted",
};

/// The view ray through the pointer, given the point it has on the level plane: from the eye
/// through that point, or along the view direction in an orthographic view.
fn view_ray(cx: &InteractionContext<'_>, m: Point3) -> Option<crate::insert_tool::Ray> {
    let camera = cx.camera();
    Some(match camera.projection() {
        arc_geom::Projection::Perspective { .. } => {
            let eye = camera.eye();
            (
                [eye.x, eye.y, eye.z],
                [m.x - eye.x, m.y - eye.y, m.z - eye.z],
            )
        }
        arc_geom::Projection::Orthographic { .. } => {
            let f = camera.forward();
            (
                [m.x - f.x * 1.0e4, m.y - f.y * 1.0e4, m.z - f.z * 1.0e4],
                [f.x, f.y, f.z],
            )
        }
    })
}

/// The snap step of the cursor on the level plane, in meters.
pub const GRID: f64 = 0.01;

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

/// How far from a 15 degree step the pointer direction snaps, in degrees.
const ANGLE_WINDOW: f64 = crate::snap::ANGLE_WINDOW;

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

/// The baselines of the walls of `storey`, which a new wall snaps to and closes on.
fn baselines(cx: &InteractionContext<'_>, storey: ObjectId) -> Vec<[[f64; 2]; 2]> {
    let Ok(document) = cx.document() else {
        return Vec::new();
    };
    spatial::walls(&document.snapshot(), storey)
        .iter()
        .filter_map(|w| archbuilder_model::wall::Wall::from_values(&w.values))
        .map(|w| [w.start, w.end])
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
    /// The insert tool: doors and windows placed in walls.
    inserts: InsertTool,
    /// The wall under the pointer and the opening that follows it, while the insert tool runs.
    hover: Option<Hover>,
    /// The outlines of the selected doors and windows.
    highlight: Vec<ProductOverlay>,
    /// The plan symbols of the doors and windows, while the camera looks (nearly) straight down,
    /// with the snapshot they were made from.
    symbols: Option<(arc_doc::Snapshot, Vec<ProductOverlay>)>,
    /// Lines the interface shows above the command line: refusals of clicks and what a delete took.
    notices: Vec<String>,
    registry: Option<Registry>,
    /// The seed of the next invocation, advanced with each one so a reopened document does not
    /// repeat the identifiers of an earlier run.
    seed: u64,
    /// The pointer on the level plane, for the rubber band and the status bar.
    cursor: Option<[f64; 2]>,
    /// The end point the pointer snapped to, for its marker.
    snap: Option<(arc_ui::SnapKind, [f64; 2])>,
    /// The tracking lines to draw, each from the end point that tracks to the cursor.
    guides: Vec<[[f64; 2]; 2]>,
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
            inserts: InsertTool::new(),
            hover: None,
            highlight: Vec::new(),
            symbols: None,
            notices: Vec::new(),
            registry: archbuilder_commands::registry().ok(),
            seed: seed.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed),
            cursor: None,
            snap: None,
            guides: Vec::new(),
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

    /// Runs `step` of the insert tool with the pieces the context gives.
    fn with_inserts(
        &mut self,
        cx: &InteractionContext<'_>,
        step: impl FnOnce(
            &mut InsertTool,
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
        step(&mut self.inserts, &env, &mut |invocation| {
            cx.invoke(invocation)
        })
    }

    /// The command line of the tool that runs.
    fn current_line(&self) -> &arc_command::CommandLine {
        if self.inserts.running() {
            self.inserts.line()
        } else {
            self.tool.line()
        }
    }

    /// Ends both tools.
    fn stop_all(&mut self) {
        self.tool.stop();
        self.inserts.stop();
        self.hover = None;
    }

    /// The messages of the interface language.
    fn localizer(&mut self, language: &str) -> Option<&arc_app::locale::Localizer> {
        if self.messages.as_ref().is_none_or(|(l, _)| l != language) {
            let localizer = crate::localizer(&[language.to_owned()]).ok()?;
            self.messages = Some((language.to_owned(), localizer));
        }
        self.messages.as_ref().map(|(_, l)| l)
    }

    /// Shows `error` above the command line, in the interface language.
    fn notice_error(&mut self, cx: &InteractionContext<'_>, error: &Error) {
        let text = self
            .localizer(cx.language())
            .map_or_else(|| error.code().to_owned(), |l| l.format_error(error).text);
        self.push_notice(text);
    }

    fn push_notice(&mut self, text: String) {
        const KEPT: usize = 3;
        self.notices.push(text);
        if self.notices.len() > KEPT {
            self.notices.remove(0);
        }
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
            .current_line()
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
const ENGLISH_SHORTCUTS: [(&str, char); 12] = [
    ("width", 'w'),
    ("sill", 's'),
    ("flip_hand", 'f'),
    ("flip_facing", 'a'),
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
        let prompt = self
            .current_line()
            .active()
            .and_then(|s| s.prompt())?
            .clone();
        if typed.is_empty() || prompt.keyword_options().next().is_none() {
            return None;
        }
        let language = cx.language();
        let localizer = self.localizer(language)?;
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
        self.refresh_highlight(cx);
        if self.inserts.running() {
            return self.insert_pointer(cx, event);
        }
        let Some(chain) = self.tool.chain().cloned() else {
            return self.pick_insert(cx, event);
        };
        let walls = baselines(cx, chain.storey);
        // The origin, the start of the chain and the ends of the walls attract and track.
        let mut ends = vec![[0.0, 0.0]];
        ends.extend(chain.start.filter(|_| chain.walls >= 1));
        ends.extend(walls.iter().flatten().copied());
        // The segment starts at the last point; the pointer follows the directions from it only
        // while a prompt asks for a point.
        let from = self
            .tool
            .line()
            .base_point()
            .map(|p| [p.x, p.y])
            .or(chain.end)
            .filter(|_| self.tool.line().active().is_some());
        let hit = event.model.and_then(|m| {
            crate::snap::snap(&crate::snap::Input {
                pointer: [m.x, m.y],
                pixels_per_unit: event.pixels_per_unit,
                ends: &ends,
                walls: &walls,
                from,
            })
        });
        self.angle = hit.as_ref().and_then(|h| h.degrees.map(|d| (h.point, d)));
        self.guides = hit.as_ref().map_or_else(Vec::new, |h| h.guides.clone());
        let at = hit.as_ref().map(|h| h.point);
        match event.kind {
            PointerKind::Move => {
                self.snap = hit.as_ref().and_then(|h| {
                    let kind = match h.kind {
                        crate::snap::Kind::Endpoint => arc_ui::SnapKind::Endpoint,
                        crate::snap::Kind::Intersection => arc_ui::SnapKind::Intersection,
                        crate::snap::Kind::Nearest => arc_ui::SnapKind::Nearest,
                        _ => return None,
                    };
                    Some((kind, h.point))
                });
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
        let pending = self.tool.take_error().or_else(|| self.inserts.take_error());
        self.notices.clear();
        self.refresh_highlight(cx);
        let inserting = self.inserts.running();
        let result = match action {
            LineAction::ClearInput => Ok(()),
            LineAction::Cancel if inserting => {
                self.stop_all();
                Ok(())
            }
            LineAction::Cancel => self.with_tool(cx, |tool, env, run| tool.escape(env, run)),
            LineAction::Back if inserting => {
                self.with_inserts(cx, |tool, env, run| tool.back(env, run))
            }
            LineAction::Back => self.with_tool(cx, |tool, env, run| tool.back(env, run)),
            LineAction::Submit(text) if kind_named(&text).is_some() => {
                let kind = kind_named(&text);
                self.intent(cx, &Intent::new(kind.map_or(WALL_CREATE, insert_command)))
            }
            // A product command named at a prompt (a toolbar button) ends the running tool first.
            LineAction::Submit(text)
                if (inserting || self.tool.active()) && self.names_command(&text) =>
            {
                self.stop_all();
                self.with_tool(cx, |tool, env, run| tool.text(env, run, &text))
            }
            LineAction::Submit(text) if inserting => {
                let text = self.typed(cx, &text);
                let under = self.hover.as_ref().map(|h| h.wall);
                self.with_inserts(cx, |tool, env, run| tool.text(env, run, &text, under))
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
                self.stop_all();
                self.intent(cx, &intent)
            }
        };
        if !self.inserts.running() {
            self.hover = None;
        }
        let fresh = self.tool.take_error().or_else(|| self.inserts.take_error());
        result
            .and(pending.map_or(Ok(()), Err))
            .and(fresh.map_or(Ok(()), Err))
    }

    fn output(&self) -> InteractionOutput {
        let line = self.current_line();
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
                active: self.tool.active() || self.inserts.running(),
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
                .chain(self.tracking())
                .chain(self.angle_label())
                .chain(self.insert_overlays())
                .chain(self.symbols.iter().flat_map(|(_, o)| o.iter().cloned()))
                // Last, so that the selection's outline lies over the hover highlight of the wall.
                .chain(self.highlight.iter().cloned())
                .collect(),
            cursor: self
                .tool
                .chain()
                .map(|_| ())
                .or(self.inserts.running().then_some(()))
                .and(self.cursor),
            history: self.notices.clone(),
            // An idle line takes inline arguments (`app.file.open path=<file>`); answers end at Enter.
            inline_arguments: !self.tool.active() && !self.inserts.running(),
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
            self.inserts.stop();
            self.hover = None;
            return self.with_tool(cx, |tool, env, run| tool.begin(env, run));
        }
        if let Some(kind) = kind_named(command).filter(|_| intent.params.is_empty()) {
            self.tool.stop();
            return self.with_inserts(cx, |tool, env, run| tool.begin(env, run, kind));
        }
        if !self.names_command(command) {
            return Ok(());
        }
        self.stop_all();
        let mut invocation = Invocation::new(
            command.to_owned(),
            serde_json::Value::Object(intent.params.clone()),
        );
        invocation.seed = self.next_seed();
        let outcome = cx.invoke(invocation)?;
        // A wall goes with its doors and windows: say so.
        let inserts = outcome
            .value
            .get("inserts")
            .and_then(serde_json::Value::as_i64);
        if matches!(command, OBJECT_DELETE | WALL_DELETE)
            && let Some(count) = inserts.filter(|c| *c > 0)
            && let Some(localizer) = self.localizer(cx.language())
        {
            let text = localizer
                .format(
                    INSERTS_DELETED.message_id,
                    &[("count", arc_base::error::Value::Int(count))],
                )
                .text;
            self.push_notice(text);
        }
        Ok(())
    }

    /// A click with no tool running: an insert whose opening the view ray enters first is selected,
    /// in plan (where the wall's top hides it) and in 3D alike, ahead of its own wall.
    fn pick_insert(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        if event.kind != PointerKind::Click || event.button != Some(PointerButton::Primary) {
            return false;
        }
        let Some(model) = event.model else {
            return false;
        };
        let Some(ray) = view_ray(cx, model) else {
            return false;
        };
        let Some(snapshot) = cx.document().ok().map(arc_doc::Document::snapshot) else {
            return false;
        };
        let Some(id) = archbuilder_model::insert::pick(&snapshot, ray) else {
            return false;
        };
        let mode = if event.modifiers.ctrl {
            "toggle"
        } else {
            "replace"
        };
        // The shell owns the selection: it carries out the command and shows the result.
        let mut intent = Intent::new(archbuilder_commands::structure::OBJECT_SELECT);
        intent.params =
            serde_json::json!({"objects": [id.to_string()], "mode": mode, "gesture": "click"})
                .as_object()
                .cloned()
                .unwrap_or_default();
        cx.ask(intent);
        true
    }

    /// The outlines that mark the selected doors and windows, which the wall's body hides in plan.
    fn refresh_highlight(&mut self, cx: &InteractionContext<'_>) {
        self.highlight.clear();
        let Ok(document) = cx.document() else {
            return;
        };
        let snapshot = document.snapshot();
        // The plan symbols show in views from above (within about 12 degrees of straight down),
        // where the wall's top hides the insert; in other views they would float in the air.
        if cx.camera().forward().z < -0.98 {
            if self
                .symbols
                .as_ref()
                .is_none_or(|(s, _)| !s.ptr_eq(&snapshot))
            {
                self.symbols = Some((snapshot.clone(), crate::labels::symbols(&snapshot)));
            }
        } else {
            self.symbols = None;
        }
        for id in cx.selection().iter().take(1000) {
            let Some(object) = spatial::get(&snapshot, *id) else {
                continue;
            };
            let Some(item) = archbuilder_model::insert::Insert::of(&snapshot, &object) else {
                continue;
            };
            let Some(host) = object.header.owner.and_then(|w| spatial::get(&snapshot, w)) else {
                continue;
            };
            let Some(wall) = archbuilder_model::wall::Wall::of(&snapshot, &host) else {
                continue;
            };
            let level = host
                .header
                .owner
                .and_then(|s| spatial::get(&snapshot, s))
                .map_or(0.0, |s| spatial::storey_elevation(&snapshot, &s));
            for line in archbuilder_model::insert::outline(&wall, level, &item).unwrap_or_default()
            {
                self.highlight.push(ProductOverlay::Path {
                    points: line.iter().map(|p| Point3::new(p[0], p[1], p[2])).collect(),
                    dashed: false,
                });
            }
        }
    }

    /// The pointer while the insert tool runs: it moves a wall highlight and the opening along the
    /// wall under it, a click places the insert there, the secondary button ends the chain.
    fn insert_pointer(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        let Some(state) = self.inserts.state().cloned() else {
            return false;
        };
        // The opening's width in effect: the prompt's, else the chain's, else the kind's own.
        let width = self
            .inserts
            .line()
            .active()
            .and_then(|s| s.prompt())
            .and_then(|p| p.message.args.get("width_m"))
            .and_then(|w| w.parse::<f64>().ok())
            .or(state.width)
            .unwrap_or(match state.kind {
                archbuilder_model::insert::Kind::Door => archbuilder_commands::insert::DOOR_WIDTH,
                archbuilder_model::insert::Kind::Window => {
                    archbuilder_commands::insert::WINDOW_WIDTH
                }
            });
        let found = event.model.and_then(|m| {
            let snapshot = cx.document().ok()?.snapshot();
            let ray = view_ray(cx, m);
            crate::insert_tool::hover(
                &snapshot,
                state.storey,
                [m.x, m.y],
                ray,
                event.pixels_per_unit,
                width,
            )
        });
        if let Some(m) = event.model {
            self.cursor = Some([m.x, m.y]);
        }
        match event.kind {
            PointerKind::Move => {
                self.hover = found;
                false
            }
            PointerKind::Click if event.button == Some(PointerButton::Primary) => {
                self.hover = found.clone();
                if let Some(h) = found {
                    let _ = self.with_inserts(cx, |tool, env, run| {
                        tool.click(env, run, h.wall, h.distance, h.interior)
                    });
                    if let Some(error) = self.inserts.take_error() {
                        self.notice_error(cx, &error);
                    }
                }
                true
            }
            PointerKind::Press if event.button == Some(PointerButton::Secondary) => {
                self.stop_all();
                true
            }
            _ => false,
        }
    }

    /// The overlays of the insert tool: the wall under the pointer outlined at its top, and the
    /// opening that follows the pointer with its distance to the nearer end of the wall.
    fn insert_overlays(&self) -> Vec<ProductOverlay> {
        let (Some(h), Some(state)) = (&self.hover, self.inserts.state()) else {
            return Vec::new();
        };
        let at = |p: [f64; 2], z: f64| Point3::new(p[0], p[1], z);
        let outline = |c: &[[f64; 2]; 4], dashed| ProductOverlay::Path {
            points: c.iter().chain(c.first()).map(|p| at(*p, h.top)).collect(),
            dashed,
        };
        // The sill and the height in effect: the prompt's, else the chain's, else the kind's own.
        let arg = |name: &str| {
            self.inserts
                .line()
                .active()
                .and_then(|s| s.prompt())
                .and_then(|p| p.message.args.get(name))
                .and_then(|v| v.parse::<f64>().ok())
        };
        let kind = state.kind == archbuilder_model::insert::Kind::Door;
        let sill = arg("sill_m")
            .or(state.sill)
            .unwrap_or(if kind { 0.0 } else { 0.9 });
        let height = arg("height_m")
            .or(state.height)
            .unwrap_or(if kind { 2.1 } else { 1.2 });
        // The opening as it would stand in each face of the wall.
        let face = |a: [f64; 2], b: [f64; 2]| ProductOverlay::Path {
            points: vec![
                at(a, h.base + sill),
                at(b, h.base + sill),
                at(b, h.base + sill + height),
                at(a, h.base + sill + height),
                at(a, h.base + sill),
            ],
            dashed: true,
        };
        let near = h.distance.min(h.length - h.distance).max(0.0);
        let mid = [
            (h.opening[0][0] + h.opening[2][0]) / 2.0,
            (h.opening[0][1] + h.opening[2][1]) / 2.0,
        ];
        vec![
            outline(&h.footprint, false),
            outline(&h.opening, true),
            face(h.opening[0], h.opening[1]),
            face(h.opening[3], h.opening[2]),
            ProductOverlay::Label {
                at: at(mid, h.top),
                text: format!("{near:.2} m"),
            },
        ]
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

    /// The dashed tracking lines.
    fn tracking(&self) -> Vec<ProductOverlay> {
        let Some(z) = self.tool.chain().map(|c| c.elevation) else {
            return Vec::new();
        };
        self.guides
            .iter()
            .map(|g| ProductOverlay::Path {
                points: g.iter().map(|p| Point3::new(p[0], p[1], z)).collect(),
                dashed: true,
            })
            .collect()
    }

    /// The marker of the point the pointer snapped to.
    fn marker(&self) -> Option<ProductOverlay> {
        let (kind, at) = self.snap?;
        let z = self.tool.chain()?.elevation;
        Some(ProductOverlay::Snap {
            kind,
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
                start_join: archbuilder_model::wall::JoinStyle::Auto,
                end_join: archbuilder_model::wall::JoinStyle::Auto,
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
