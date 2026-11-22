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

mod habits;

pub use habits::{GRID_SETTING, SNAP_SETTING};

use std::sync::atomic::{AtomicU64, Ordering};

use arc_app::application::{Application, TabId};
use arc_app::shell::{
    ContextEntry, Interaction, InteractionContext, InteractionOutput, KeyInput, KeyReply,
    ViewRequest,
};
use arc_base::math::{Point3, Vector3};
use arc_base::{Error, ObjectId, Result};
use arc_command::prompt::Accept;
use arc_command::{Invocation, Invoker, Registry, Roots, Session};
use arc_ui::egui::PointerButton;
use arc_ui::{
    DynamicEvent, Intent, LineAction, LineStatus, OptionControl, PointerEvent, PointerKind,
    ProductOverlay, PromptControls, WorkPlane,
};
use archbuilder_commands::floor::{FLOOR_ADD_HOLE, FLOOR_CREATE};
use archbuilder_commands::room::ROOM_PLACE;
use archbuilder_commands::stair::STAIR_CREATE;
use archbuilder_commands::structure::LANGUAGE_SETTING;
use archbuilder_commands::wall::{OBJECT_DELETE, WALL_CREATE, WALL_DELETE};
use archbuilder_io::command::EXPORT_IFC;
use archbuilder_model::spatial;
use archbuilder_model::wall::LocationLine;
use std::path::{Path, PathBuf};

use crate::floor_tool::FloorTool;
use crate::insert_tool::{Hover, InsertTool, command as insert_command, kind_named};
use crate::modify_tool::{ModifyTool, command_named};
use crate::picker::Picker;
use crate::room_tool::RoomTool;
use crate::stair_tool::StairTool;
use crate::wall_tool::{Env, Tool, storey_for};

/// A message of the application, named by identifier.
struct Message {
    message_id: &'static str,
}

/// What a delete says when doors and windows went with their wall.
const INSERTS_DELETED: Message = Message {
    message_id: "ab-notice-inserts-deleted",
};

/// What a modify command says of the walls it could not reconnect.
const MODIFY_UNJOINED: Message = Message {
    message_id: "ab-notice-modify-unjoined",
};

/// What a modify command says of a door or window that now opens into another wall.
const MODIFY_MEETS: Message = Message {
    message_id: "ab-notice-modify-meets",
};

/// The point a click picked, echoed in the history.
const PICKED_POINT: Message = Message {
    message_id: "ab-notice-picked-point",
};

/// What a move or copy says when floor edges stopped following a wall.
const MODIFY_DISSOCIATED: Message = Message {
    message_id: "ab-notice-modify-dissociated",
};

/// What an export says when it wrote the file.
const IFC_EXPORTED: Message = Message {
    message_id: "ab-notice-ifc-exported",
};

/// What an export says when its loss report is not empty.
const IFC_LOSS: Message = Message {
    message_id: "ab-notice-ifc-loss",
};

/// What a delete says when floor edges lost their wall.
const FLOOR_EDGES_FROZEN: Message = Message {
    message_id: "ab-notice-floor-edges-frozen",
};

/// What `place_all` says of the slivers it skipped.
const ROOM_SLIVERS: Message = Message {
    message_id: "ab-notice-room-slivers",
};

/// What a paste says when the new objects lie exactly on others.
const PASTE_ON_TOP: Message = Message {
    message_id: "ab-notice-paste-on-top",
};

/// Whether a wall among `created` (identifiers as text) has the same baseline as another wall.
fn pasted_on_top(snapshot: &arc_doc::Snapshot, created: &[String]) -> bool {
    let walls: Vec<(String, archbuilder_model::wall::Wall)> =
        spatial::objects(snapshot, archbuilder_model::types::WALL)
            .iter()
            .filter_map(|o| {
                archbuilder_model::wall::Wall::from_values(&o.values).map(|w| (o.id.to_string(), w))
            })
            .collect();
    walls
        .iter()
        .filter(|(id, _)| created.contains(id))
        .any(|(id, a)| {
            walls
                .iter()
                .any(|(other, b)| other != id && a.start == b.start && a.end == b.end)
        })
}

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

/// How far from the top right corner of the viewport the view cube reaches, in pixels.
const CUBE_CLEARANCE: f64 = 150.0;
/// How far a label near the view cube moves to the left of the cursor, in pixels.
const CUBE_LABEL_SHIFT: f64 = 140.0;

/// What the dimension input says of an entry that is not a length.
const DIM_INVALID: Message = Message {
    message_id: "ab-dim-edit-invalid",
};

/// How far from an object a point of a plan view still picks it, in pixels.
const PICK_PIXELS: f64 = 5.0;

/// The snap step of the cursor on the level plane, in meters.
pub const GRID: f64 = 0.01;

/// The plane point input lands on: the level plane of the storey the wall tool draws on (the
/// selection's storey, else the lowest), for [`arc_app::shell::ShellConfig::work_plane`].
#[must_use]
pub fn work_plane(app: &Application, tab: TabId, selection: &[ObjectId]) -> Option<WorkPlane> {
    let snapshot = app.tab(tab).ok()?.document().snapshot();
    let (storey, elevation) = storey_for(&snapshot, selection)?;
    // The room drawing follows the storey the viewport works on and the selection.
    let rooms = crate::room_view::shared();
    rooms.set_active(Some(storey));
    rooms.set_selection(selection);
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

/// The folder a relative export path is taken in: the folder of the document's file when it was
/// saved, else the user's documents folder, else the home folder, else the working directory.
fn export_base(saved: Option<&Path>) -> PathBuf {
    if let Some(folder) = saved
        .and_then(Path::parent)
        .filter(|f| !f.as_os_str().is_empty())
    {
        return folder.to_path_buf();
    }
    let home = ["HOME", "USERPROFILE"]
        .iter()
        .find_map(|n| std::env::var_os(n).filter(|v| !v.is_empty()))
        .map(PathBuf::from);
    home.map_or_else(PathBuf::new, |h| {
        let documents = h.join("Documents");
        if documents.is_dir() { documents } else { h }
    })
}

/// The path a typed `archbuilder.file.export_ifc path=<file>` line names: `Some(None)` for the
/// command without a path, `None` when the line is another command.
fn export_line(text: &str) -> Option<Option<String>> {
    let rest = text.trim().strip_prefix(EXPORT_IFC)?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let path = rest
        .trim()
        .strip_prefix("path=")
        .map(|p| p.trim().trim_matches('"'));
    Some(path.filter(|p| !p.is_empty()).map(str::to_owned))
}

/// The most notices the history keeps.
const MAX_NOTICES: usize = 500;

/// Appends `text`, dropping the oldest notices beyond [`MAX_NOTICES`] and counting them.
fn push_bounded(notices: &mut Vec<String>, first: &mut u64, text: String) {
    notices.push(text);
    if notices.len() > MAX_NOTICES {
        notices.remove(0);
        *first += 1;
    }
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
    /// The floor tool: floors made from picked walls or a sketch, and their holes.
    floors: FloorTool,
    /// The modify tool: move, copy, rotate and mirror.
    modify: ModifyTool,
    /// The room tool: rooms placed by clicking inside regions closed by walls.
    rooms: RoomTool,
    /// The stair tool: stairs from a start and a direction.
    stairs: StairTool,
    /// The ghost of the stair the stair tool would make.
    stair_ghost: Vec<ProductOverlay>,
    /// The stair warnings the history last showed, and the snapshot they were found in, so each
    /// change (a stair made, an edit of the grid, a paste, an undo) says only what is new.
    stair_warnings: (Option<arc_doc::Snapshot>, Vec<String>),
    /// The size of the viewport's image in pixels, once a pointer event has shown it.
    view_size: Option<[f64; 2]>,
    /// What a click in a plan view picks, shared by the hover and the click.
    picker: Picker,
    /// The outline of what a click would pick, in a plan view.
    pick_hover: Vec<ProductOverlay>,
    /// The outline of the region under the pointer while the room tool runs.
    room_hover: Vec<ProductOverlay>,
    /// The walls the wall chain had when it last changed, to say once when it ends.
    chain_walls: usize,
    /// Whether the last call said a chain ended: the shell then adds no line of its own for the
    /// command that closed it.
    just_reported: bool,
    /// The copies the copy command had when it last changed, to say once when it ends.
    copy_count: usize,
    /// What the pointer would pick for a floor, outlined while the floor tool runs.
    floor_hover: Vec<ProductOverlay>,
    /// The wall under the pointer and the opening that follows it, while the insert tool runs.
    hover: Option<Hover>,
    /// The outlines of the selected doors and windows.
    highlight: Vec<ProductOverlay>,
    /// The plan symbols of the doors and windows, while the camera looks (nearly) straight down,
    /// with the snapshot they were made from.
    symbols: Option<(arc_doc::Snapshot, Vec<ProductOverlay>)>,
    /// The room warnings the history last showed, so a command says only what changed.
    room_warnings: Vec<String>,
    /// Lines the interface shows above the command line: refusals of clicks and what a delete took.
    notices: Vec<String>,
    /// The number of notices dropped from the front of `notices`: the sequence number of its
    /// first line, which the shell places its own lines by.
    notices_first: u64,
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
    /// The ghost of what the modify tool would leave.
    ghost: Vec<ProductOverlay>,
    /// The elevation of the modify tool's plane, while it runs.
    plane: Option<f64>,
    /// The messages of the interface language, for the keywords typed in it.
    messages: Option<(String, arc_app::locale::Localizer)>,
    /// Direct manipulation: dragging, handles and temporary dimensions.
    direct: crate::direct::Direct,
    /// The handles, dimensions and drag preview of the selection, refreshed each frame.
    direct_overlays: Vec<ProductOverlay>,
    /// The mouse pointer direct manipulation asks for.
    direct_icon: Option<arc_ui::egui::CursorIcon>,
    /// Whether the pointer is over the view cube's corner of the viewport.
    near_cube: bool,
    /// The screen's right direction on the plan, for placing labels.
    right_xy: [f64; 2],
    /// The pixels per meter of the last pointer event, which sizes the handles.
    ppu: f64,
    /// The project's length unit and the language's decimal separator, for the live labels.
    label_units: (archbuilder_model::units::LengthUnit, char),
    /// Tab, the context menu, the clipboard, shortcuts, hide and isolate (see [`habits`]).
    habits: habits::Habits,
}

impl WallInteraction {
    /// The interaction of a new tab.
    #[must_use]
    pub fn new(seed: &AtomicU64) -> Self {
        Self {
            tool: Tool::new(),
            inserts: InsertTool::new(),
            floors: FloorTool::new(),
            modify: ModifyTool::new(),
            rooms: RoomTool::new(),
            stairs: StairTool::new(),
            stair_ghost: Vec::new(),
            stair_warnings: (None, Vec::new()),
            view_size: None,
            picker: Picker::default(),
            pick_hover: Vec::new(),
            room_hover: Vec::new(),
            chain_walls: 0,
            just_reported: false,
            copy_count: 0,
            floor_hover: Vec::new(),
            hover: None,
            highlight: Vec::new(),
            symbols: None,
            room_warnings: Vec::new(),
            notices: Vec::new(),
            notices_first: 0,
            registry: archbuilder_commands::registry()
                .and_then(|mut r| archbuilder_io::register(&mut r).map(|()| r))
                .ok(),
            seed: seed.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed),
            cursor: None,
            snap: None,
            guides: Vec::new(),
            angle: None,
            plane: None,
            ghost: Vec::new(),
            messages: None,
            direct: crate::direct::Direct::new(),
            direct_overlays: Vec::new(),
            direct_icon: None,
            ppu: 0.0,
            near_cube: false,
            right_xy: [1.0, 0.0],
            label_units: (archbuilder_model::units::LengthUnit::Meter, '.'),
            habits: habits::Habits::default(),
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

    /// Runs `step` of the floor tool with the pieces the context gives.
    fn with_floors(
        &mut self,
        cx: &InteractionContext<'_>,
        step: impl FnOnce(
            &mut FloorTool,
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
        step(&mut self.floors, &env, &mut |invocation| {
            cx.invoke(invocation)
        })
    }

    /// Runs `step` of the room tool with the pieces the context gives.
    fn with_rooms(
        &mut self,
        cx: &InteractionContext<'_>,
        step: impl FnOnce(
            &mut RoomTool,
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
        step(&mut self.rooms, &env, &mut |invocation| {
            cx.invoke(invocation)
        })
    }

    /// Runs `step` of the stair tool with the pieces the context gives.
    fn with_stairs(
        &mut self,
        cx: &InteractionContext<'_>,
        step: impl FnOnce(
            &mut StairTool,
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
        step(&mut self.stairs, &env, &mut |invocation| {
            cx.invoke(invocation)
        })
    }

    /// Runs `step` of the modify tool with the pieces the context gives.
    fn with_modify(
        &mut self,
        cx: &InteractionContext<'_>,
        step: impl FnOnce(
            &mut ModifyTool,
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
        step(&mut self.modify, &env, &mut |invocation| {
            cx.invoke(invocation)
        })
    }

    /// The command line of the tool that runs.
    fn current_line(&self) -> &arc_command::CommandLine {
        if self.modify.running() {
            self.modify.line()
        } else if self.rooms.running() {
            self.rooms.line()
        } else if self.stairs.running() {
            self.stairs.line()
        } else if self.floors.running() {
            self.floors.line()
        } else if self.inserts.running() {
            self.inserts.line()
        } else {
            self.tool.line()
        }
    }

    /// Clears what the modify tool drew: the ghost, the rubber band's angle label, guides and marker.
    fn clear_modify_overlays(&mut self) {
        self.ghost.clear();
        self.plane = None;
        self.angle = None;
        self.guides.clear();
        self.snap = None;
    }

    /// Ends both tools.
    fn stop_all(&mut self) {
        self.tool.stop();
        self.inserts.stop();
        self.floors.stop();
        self.modify.stop();
        self.rooms.stop();
        self.stairs.stop();
        self.stair_ghost.clear();
        self.room_hover.clear();
        self.floor_hover.clear();
        self.hover = None;
    }

    /// The elevation of the plane the running tool works on.
    fn elevation(&self) -> Option<f64> {
        self.tool.chain().map(|c| c.elevation).or(self.plane)
    }

    /// Says `Done: <command>` once when a chain of walls or of copies ends after placing some.
    fn report_chains(&mut self, cx: &InteractionContext<'_>) {
        let walls = self.tool.chain().map_or(0, |c| c.walls);
        let copies = self.modify.copies();
        for (now, before, label) in [
            (walls, self.chain_walls, "ab-cmd-wall-create-label"),
            (copies, self.copy_count, "ab-cmd-modify-copy-label"),
        ] {
            if now < before
                && let Some(l) = self.localizer(cx.language())
            {
                let name = l.format(label, &[]).text;
                let text = l
                    .format("app-history-ran", &[("command", name.into())])
                    .text;
                self.push_notice(text);
                self.just_reported = true;
            }
        }
        self.chain_walls = walls;
        self.copy_count = copies;
        if self.modify.take_cancelled()
            && let Some(l) = self.localizer(cx.language())
        {
            let text = l.format("app-history-cancel", &[]).text;
            self.push_notice(text);
        }
        for (command, done) in self.modify.take_ran() {
            self.after_modify(cx, command, &done);
        }
    }

    /// Selects the copies a modify command made and says what it could not keep: floor edges that
    /// lost their wall, walls it could not reconnect, doors and windows that now open into a wall.
    fn after_modify(
        &mut self,
        cx: &InteractionContext<'_>,
        command: &str,
        done: &serde_json::Value,
    ) {
        // The new objects of a copy are selected, as the command's own selection says. A
        // mirror keeps what was selected: the originals when they stay, else the mirrored
        // objects themselves.
        let created: Vec<String> = done
            .get("created")
            .and_then(serde_json::Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if !created.is_empty() && command != archbuilder_commands::modify::MODIFY_MIRROR {
            let mut intent = Intent::new(archbuilder_commands::structure::OBJECT_SELECT);
            intent.params = serde_json::json!({"objects": created, "mode": "replace"})
                .as_object()
                .cloned()
                .unwrap_or_default();
            cx.ask(intent);
        }
        let mut lines: Vec<(&str, Vec<(&str, arc_base::error::Value)>)> = Vec::new();
        if let Some(count) = done
            .get("dissociated_edges")
            .and_then(serde_json::Value::as_i64)
            .filter(|c| *c > 0)
        {
            lines.push((
                MODIFY_DISSOCIATED.message_id,
                vec![("count", arc_base::error::Value::Int(count))],
            ));
        }
        let strings = |key: &str| -> Vec<String> {
            done.get(key)
                .and_then(serde_json::Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        // A paste that lies on its originals names that cause, not each collision it makes.
        let on_top = command == archbuilder_commands::modify::CLIPBOARD_PASTE
            && cx
                .document()
                .is_ok_and(|d| pasted_on_top(&d.snapshot(), &created));
        if on_top {
            lines.clear();
            lines.push((PASTE_ON_TOP.message_id, Vec::new()));
        }
        let unjoined = strings("unjoined");
        if !unjoined.is_empty() {
            lines.push((
                MODIFY_UNJOINED.message_id,
                vec![("names", unjoined.join(", ").into())],
            ));
        }
        for pair in done
            .get("meets")
            .filter(|_| !on_top)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .take(20)
        {
            let at = |k: usize| {
                pair.get(k)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
            };
            lines.push((
                MODIFY_MEETS.message_id,
                vec![("name", at(0).into()), ("other", at(1).into())],
            ));
        }
        if let Some(l) = self.localizer(cx.language()) {
            let texts: Vec<String> = lines
                .iter()
                .map(|(id, args)| l.format(id, args).text)
                .collect();
            for text in texts {
                self.push_notice(text);
            }
        }
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
        push_bounded(&mut self.notices, &mut self.notices_first, text);
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
const ENGLISH_SHORTCUTS: [(&str, char); 27] = [
    ("straight", 's'),
    ("l_turn", 'l'),
    ("u_turn", 'u'),
    ("tread", 't'),
    ("left", 'l'),
    ("right", 'r'),
    ("displacement", 'd'),
    ("reference", 'r'),
    ("yes", 'y'),
    ("no", 'n'),
    ("offset", 'o'),
    ("sketch", 's'),
    ("walls", 'w'),
    ("done", 'd'),
    ("undo", 'u'),
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
        self.just_reported = false;
        if event.kind == PointerKind::DoubleClick {
            self.double_click(cx, event);
            return false;
        }
        if matches!(event.kind, PointerKind::Move | PointerKind::Click) {
            self.habits.pointer = Some(*event);
        }
        let handled = self.pointer_inner(cx, event);
        self.report_chains(cx);
        self.note_last();
        if !self.modify.running() && !self.ghost.is_empty() {
            self.clear_modify_overlays();
        }
        handled
    }

    fn input_changed(&mut self, cx: &InteractionContext<'_>, text: &str) -> Option<LineAction> {
        self.habit_input(cx, text)
    }

    fn handles(&self, command: &str) -> bool {
        // Undo steps back inside a running command and cancels it at its first prompt.
        command == arc_app::commands::EDIT_UNDO
    }

    fn key(&mut self, cx: &InteractionContext<'_>, input: &KeyInput) -> KeyReply {
        self.habit_key(cx, input)
    }

    fn context_menu(&self, cx: &InteractionContext<'_>) -> Vec<ContextEntry> {
        self.habit_menu(cx)
    }

    fn take_view(&mut self) -> Option<ViewRequest> {
        self.habits.take_view()
    }

    fn frame(&mut self, cx: &InteractionContext<'_>) {
        let right = cx.camera().right();
        self.right_xy = [right.x, right.y];
        if let Ok(document) = cx.document() {
            let unit = archbuilder_model::spatial::length_unit(&document.snapshot());
            self.label_units = (unit, cx.decimal_separator());
        }
        self.habit_frame(cx);
        self.say_stair_warnings(cx);
        self.refresh_highlight(cx);
        self.refresh_direct(cx);
    }

    fn dynamic(&mut self, cx: &InteractionContext<'_>, event: &DynamicEvent) {
        if self.rename_event(cx, event) {
            return;
        }
        if let Some(done) = self.direct.edited(event) {
            self.run_direct(cx, done);
        }
        if self.direct.take_invalid()
            && let Some(l) = self.localizer(cx.language())
        {
            let text = l.format(DIM_INVALID.message_id, &[]).text;
            self.push_notice(text);
        }
        self.refresh_direct(cx);
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "one arm per line action and tool"
    )]
    fn line(&mut self, cx: &InteractionContext<'_>, action: LineAction) -> Result<()> {
        self.just_reported = false;
        // An error a click left comes first; one this action left (a rejected answer) is shown now.
        let pending = self
            .tool
            .take_error()
            .or_else(|| self.inserts.take_error())
            .or_else(|| self.floors.take_error())
            .or_else(|| self.rooms.take_error())
            .or_else(|| self.stairs.take_error())
            .or_else(|| self.modify.take_error());
        self.refresh_highlight(cx);
        self.update_view(cx, None);
        if matches!(action, LineAction::Cancel | LineAction::ClearInput) {
            self.picker.reset();
            self.direct.cancel();
            self.refresh_direct(cx);
        }
        // A line that is exactly a shortcut starts its command, as Enter on it does.
        let shortcut = match &action {
            LineAction::Submit(text) => self.shortcut_for(cx, text),
            _ => None,
        };
        let rooming = self.rooms.running() || self.stairs.running();
        let stairing = self.stairs.running();
        let inserting = self.inserts.running();
        let flooring = self.floors.running();
        let modifying = self.modify.running();
        let result = match action {
            LineAction::ClearInput => Ok(()),
            LineAction::Cancel if rooming => {
                self.stop_all();
                Ok(())
            }
            LineAction::Cancel if flooring => {
                self.with_floors(cx, |tool, env, run| tool.escape(env, run))
            }
            LineAction::Cancel if modifying => {
                self.modify.stop();
                Ok(())
            }
            LineAction::Cancel if inserting => {
                self.stop_all();
                Ok(())
            }
            LineAction::Cancel => self.with_tool(cx, |tool, env, run| tool.escape(env, run)),
            LineAction::Back if stairing => {
                self.with_stairs(cx, |tool, env, run| tool.back(env, run))
            }
            LineAction::Back if rooming => Ok(()),
            LineAction::Back if flooring => {
                self.with_floors(cx, |tool, env, run| tool.back(env, run))
            }
            LineAction::Back if modifying => {
                self.with_modify(cx, |tool, env, run| tool.back(env, run))
            }
            LineAction::Back if inserting => {
                self.with_inserts(cx, |tool, env, run| tool.back(env, run))
            }
            LineAction::Back => self.with_tool(cx, |tool, env, run| tool.back(env, run)),
            LineAction::Submit(text) if text.trim() == arc_app::commands::EDIT_UNDO => {
                self.undo(cx)
            }
            LineAction::Invoke(intent) if intent.command == arc_app::commands::EDIT_UNDO => {
                self.undo(cx)
            }
            LineAction::Submit(_) if shortcut.is_some() => {
                let command = shortcut.clone().unwrap_or_default();
                self.intent(cx, &Intent::new(command))
            }
            LineAction::Submit(text)
                if !(inserting || flooring || modifying || rooming || self.tool.active())
                    && self.habit_word(&text).is_some() =>
            {
                let intent = self
                    .habit_word(&text)
                    .unwrap_or_else(|| Intent::new(WALL_CREATE));
                self.intent(cx, &intent)
            }
            LineAction::Submit(text) if export_line(&text).is_some() => {
                self.stop_all();
                self.export_ifc(cx, export_line(&text).flatten())
            }
            LineAction::Submit(text)
                if (inserting || flooring || modifying || rooming || self.tool.active())
                    && self.names_command(&text) =>
            {
                self.stop_all();
                self.intent(cx, &Intent::new(text.trim()))
            }
            LineAction::Submit(text) if stairing => {
                let text = self.typed(cx, &text);
                let r = self.with_stairs(cx, |tool, env, run| tool.text(env, run, &text));
                // A refused answer is said here; the prompt stays.
                if let Some(error) = self.stairs.take_error() {
                    self.notice_error(cx, &error);
                }
                r
            }
            LineAction::Submit(text) if is_stair_word(&text) && !self.tool.active() => {
                self.intent(cx, &Intent::new(STAIR_CREATE))
            }
            LineAction::Submit(text) if rooming => {
                let text = self.typed(cx, &text);
                self.with_rooms(cx, |tool, env, run| tool.text(env, run, &text))
            }
            LineAction::Submit(text) if is_room_word(&text) && !self.tool.active() => {
                self.intent(cx, &Intent::new(ROOM_PLACE))
            }
            LineAction::Submit(text) if modifying => {
                let text = self.typed(cx, &text);
                self.with_modify(cx, |tool, env, run| tool.text(env, run, &text))
            }
            LineAction::Submit(text)
                if !inserting
                    && !flooring
                    && !self.tool.active()
                    && command_named(&text).is_some() =>
            {
                self.intent(
                    cx,
                    &Intent::new(command_named(&text).unwrap_or(WALL_CREATE)),
                )
            }
            LineAction::Submit(text) if flooring => {
                let text = self.typed(cx, &text);
                self.with_floors(cx, |tool, env, run| tool.text(env, run, &text))
            }
            LineAction::Submit(text) if kind_named(&text).is_some() => {
                let kind = kind_named(&text);
                self.intent(cx, &Intent::new(kind.map_or(WALL_CREATE, insert_command)))
            }
            LineAction::Submit(text) if inserting => {
                let text = self.typed(cx, &text);
                let under = self.hover.as_ref().map(|h| h.wall);
                self.with_inserts(cx, |tool, env, run| tool.text(env, run, &text, under))
            }
            LineAction::Submit(text) if is_floor_word(&text) && !self.tool.active() => {
                self.intent(cx, &Intent::new(FLOOR_CREATE))
            }
            LineAction::Submit(text) if text.trim() == FLOOR_ADD_HOLE => {
                self.intent(cx, &Intent::new(FLOOR_ADD_HOLE))
            }
            LineAction::Submit(text) if !self.tool.active() && is_erase_word(text.trim()) => {
                self.intent(cx, &Intent::new(OBJECT_DELETE))
            }
            LineAction::Submit(text) => {
                let text = self.typed(cx, &text);
                self.with_tool(cx, |tool, env, run| tool.text(env, run, &text))
            }
            LineAction::Repeat => self.repeat(cx),
            LineAction::Invoke(intent) => self.intent(cx, &intent),
            LineAction::CancelThen(intent) => {
                self.stop_all();
                self.intent(cx, &intent)
            }
        };
        if !self.inserts.running() {
            self.hover = None;
        }
        if !self.floors.running() {
            self.floor_hover.clear();
        }
        if !self.rooms.running() {
            self.room_hover.clear();
        }
        if !self.stairs.running() {
            self.stair_ghost.clear();
        }
        self.report_chains(cx);
        if !self.modify.running() {
            self.clear_modify_overlays();
        }
        self.note_last();
        if self.tool.take_undo() {
            // The wall taken back is undone with the document's own undo.
            cx.ask(Intent::new(arc_app::commands::EDIT_UNDO));
        }
        let fresh = self
            .tool
            .take_error()
            .or_else(|| self.inserts.take_error())
            .or_else(|| self.floors.take_error())
            .or_else(|| self.rooms.take_error())
            .or_else(|| self.stairs.take_error())
            .or_else(|| self.modify.take_error());
        result
            .and(pending.map_or(Ok(()), Err))
            .and(fresh.map_or(Ok(()), Err))
    }

    fn output(&self) -> InteractionOutput {
        let line = self.current_line();
        let prompt = line.active().and_then(|s| s.prompt()).cloned().or_else(|| {
            self.modify
                .gathering()
                .then(archbuilder_commands::modify::select_prompt)
        });
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
                active: self.tool.active()
                    || self.inserts.running()
                    || self.floors.running()
                    || self.rooms.running()
                    || self.stairs.running()
                    || self.modify.running()
                    || self.direct.active()
                    || self.habits.rename.is_some(),
                // The walls of a chain count: Back (Ctrl+Z, Undo last point) takes the last one back.
                answers: line.active().map_or(0, |s| s.answers().len())
                    + self.tool.chain().map_or(0, |c| c.walls),
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
                .chain(self.floor_overlays())
                .chain(self.room_hover.iter().cloned())
                .chain(self.stair_ghost.iter().cloned())
                .chain(self.pick_hover.iter().cloned())
                .chain(self.ghost.iter().cloned())
                .chain(self.direct_overlays.iter().cloned())
                .chain(self.symbols.iter().flat_map(|(_, o)| o.iter().cloned()))
                // Last, so that the selection's outline lies over the hover highlight of the wall.
                .chain(self.highlight.iter().cloned())
                .collect(),
            cursor: self
                .tool
                .chain()
                .map(|_| ())
                .or(self.inserts.running().then_some(()))
                .or(self.floors.running().then_some(()))
                .or(self.rooms.running().then_some(()))
                .or(self.stairs.running().then_some(()))
                .or(self.modify.prompting().then_some(()))
                .and(self.cursor),
            dynamic: self
                .direct
                .input()
                .map(|(_, field)| arc_app::shell::DynamicOutput {
                    prompt: crate::direct::EDIT_PROMPT.message_id.to_owned(),
                    fields: vec![field],
                })
                .or_else(|| {
                    self.habits
                        .rename_field()
                        .map(|field| arc_app::shell::DynamicOutput {
                            prompt: habits::rename_label().to_owned(),
                            fields: vec![field],
                        })
                }),
            pointer_icon: self.direct_icon,
            banner: self.habits.banner.clone(),
            hidden: self.habits.lists.hidden.clone(),
            isolated: self.habits.lists.isolated.clone(),
            history: self.notices.clone(),
            history_first: self.notices_first,
            // An idle line takes inline arguments (`app.file.open path=<file>`); answers end at Enter.
            inline_arguments: !self.tool.active()
                && !self.inserts.running()
                && !self.floors.running()
                && !self.rooms.running()
                && !self.stairs.running()
                && !self.modify.running(),
            // A chain of walls or of copies says once when it ends, not after each wall or copy.
            reports_results: self.just_reported
                || self.tool.chain().is_some_and(|c| c.walls > 0)
                || self.modify.copies() > 0,
            ..InteractionOutput::default()
        }
    }
}

impl WallInteraction {
    fn pointer_inner(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        if let Some(ppu) = event.pixels_per_unit.filter(|p| *p > 0.0) {
            self.ppu = ppu;
        }
        // The view cube stands in the top right corner of the viewport: labels keep clear of it.
        self.near_cube = self.view_size.is_some_and(|[w, _]| {
            event.pixel[0] > w - CUBE_CLEARANCE && event.pixel[1] < CUBE_CLEARANCE
        });
        self.refresh_highlight(cx);
        self.update_view(cx, Some(event));
        if self.modify.prompting() {
            return self.modify_pointer(cx, event);
        }
        if self.rooms.running() {
            return self.room_pointer(cx, event);
        }
        if self.stairs.running() {
            return self.stair_pointer(cx, event);
        }
        if self.floors.running() {
            return self.floor_pointer(cx, event);
        }
        if self.inserts.running() {
            return self.insert_pointer(cx, event);
        }
        let Some(chain) = self.tool.chain().cloned() else {
            if !self.tool.active()
                && let Some(taken) = self.direct_pointer(cx, event)
            {
                return taken;
            }
            return self.pick_idle(cx, event);
        };
        let walls = baselines(cx, chain.storey);
        let centers = self.room_centers(cx, chain.storey);
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
            crate::snap::snap(&self.snap_input([m.x, m.y], event, &ends, &walls, from, &centers))
        });
        self.angle = hit.as_ref().and_then(|h| h.degrees.map(|d| (h.point, d)));
        self.guides = hit.as_ref().map_or_else(Vec::new, |h| h.guides.clone());
        let at = hit.as_ref().map(|h| h.point);
        match event.kind {
            PointerKind::Move => {
                self.snap = hit
                    .as_ref()
                    .and_then(|h| h.kind.marker().map(|kind| (kind, h.point)));
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
            _ => false,
        }
    }

    /// The pointer while the modify tool asks for points: it snaps like the wall tool, a click is
    /// the next point; the secondary button opens the context menu, whose first entry is Enter.
    fn modify_pointer(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        let Some(snapshot) = cx.document().ok().map(arc_doc::Document::snapshot) else {
            return false;
        };
        let objects = self
            .modify
            .current()
            .map(|r| r.objects.clone())
            .unwrap_or_default();
        let fixed = self.modify.current().and_then(|r| r.storey).and_then(|s| {
            spatial::get(&snapshot, s).map(|o| (s, spatial::storey_elevation(&snapshot, &o)))
        });
        let Some((storey, z)) = fixed.or_else(|| storey_for(&snapshot, &objects)) else {
            return false;
        };
        self.plane = Some(z);
        let walls = baselines(cx, storey);
        let centers = self.room_centers(cx, storey);
        let mut ends = vec![[0.0, 0.0]];
        ends.extend(walls.iter().flatten().copied());
        let from = self.modify.line().base_point().map(|p| [p.x, p.y]);
        let hit = event.model.and_then(|m| {
            crate::snap::snap(&self.snap_input([m.x, m.y], event, &ends, &walls, from, &centers))
        });
        self.angle = hit.as_ref().and_then(|h| h.degrees.map(|d| (h.point, d)));
        self.guides = hit.as_ref().map_or_else(Vec::new, |h| h.guides.clone());
        let at = hit.as_ref().map(|h| h.point);
        match event.kind {
            PointerKind::Move => {
                self.snap = hit
                    .as_ref()
                    .and_then(|h| h.kind.marker().map(|kind| (kind, h.point)));
                if let Some(at) = at {
                    self.cursor = Some(at);
                    self.modify.aim(at);
                    self.ghost = self.make_ghost(&snapshot, &objects, at, z);
                }
                false
            }
            PointerKind::Click if event.button == Some(PointerButton::Primary) => {
                if let Some(at) = at {
                    self.cursor = Some(at);
                    let _ = self.with_modify(cx, |tool, env, run| tool.click(env, run, at));
                    if let Some(error) = self.modify.take_error() {
                        self.notice_error(cx, &error);
                    } else {
                        self.echo_point(cx, at);
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Says the point a click picked, in the project's unit, in the history.
    fn echo_point(&mut self, cx: &InteractionContext<'_>, at: [f64; 2]) {
        let Some(unit) = cx
            .document()
            .ok()
            .map(|d| archbuilder_model::spatial::length_unit(&d.snapshot()))
        else {
            return;
        };
        let separator = cx.decimal_separator();
        let text = |v: f64| archbuilder_model::units::length_text(v, unit, separator);
        let (x, y) = (text(at[0]), text(at[1]));
        if let Some(l) = self.localizer(cx.language()) {
            let line = l
                .format(PICKED_POINT.message_id, &[("x", x.into()), ("y", y.into())])
                .text;
            self.push_notice(line);
        }
    }

    /// The outline of the objects as the command would leave them with the pointer at `at`, and
    /// the line from the base point.
    fn make_ghost(
        &self,
        snapshot: &arc_doc::Snapshot,
        objects: &[ObjectId],
        at: [f64; 2],
        z: f64,
    ) -> Vec<ProductOverlay> {
        use archbuilder_model::transform::Xform;
        let (Some(run), Some(base)) = (self.modify.current(), self.modify.line().base_point())
        else {
            return Vec::new();
        };
        let base = [base.x, base.y];
        let key = self
            .modify
            .line()
            .active()
            .and_then(|s| s.prompt())
            .map(|p| p.key.clone())
            .unwrap_or_default();
        let xf = if run.command == archbuilder_commands::modify::MODIFY_ROTATE {
            ["angle", "reference", "new"]
                .iter()
                .any(|k| key.starts_with(k))
                .then(|| Xform::Rotate {
                    base,
                    angle: (at[1] - base[1]).atan2(at[0] - base[0]),
                })
        } else if run.command == archbuilder_commands::modify::MODIFY_MIRROR {
            key.starts_with("second")
                .then(|| Xform::mirror(base, at))
                .flatten()
        } else {
            key.starts_with("second")
                .then(|| Xform::Translate([at[0] - base[0], at[1] - base[1]]))
        };
        let at3 = |p: [f64; 2]| Point3::new(p[0], p[1], z);
        let mut out = vec![ProductOverlay::Line {
            from: at3(base),
            to: at3(at),
        }];
        let Some(xf) = xf else {
            return out;
        };
        let path = |points: &[[f64; 2]]| ProductOverlay::Path {
            points: points
                .iter()
                .chain(points.first())
                .map(|p| at3(xf.point(*p)))
                .collect(),
            dashed: true,
        };
        // Walls as they would stand, the neighbors that stretch to keep their joins included.
        let copying = matches!(
            run.command,
            archbuilder_commands::modify::MODIFY_COPY
                | archbuilder_commands::modify::CLIPBOARD_PASTE
        );
        if let Some(content) = &run.content {
            for poly in archbuilder_commands::clip::outlines(content) {
                out.push(path(&poly));
            }
            return out;
        }
        if !copying {
            for wall in archbuilder_commands::modify::preview(snapshot, objects, &xf) {
                out.push(ProductOverlay::Path {
                    points: wall
                        .plain_footprint()
                        .iter()
                        .chain(std::iter::once(&wall.plain_footprint()[0]))
                        .map(|p| at3(*p))
                        .collect(),
                    dashed: true,
                });
            }
        }
        for id in objects.iter().take(2000) {
            let Some(object) = spatial::get(snapshot, *id) else {
                continue;
            };
            if let Some(wall) = archbuilder_model::wall::Wall::from_values(&object.values) {
                if copying {
                    out.push(path(&wall.plain_footprint()));
                }
            } else if let Some(boundary) =
                archbuilder_model::floor::Floor::from_values(&object.values)
                    .and_then(|f| f.boundary_in(snapshot))
            {
                out.extend(boundary.loops.iter().map(|l| path(l)));
            }
        }
        out
    }
}

impl WallInteraction {
    /// Whether a tool that asks for points is running: direct manipulation leaves the pointer to it.
    fn tool_running(&self) -> bool {
        self.tool.active()
            || self.inserts.running()
            || self.floors.running()
            || self.rooms.running()
            || self.stairs.running()
            || self.modify.running()
    }

    /// The camera as direct manipulation reads it.
    fn direct_frame(cx: &InteractionContext<'_>, ppu: f64) -> crate::direct::Frame {
        let c = cx.camera();
        let v = |p: Vector3| [p.x, p.y, p.z];
        let eye = c.eye();
        let perspective = matches!(c.projection(), arc_geom::Projection::Perspective { .. });
        crate::direct::Frame {
            ppu,
            right: v(c.right()),
            up: v(c.up()),
            forward: v(c.forward()),
            eye: perspective.then_some([eye.x, eye.y, eye.z]),
            target_depth: (c.target() - eye).norm(),
            top: c.forward().z < -0.98,
        }
    }

    /// Recomputes the handles, dimensions and drag preview.
    fn refresh_direct(&mut self, cx: &InteractionContext<'_>) {
        self.direct_overlays.clear();
        self.direct_icon = None;
        if self.tool_running() || cx.read_only() {
            self.direct.cancel();
            return;
        }
        let (Ok(document), true) = (cx.document(), self.ppu > 0.0) else {
            return;
        };
        let snapshot = document.snapshot();
        let unit = archbuilder_model::spatial::length_unit(&snapshot);
        let frame = Self::direct_frame(cx, self.ppu);
        self.direct_overlays = self.direct.overlays(
            &snapshot,
            cx.selection(),
            &frame,
            unit,
            cx.decimal_separator(),
        );
        self.direct_icon = self.direct.icon(cx.selection());
    }

    /// Runs the commands of a finished gesture, each one undo step, and says what they report.
    fn run_direct(&mut self, cx: &InteractionContext<'_>, done: crate::direct::Done) {
        for command in done.commands {
            let mut invocation = Invocation::new(command.name, command.params);
            invocation.seed = self.next_seed();
            match cx.invoke(invocation) {
                Ok(outcome) => self.after_modify(cx, command.name, &outcome.value),
                Err(error) => self.notice_error(cx, &error),
            }
        }
    }

    /// Direct manipulation of the selection with the pointer; `None` when it leaves the event to
    /// the viewport and the picking.
    fn direct_pointer(
        &mut self,
        cx: &InteractionContext<'_>,
        event: &PointerEvent,
    ) -> Option<bool> {
        let document = cx.document().ok()?;
        if cx.read_only() {
            return None;
        }
        let snapshot = document.snapshot();
        if let Some(ppu) = event.pixels_per_unit.filter(|p| *p > 0.0) {
            self.ppu = ppu;
        }
        let input = crate::direct::Input {
            snapshot: &snapshot,
            selection: cx.selection(),
            frame: Self::direct_frame(cx, self.ppu.max(1.0e-6)),
            ray: event.model.and_then(|m| view_ray(cx, m)),
            pixel: event.pixel,
            at: event.model.map(|m| [m.x, m.y]),
            shift: event.modifiers.shift,
            ctrl: event.modifiers.ctrl,
            free: event.modifiers.alt || !self.habits.snap_on,
            unit: archbuilder_model::spatial::length_unit(&snapshot),
            separator: cx.decimal_separator(),
        };
        let primary = event.button == Some(PointerButton::Primary);
        let taken = match event.kind {
            PointerKind::Move => {
                let taken = self.direct.moved(&input);
                if let Some(id) = self.direct.take_select() {
                    let mut intent = Intent::new(archbuilder_commands::structure::OBJECT_SELECT);
                    intent.params = serde_json::json!({
                        "objects": [id.to_string()], "mode": "replace", "gesture": "drag"
                    })
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                    cx.ask(intent);
                }
                if taken {
                    self.pick_hover.clear();
                }
                taken.then_some(true)
            }
            PointerKind::Press if primary => self.direct.press(&input).then_some(true),
            PointerKind::Release if primary => {
                if let Some(done) = self.direct.release(&input) {
                    self.run_direct(cx, done);
                }
                None
            }
            PointerKind::Click if primary => {
                if self.direct.take_swallow() {
                    Some(true)
                } else if !input.frame.top
                    && let Some(id) = crate::direct::body_at(&input)
                {
                    // The press was the engine's, so the viewport did not see it: select here.
                    let mode = if event.modifiers.command {
                        "toggle"
                    } else {
                        "replace"
                    };
                    let mut intent = Intent::new(archbuilder_commands::structure::OBJECT_SELECT);
                    intent.params = serde_json::json!({
                        "objects": [id.to_string()], "mode": mode, "gesture": "click"
                    })
                    .as_object()
                    .cloned()
                    .unwrap_or_default();
                    cx.ask(intent);
                    Some(true)
                } else {
                    None
                }
            }
            _ => None,
        };
        self.refresh_direct(cx);
        taken
    }

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
        if let Some(result) = self.ui_intent(cx, intent) {
            return result;
        }
        self.note_intent(intent);
        let command = intent.command.as_str();
        if command == EXPORT_IFC {
            self.stop_all();
            let path = intent
                .params
                .get("path")
                .and_then(serde_json::Value::as_str);
            return self.export_ifc(cx, path.map(str::to_owned));
        }
        if command == WALL_CREATE && intent.params.is_empty() {
            self.inserts.stop();
            self.hover = None;
            return self.with_tool(cx, |tool, env, run| tool.begin(env, run));
        }
        if command == FLOOR_CREATE && intent.params.is_empty() {
            self.tool.stop();
            self.inserts.stop();
            self.hover = None;
            return self.with_floors(cx, |tool, env, run| tool.begin(env, run, None));
        }
        if command == ROOM_PLACE && intent.params.is_empty() {
            self.stop_all();
            return self.with_rooms(cx, |tool, env, run| tool.begin(env, run));
        }
        if command == STAIR_CREATE && intent.params.is_empty() {
            self.stop_all();
            return self.with_stairs(cx, |tool, env, run| tool.begin(env, run));
        }
        if command == FLOOR_ADD_HOLE && intent.params.is_empty() {
            self.tool.stop();
            self.inserts.stop();
            self.hover = None;
            let snapshot = cx.document()?.snapshot();
            let floor = cx.selection().iter().copied().find(|id| {
                spatial::type_of(&snapshot, *id).as_deref() == Some(archbuilder_model::types::FLOOR)
            });
            let Some(floor) = floor else {
                return Err(Error::new(
                    archbuilder_commands::error::codes::NO_FLOOR_SELECTED,
                ));
            };
            return self.with_floors(cx, |tool, env, run| tool.begin(env, run, Some(floor)));
        }
        if let Some(id) = modify_id(command).filter(|_| intent.params.is_empty()) {
            self.stop_all();
            return self.with_modify(cx, |tool, env, run| tool.begin(env, run, id));
        }
        if let Some(kind) = kind_named(command).filter(|_| intent.params.is_empty()) {
            self.tool.stop();
            self.floors.stop();
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
        self.after_command(cx);
        self.say_slivers(cx, &outcome.value);
        // Floor edges that followed a deleted wall stay put: say so.
        let frozen = outcome
            .value
            .get("frozen_edges")
            .and_then(serde_json::Value::as_i64);
        if let Some(count) = frozen.filter(|c| *c > 0)
            && let Some(localizer) = self.localizer(cx.language())
        {
            let text = localizer
                .format(
                    FLOOR_EDGES_FROZEN.message_id,
                    &[("count", arc_base::error::Value::Int(count))],
                )
                .text;
            self.push_notice(text);
        }
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

    /// After a command of the product: what the pointer outlined may be gone (a delete), so the
    /// next move outlines again; the room warnings are said.
    fn after_command(&mut self, cx: &InteractionContext<'_>) {
        self.pick_hover.clear();
        self.say_room_warnings(cx);
    }

    /// Says the stair warnings (stride, overlap, uncut opening) that a change of the document
    /// brought, once each: checked when the snapshot changes, whatever changed it.
    fn say_stair_warnings(&mut self, cx: &InteractionContext<'_>) {
        let Ok(document) = cx.document() else {
            return;
        };
        let snapshot = document.snapshot();
        if self
            .stair_warnings
            .0
            .as_ref()
            .is_some_and(|s| s.ptr_eq(&snapshot))
        {
            return;
        }
        let first = self.stair_warnings.0.is_none();
        let errors: Vec<Error> = spatial::objects(&snapshot, archbuilder_model::types::STAIR)
            .iter()
            .take(200)
            .flat_map(|o| archbuilder_model::validate::stair_warnings(&snapshot, o))
            .collect();
        let Some(localizer) = self.localizer(cx.language()) else {
            return;
        };
        let found: Vec<String> = errors
            .iter()
            .map(|e| localizer.format_error(e).text)
            .collect();
        let fresh: Vec<String> = found
            .iter()
            .filter(|t| !self.stair_warnings.1.contains(t))
            .cloned()
            .collect();
        self.stair_warnings = (Some(snapshot), found);
        // A document that opens with warnings does not repeat them all in the history.
        if !first {
            for text in fresh.into_iter().take(8) {
                self.push_notice(text);
            }
        }
    }

    /// Says how many slivers between walls `place_all` skipped, when it skipped any.
    fn say_slivers(&mut self, cx: &InteractionContext<'_>, value: &serde_json::Value) {
        // Slivers between walls get no room: say how many were skipped.
        if let Some(count) = value
            .get("slivers")
            .and_then(serde_json::Value::as_i64)
            .filter(|c| *c > 0)
            && let Some(localizer) = self.localizer(cx.language())
        {
            let text = localizer
                .format(
                    ROOM_SLIVERS.message_id,
                    &[
                        ("count", arc_base::error::Value::Int(count)),
                        (
                            "least",
                            format!("{:.2}", archbuilder_model::room::MIN_ROOM_AREA).into(),
                        ),
                    ],
                )
                .text;
            self.push_notice(text);
        }
    }

    /// Says in the history which rooms a command left unenclosed, redundant or with a duplicate
    /// number, once for each change of the set.
    fn say_room_warnings(&mut self, cx: &InteractionContext<'_>) {
        let Ok(document) = cx.document() else {
            return;
        };
        let snapshot = document.snapshot();
        let Some(localizer) = self.localizer(cx.language()) else {
            return;
        };
        let found: Vec<String> = spatial::objects(&snapshot, archbuilder_model::types::STOREY)
            .iter()
            .flat_map(|s| archbuilder_model::validate::room_findings(&snapshot, s.id))
            .map(|e| localizer.format_error(&e).text)
            .collect();
        let fresh: Vec<String> = found
            .iter()
            .filter(|t| !self.room_warnings.contains(t))
            .cloned()
            .collect();
        self.room_warnings = found;
        for text in fresh.into_iter().take(8) {
            self.push_notice(text);
        }
    }

    /// Exports the document as an IFC4 file to `path` and says so in the history. Without a path
    /// the shell is asked for one: it shows the save dialog the product registered for the
    /// command and sends the command back with the chosen file. The file is written by the
    /// export command inside a session whose only write root is the directory of the path, as
    /// the shell's save does.
    fn export_ifc(&mut self, cx: &InteractionContext<'_>, path: Option<String>) -> Result<()> {
        let Some(path) = path else {
            cx.ask(Intent::new(EXPORT_IFC));
            return Ok(());
        };
        let path = Some(path)
            .filter(|p| !p.trim().is_empty())
            .ok_or_else(|| Error::new(archbuilder_io::error::codes::PATH_REQUIRED))?;
        let target = export_base(cx.document_path()).join(path.trim());
        let target = std::path::absolute(target)
            .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
        let directory = target
            .parent()
            .map_or_else(|| target.clone(), PathBuf::from);
        let mut session = Self::session(cx);
        session.roots = Roots::new([&directory], [&directory])?;
        let seed = self.next_seed();
        let (Some(registry), document) = (self.registry.as_ref(), cx.document()?) else {
            return Ok(());
        };
        let mut invocation = Invocation::new(
            EXPORT_IFC.to_owned(),
            serde_json::json!({ "path": target.display().to_string() }),
        );
        invocation.seed = seed;
        let outcome = Invoker::new(registry).invoke(document, &session, invocation)?;
        let number = |key: &str| outcome.value.get(key).and_then(serde_json::Value::as_i64);
        // The full path, so a relative name still says where the file went.
        let name = target.display().to_string();
        let (count, loss) = (
            number("elements").unwrap_or(0),
            number("dropped").unwrap_or(0) + number("approximated").unwrap_or(0),
        );
        if let Some(localizer) = self.localizer(cx.language()) {
            let said =
                |id: &str, args: &[(&str, arc_base::error::Value)]| localizer.format(id, args).text;
            let mut lines = vec![said(
                IFC_EXPORTED.message_id,
                &[
                    ("name", name.into()),
                    ("count", arc_base::error::Value::Int(count)),
                ],
            )];
            if loss > 0 {
                lines.push(said(
                    IFC_LOSS.message_id,
                    &[("count", arc_base::error::Value::Int(loss))],
                ));
            }
            for line in lines {
                self.push_notice(line);
            }
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
        let Some(id) = archbuilder_model::insert::pick(&snapshot, ray)
            .filter(|id| self.habits.vis.shows(&snapshot, *id))
        else {
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
            if self.habits.vis.active()
                || self
                    .symbols
                    .as_ref()
                    .is_none_or(|(s, _)| !s.ptr_eq(&snapshot))
            {
                let vis = &self.habits.vis;
                self.symbols = Some((
                    snapshot.clone(),
                    crate::labels::symbols(&snapshot, &|id| vis.shows(&snapshot, id)),
                ));
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
    /// wall under it, a click places the insert there, a secondary click opens the context menu.
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
        let z = self.elevation()?;
        // While a wall is drawn the label shows its live length and angle at the cursor.
        let live = self.tool.chain().and_then(|chain| {
            let from = self
                .tool
                .line()
                .base_point()
                .map(|p| [p.x, p.y])
                .or(chain.end)
                .filter(|_| self.tool.line().active().is_some())?;
            Some((from, self.cursor?))
        });
        if let Some((from, to)) = live {
            let length = (to[0] - from[0]).hypot(to[1] - from[1]);
            if length > GRID {
                let degrees = (to[1] - from[1])
                    .atan2(to[0] - from[0])
                    .to_degrees()
                    .rem_euclid(360.0);
                let (unit, separator) = self.label_units;
                // Near the view cube the label moves to the left of the cursor.
                let back = if self.near_cube && self.ppu > 0.0 {
                    CUBE_LABEL_SHIFT / self.ppu
                } else {
                    0.0
                };
                return Some(ProductOverlay::Label {
                    at: Point3::new(
                        to[0] - self.right_xy[0] * back,
                        to[1] - self.right_xy[1] * back,
                        z,
                    ),
                    text: format!(
                        "{}  {degrees:.1}\u{b0}",
                        archbuilder_model::units::length_text(length, unit, separator)
                    ),
                });
            }
        }
        let ([x, y], degrees) = self.angle?;
        Some(ProductOverlay::Label {
            at: Point3::new(x, y, z),
            text: format!("{degrees}\u{b0}"),
        })
    }

    /// The dashed tracking lines.
    fn tracking(&self) -> Vec<ProductOverlay> {
        let Some(z) = self.elevation() else {
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
        let z = self.elevation()?;
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

/// The identifier of a modify command, as `'static`.
fn modify_id(command: &str) -> Option<&'static str> {
    use archbuilder_commands::modify::{MODIFY_COPY, MODIFY_MIRROR, MODIFY_MOVE, MODIFY_ROTATE};
    [MODIFY_MOVE, MODIFY_COPY, MODIFY_ROTATE, MODIFY_MIRROR]
        .into_iter()
        .find(|c| *c == command)
}

/// Whether `text` asks for the room tool.
fn is_room_word(text: &str) -> bool {
    let t = text.trim();
    [ROOM_PLACE, "room", "oda"]
        .iter()
        .any(|w| t.eq_ignore_ascii_case(w) || t.to_lowercase() == *w)
}

/// Whether `text` asks for the stair tool.
fn is_stair_word(text: &str) -> bool {
    let t = text.trim();
    [STAIR_CREATE, "stair", "merdiven"]
        .iter()
        .any(|w| t.eq_ignore_ascii_case(w) || t.to_lowercase() == *w)
}

/// Whether `text` asks for the floor tool.
fn is_floor_word(text: &str) -> bool {
    let t = text.trim();
    [FLOOR_CREATE, "floor", "slab", "döşeme", "doseme"]
        .iter()
        .any(|w| t.eq_ignore_ascii_case(w) || t.to_lowercase() == *w)
}

impl WallInteraction {
    /// The pointer while the room tool runs: it outlines the region under it, a click places a
    /// room there, a secondary click opens the context menu.
    fn room_pointer(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        let Some(storey) = self.rooms.storey() else {
            return false;
        };
        let at = event.model.map(|m| [m.x, m.y]);
        if let Some(at) = at {
            self.cursor = Some(at);
        }
        match event.kind {
            PointerKind::Move => {
                self.room_hover = match (at, cx.document()) {
                    (Some(at), Ok(document)) => {
                        let snapshot = document.snapshot();
                        let z = spatial::get(&snapshot, storey)
                            .map_or(0.0, |s| spatial::storey_elevation(&snapshot, &s));
                        let walls = archbuilder_model::floor::storey_walls(&snapshot, storey);
                        archbuilder_model::room::footprints(&walls, &[at])
                            .into_iter()
                            .filter(|f| !f.polygon.is_empty())
                            .map(|f| ProductOverlay::Path {
                                points: f
                                    .polygon
                                    .iter()
                                    .chain(f.polygon.first())
                                    .map(|p| Point3::new(p[0], p[1], z))
                                    .collect(),
                                dashed: true,
                            })
                            .collect()
                    }
                    _ => Vec::new(),
                };
                false
            }
            PointerKind::Click if event.button == Some(PointerButton::Primary) => {
                if let Some(at) = at {
                    let _ = self.with_rooms(cx, |tool, env, run| tool.click(env, run, at));
                    if let Some(error) = self.rooms.take_error() {
                        self.notice_error(cx, &error);
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// The pointer while the stair tool runs: it snaps like the floor sketch (wall corners and
    /// ends, tracking lines, the grid); once the start is given it draws the stair the pointer
    /// would make; a click is the next point.
    fn stair_pointer(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        let Some((storey, z)) = self.stairs.storey() else {
            return false;
        };
        let Ok(document) = cx.document() else {
            return false;
        };
        let snapshot = document.snapshot();
        let walls = archbuilder_model::floor::storey_walls(&snapshot, storey);
        let baselines: Vec<[[f64; 2]; 2]> = walls.iter().map(|(_, w)| [w.start, w.end]).collect();
        let centers = self.room_centers(cx, storey);
        let mut ends = vec![[0.0, 0.0]];
        ends.extend(walls.iter().flat_map(|(_, w)| w.plain_footprint()));
        let from = self.stairs.line().base_point().map(|p| [p.x, p.y]);
        let hit = event.model.and_then(|m| {
            crate::snap::snap(&self.snap_input(
                [m.x, m.y],
                event,
                &ends,
                &baselines,
                from,
                &centers,
            ))
        });
        self.angle = hit.as_ref().and_then(|h| h.degrees.map(|d| (h.point, d)));
        self.guides = hit.as_ref().map_or_else(Vec::new, |h| h.guides.clone());
        self.snap = hit
            .as_ref()
            .and_then(|h| h.kind.marker().map(|kind| (kind, h.point)));
        let at = hit.map(|h| h.point);
        if let Some(at) = at {
            self.cursor = Some(at);
        }
        match event.kind {
            PointerKind::Move => {
                self.stair_ghost = at
                    .and_then(|at| self.stairs.preview(&snapshot, at))
                    .map(|stair| stair_ghost(&snapshot, storey, z, &stair))
                    .unwrap_or_default();
                false
            }
            PointerKind::Click if event.button == Some(PointerButton::Primary) => {
                if let Some(at) = at {
                    let _ = self.with_stairs(cx, |tool, env, run| tool.click(env, run, at));
                    if let Some(error) = self.stairs.take_error() {
                        self.notice_error(cx, &error);
                    }
                    self.stair_ghost.clear();
                }
                true
            }
            _ => false,
        }
    }

    /// Tells the room drawing what a view from above shows: the image size is learned from a
    /// pointer event (the pointer's pixel and model point fix the image's middle), the scale and
    /// middle come from the camera each time. Other views clear it.
    fn update_view(&mut self, cx: &InteractionContext<'_>, event: Option<&PointerEvent>) {
        let camera = cx.camera();
        let arc_geom::Projection::Orthographic { height } = camera.projection() else {
            crate::room_view::shared().set_view(None);
            return;
        };
        if camera.forward().z >= -0.98 || height <= 0.0 {
            crate::room_view::shared().set_view(None);
            return;
        }
        let (t, r, u) = (camera.target(), camera.right(), camera.up());
        if let Some(e) = event
            && let (Some(m), Some(ppu)) = (e.model, e.pixels_per_unit)
        {
            let along = (m.x - t.x) * r.x + (m.y - t.y) * r.y;
            let (w, h) = (2.0 * (e.pixel[0] - along * ppu), height * ppu);
            if w.is_finite() && w > 0.0 && h.is_finite() && h > 0.0 {
                self.view_size = Some([w, h]);
            }
        }
        let view = self.view_size.map(|[w, h]| {
            let norm = |x: f64, y: f64| {
                let n = x.hypot(y);
                if n > 0.0 { [x / n, y / n] } else { [x, y] }
            };
            crate::room_view::ViewRect {
                center: [t.x, t.y],
                right: norm(r.x, r.y),
                up: norm(u.x, u.y),
                ppu: h / height,
                size: [w, h],
            }
        });
        crate::room_view::shared().set_view(view);
    }

    /// With no tool running: in a view from above one ordered list ([`Picker`]) decides what the
    /// hover outlines and what a click selects; in other views a click picks the door or window
    /// first and the rest is left to the viewport.
    fn pick_idle(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        if cx.camera().forward().z >= -0.98 {
            self.pick_hover.clear();
            return self.pick_insert(cx, event);
        }
        let (Some(model), Ok(document)) = (event.model, cx.document()) else {
            self.pick_hover.clear();
            return false;
        };
        let snapshot = document.snapshot();
        let aperture = event
            .pixels_per_unit
            .filter(|p| *p > 0.0)
            .map_or(0.05, |p| PICK_PIXELS / p);
        let vis = &self.habits.vis;
        let target = self.picker.target(
            &snapshot,
            &crate::picker::Probe {
                pixel: event.pixel,
                at: [model.x, model.y],
                aperture,
                ray: view_ray(cx, model),
                now: std::time::Instant::now(),
                shown: &|id| vis.shows(&snapshot, id),
            },
        );
        match event.kind {
            PointerKind::Move => {
                self.pick_hover = target
                    .as_ref()
                    .map(|t| {
                        self.picker
                            .outline(&snapshot, t.id)
                            .into_iter()
                            .map(|loop_| ProductOverlay::Path {
                                points: loop_
                                    .iter()
                                    .map(|p| Point3::new(p[0], p[1], p[2]))
                                    .collect(),
                                dashed: false,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                target.is_some()
            }
            PointerKind::Click if event.button == Some(PointerButton::Primary) => {
                let Some(target) = target else {
                    return false;
                };
                self.picker
                    .commit(&snapshot, event.pixel, &target, std::time::Instant::now());
                // The outline shows what a click would pick; after one it waits for the next move.
                self.pick_hover.clear();
                let mode = if event.modifiers.shift {
                    "add"
                } else if event.modifiers.command {
                    "toggle"
                } else {
                    "replace"
                };
                let mut intent = Intent::new(archbuilder_commands::structure::OBJECT_SELECT);
                intent.params = serde_json::json!({
                    "objects": [target.id.to_string()], "mode": mode, "gesture": "click"
                })
                .as_object()
                .cloned()
                .unwrap_or_default();
                cx.ask(intent);
                true
            }
            // A secondary click on an object that is not selected selects it, so the menu that
            // opens is the menu of what is under the pointer.
            PointerKind::Click if event.button == Some(PointerButton::Secondary) => {
                if let Some(t) = target.filter(|t| !cx.selection().contains(&t.id)) {
                    cx.ask(habits::select_intent(&[t.id], "replace"));
                }
                false
            }
            _ => false,
        }
    }

    /// The pointer while the floor tool runs. In the sketch mode it snaps like the wall tool (to
    /// wall corners and ends, the points of the sketch, tracking lines and the grid) and a click
    /// is the next point; in the walls mode it outlines what a click would pick and a click picks.
    fn floor_pointer(&mut self, cx: &InteractionContext<'_>, event: &PointerEvent) -> bool {
        let Some(state) = self.floors.state().cloned() else {
            return false;
        };
        let (sketch, points) = self.floors.progress();
        let walls = cx
            .document()
            .map(|d| archbuilder_model::floor::storey_walls(&d.snapshot(), state.storey))
            .unwrap_or_default();
        let at = if sketch {
            let baselines: Vec<[[f64; 2]; 2]> =
                walls.iter().map(|(_, w)| [w.start, w.end]).collect();
            let centers = self.room_centers(cx, state.storey);
            let mut ends = vec![[0.0, 0.0]];
            ends.extend(points.iter().copied());
            ends.extend(walls.iter().flat_map(|(_, w)| w.plain_footprint()));
            let hit = event.model.and_then(|m| {
                crate::snap::snap(&self.snap_input(
                    [m.x, m.y],
                    event,
                    &ends,
                    &baselines,
                    points.last().copied(),
                    &centers,
                ))
            });
            self.angle = hit.as_ref().and_then(|h| h.degrees.map(|d| (h.point, d)));
            self.guides = hit.as_ref().map_or_else(Vec::new, |h| h.guides.clone());
            self.snap = hit
                .as_ref()
                .and_then(|h| h.kind.marker().map(|kind| (kind, h.point)));
            hit.map(|h| h.point)
        } else {
            self.snap = None;
            self.guides.clear();
            self.angle = None;
            event.model.map(|m| [m.x, m.y])
        };
        if let Some(at) = at {
            self.cursor = Some(at);
        }
        match event.kind {
            PointerKind::Move => {
                self.floor_hover = match (sketch, at) {
                    (false, Some(at)) => Self::pick_outline(&walls, &points, at, state.elevation),
                    _ => Vec::new(),
                };
                false
            }
            PointerKind::Click if event.button == Some(PointerButton::Primary) => {
                if let Some(at) = at {
                    let _ = self.with_floors(cx, |tool, env, run| tool.click(env, run, at));
                    if let Some(error) = self.floors.take_error() {
                        self.notice_error(cx, &error);
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// The outline of what a click at `at` would pick: the wall under it, or the loop around it.
    fn pick_outline(
        walls: &[(ObjectId, archbuilder_model::wall::Wall)],
        picked: &[[f64; 2]],
        at: [f64; 2],
        z: f64,
    ) -> Vec<ProductOverlay> {
        let path = |points: &[[f64; 2]]| ProductOverlay::Path {
            points: points
                .iter()
                .chain(points.first())
                .map(|p| Point3::new(p[0], p[1], z))
                .collect(),
            dashed: true,
        };
        let by_id = |id: ObjectId| walls.iter().find(|(w, _)| *w == id).map(|(_, w)| *w);
        let mut out: Vec<ProductOverlay> = picked
            .iter()
            .filter_map(|p| archbuilder_model::floor::wall_at(walls, *p).and_then(by_id))
            .map(|w| path(&w.plain_footprint()))
            .collect();
        if let Some(wall) = archbuilder_model::floor::wall_at(walls, at).and_then(by_id) {
            out.push(path(&wall.plain_footprint()));
        } else if picked.is_empty()
            && let Some(ids) = archbuilder_model::floor::loop_around(walls, at)
        {
            let chain: Vec<_> = ids
                .iter()
                .filter_map(|id| walls.iter().find(|(w, _)| w == id).copied())
                .collect();
            if let Ok(vertices) = archbuilder_model::floor::vertices_of_walls(
                &chain,
                archbuilder_model::floor::Face::Interior,
                0.0,
            ) {
                let points: Vec<[f64; 2]> = vertices.iter().map(|v| v.point).collect();
                out.push(path(&points));
            }
        }
        out
    }

    /// The overlays of the floor tool: the outline of what a click would pick, and the sketch with
    /// its rubber band.
    fn floor_overlays(&self) -> Vec<ProductOverlay> {
        let (Some(state), true) = (self.floors.state(), self.floors.running()) else {
            return Vec::new();
        };
        let (sketch, points) = self.floors.progress();
        let mut out = self.floor_hover.clone();
        if sketch && !points.is_empty() {
            let z = state.elevation;
            let mut line: Vec<Point3> = points.iter().map(|p| Point3::new(p[0], p[1], z)).collect();
            if let Some(c) = self.cursor.filter(|_| !self.floors.band_off()) {
                line.push(Point3::new(c[0], c[1], z));
                if points.len() >= 2 {
                    line.push(Point3::new(points[0][0], points[0][1], z));
                }
            }
            out.push(ProductOverlay::Path {
                points: line,
                dashed: true,
            });
        }
        out
    }
}

/// The dashed outline and riser lines of the stair the stair tool would make on `storey`, at the
/// elevation `z` of its level.
fn stair_ghost(
    snapshot: &arc_doc::Snapshot,
    storey: ObjectId,
    z: f64,
    stair: &archbuilder_model::stair::Stair,
) -> Vec<ProductOverlay> {
    let Some(height) = spatial::get(snapshot, storey)
        .and_then(|s| spatial::level_above(snapshot, &s))
        .and_then(|l| {
            archbuilder_model::types::meters(&l.values, archbuilder_model::types::level::ELEVATION)
        })
        .map(|top| top - z)
    else {
        return Vec::new();
    };
    let Ok(layout) = archbuilder_model::stair::Layout::new(stair, height) else {
        return Vec::new();
    };
    let plan = layout.plan(archbuilder_model::stair::CUT_HEIGHT);
    let path = |points: Vec<[f64; 2]>| ProductOverlay::Path {
        points: points.iter().map(|p| Point3::new(p[0], p[1], z)).collect(),
        dashed: true,
    };
    let mut out: Vec<ProductOverlay> = plan
        .outline
        .iter()
        .map(|r| path(r.iter().chain(r.first()).copied().collect()))
        .collect();
    out.extend(plan.treads.iter().map(|(pair, _)| path(pair.to_vec())));
    out.push(path(plan.walk.clone()));
    out.push(path(plan.arrow.to_vec()));
    out
}

#[cfg(test)]
mod tests {
    use super::{MAX_NOTICES, export_base, export_line, push_bounded};

    #[test]
    fn notices_accumulate_in_order_and_the_oldest_go_beyond_the_bound() {
        let (mut notices, mut first) = (Vec::new(), 0);
        for n in 0..5 {
            push_bounded(&mut notices, &mut first, n.to_string());
        }
        // An export notice is still there after later lines, in order.
        assert_eq!(notices, ["0", "1", "2", "3", "4"]);
        for n in 5..MAX_NOTICES + 10 {
            push_bounded(&mut notices, &mut first, n.to_string());
        }
        assert_eq!((notices.len(), first), (MAX_NOTICES, 10));
        assert_eq!(notices.first().map(String::as_str), Some("10"));
    }
    use std::path::Path;

    #[test]
    fn a_relative_export_path_is_taken_in_the_folder_of_the_saved_document() {
        let saved = Path::new("plans").join("house.archb");
        assert_eq!(export_base(Some(&saved)), Path::new("plans"));
        // A document never saved, or a file name alone, falls back to the documents or home folder.
        assert_eq!(
            export_base(None),
            export_base(Some(Path::new("house.archb")))
        );
    }

    #[test]
    fn the_typed_export_line_names_its_path_or_none() {
        let line = |t: &str| export_line(t);
        assert_eq!(line("archbuilder.file.export_ifc"), Some(None));
        assert_eq!(
            line("archbuilder.file.export_ifc path=/tmp/a b.ifc"),
            Some(Some("/tmp/a b.ifc".to_owned()))
        );
        assert_eq!(
            line("archbuilder.file.export_ifc  path=\"C:\\x.ifc\""),
            Some(Some("C:\\x.ifc".to_owned()))
        );
        assert_eq!(line("archbuilder.file.export_ifc path="), Some(None));
        assert_eq!(line("archbuilder.file.export_ifcx"), None);
        assert_eq!(line("wall"), None);
    }
}
