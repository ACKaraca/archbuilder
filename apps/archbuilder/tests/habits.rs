//! The everyday habits without a window: Tab through the candidates under the pointer, hide and
//! isolate against picking, select all of a kind, the walls joined to a wall, the previous
//! selection, and the shortcut table through its file commands.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;
use std::time::Instant;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Outcome, Permission, Roots, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder::picker::{Picker, Probe};
use archbuilder::selection::{self, Memory};
use archbuilder::shortcuts::{Table, defaults, ui};
use archbuilder::visibility::{self, Visibility};
use archbuilder_model::spatial;
use archbuilder_model::types::{DOOR, FLOOR, WALL};
use serde_json::{Value, json};

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
}

impl Rig {
    fn new() -> Self {
        let document = Document::open(
            Snapshot::empty(Arc::new(archbuilder_commands::types().unwrap())),
            Arc::new(Evaluators::new()),
            DocumentConfig {
                workers: 0,
                ..DocumentConfig::default()
            },
        )
        .unwrap();
        archbuilder_commands::install(&document);
        let mut registry = archbuilder_commands::registry().unwrap();
        archbuilder_io::register(&mut registry).unwrap();
        let rig = Self {
            document,
            registry,
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
        };
        rig.run("archbuilder.project.new", json!({})).unwrap();
        rig
    }

    fn run(&self, command: &str, params: Value) -> arc_base::Result<Outcome> {
        Invoker::new(&self.registry).invoke(
            &self.document,
            &self.session,
            Invocation {
                seed: 5,
                ..Invocation::new(command, params)
            },
        )
    }

    /// A 6 x 4 room of four walls with a door in the first and a floor.
    fn room(&self) -> Vec<ObjectId> {
        let corners = [[0.0, 0.0], [6.0, 0.0], [6.0, 4.0], [0.0, 4.0]];
        let walls: Vec<ObjectId> = (0..4)
            .map(|k| {
                self.run(
                    "archbuilder.wall.create",
                    json!({"start": corners[k], "end": corners[(k + 1) % 4], "height": 3.0}),
                )
                .unwrap()
                .value["wall"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect();
        self.run(
            "archbuilder.door.place",
            json!({"wall": walls[0], "distance": 2.0}),
        )
        .unwrap();
        self.run("archbuilder.floor.create", json!({"picks": [[3.0, 2.0]]}))
            .unwrap();
        walls
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }
}

fn kind(snapshot: &Snapshot, id: ObjectId) -> String {
    spatial::type_of(snapshot, id).unwrap()
}

/// What is under a point just inside the first wall's inner face: the wall, then the floor.
const AT: [f64; 2] = [4.5, 0.12];
const APERTURE: f64 = 0.05;
const PIXEL: [f64; 2] = [100.0, 100.0];

fn target(
    picker: &mut Picker,
    snapshot: &Snapshot,
    pixel: [f64; 2],
    shown: &dyn Fn(ObjectId) -> bool,
) -> Option<archbuilder::picker::Target> {
    picker.target(
        snapshot,
        &Probe {
            pixel,
            at: AT,
            aperture: APERTURE,
            ray: None,
            now: Instant::now(),
            shown,
        },
    )
}

#[test]
fn tab_steps_the_hover_down_the_candidates_and_a_click_selects_what_the_hover_shows() {
    let rig = Rig::new();
    rig.room();
    let snapshot = rig.snapshot();
    let mut picker = Picker::default();
    let all = |_: ObjectId| true;
    let first = target(&mut picker, &snapshot, PIXEL, &all).unwrap();
    assert_eq!(kind(&snapshot, first.id), WALL);
    // Tab: the next candidate under the pointer is the floor; again, back to the wall.
    assert!(picker.step());
    let second = target(&mut picker, &snapshot, PIXEL, &all).unwrap();
    assert_eq!(kind(&snapshot, second.id), FLOOR);
    assert!(picker.step());
    assert_eq!(
        target(&mut picker, &snapshot, PIXEL, &all).unwrap().id,
        first.id
    );
    // A move of a few pixels keeps the choice; a move further away starts over at the top.
    assert!(picker.step());
    let near = target(&mut picker, &snapshot, [102.0, 101.0], &all).unwrap();
    assert_eq!(near.id, second.id);
    let far = target(&mut picker, &snapshot, [140.0, 100.0], &all).unwrap();
    assert_eq!(far.id, first.id);
    // A click after Tab selects the candidate the hover showed.
    assert!(picker.step());
    let shown = target(&mut picker, &snapshot, [140.0, 100.0], &all).unwrap();
    assert_eq!(kind(&snapshot, shown.id), FLOOR);
    picker.commit(&snapshot, [140.0, 100.0], &shown, Instant::now());
    // Nothing to step through where one object is under the pointer.
    let mut lone = Picker::default();
    let alone = lone
        .target(
            &snapshot,
            &Probe {
                pixel: PIXEL,
                at: [3.0, 2.0],
                aperture: APERTURE,
                ray: None,
                now: Instant::now(),
                shown: &all,
            },
        )
        .unwrap();
    assert_eq!(kind(&snapshot, alone.id), FLOOR);
    assert!(!lone.step());
}

#[test]
fn hidden_and_isolated_out_objects_are_not_picked_and_reset_brings_them_back() {
    let rig = Rig::new();
    let walls = rig.room();
    let snapshot = rig.snapshot();
    let mut vis = Visibility::default();
    let mut picker = Picker::default();
    let top = |vis: &Visibility, picker: &mut Picker| {
        target(picker, &snapshot, PIXEL, &|id| vis.shows(&snapshot, id))
            .map(|t| kind(&snapshot, t.id))
    };
    assert_eq!(top(&vis, &mut picker).as_deref(), Some(WALL));
    // Hiding the wall takes its door with it, and the floor is the candidate left.
    vis.hide(&snapshot, &walls[..1]);
    let door = spatial::objects(&snapshot, DOOR)[0].id;
    assert!(!vis.shows(&snapshot, walls[0]) && !vis.shows(&snapshot, door));
    assert!(vis.shows(&snapshot, walls[1]));
    assert_eq!(top(&vis, &mut picker).as_deref(), Some(FLOOR));
    let lists = vis.lists(&snapshot).clone();
    assert_eq!(lists.hidden.len(), 2);
    assert!(lists.isolated.is_none());
    // Isolating the floor leaves the floor only.
    let floor = spatial::objects(&snapshot, FLOOR)[0].id;
    let mut vis = Visibility::default();
    vis.isolate(&snapshot, &[floor]);
    assert_eq!(top(&vis, &mut picker).as_deref(), Some(FLOOR));
    assert!(!vis.shows(&snapshot, walls[2]));
    assert_eq!(vis.lists(&snapshot).isolated.as_deref(), Some(&[floor][..]));
    // Hiding a kind hides every wall and door.
    let mut vis = Visibility::default();
    vis.hide_kinds(&visibility::kinds_of(&snapshot, &walls[..1]));
    assert!(walls.iter().all(|w| !vis.shows(&snapshot, *w)) && vis.shows(&snapshot, floor));
    // Reset shows everything again and picks the wall first.
    vis.reset();
    assert!(!vis.active());
    assert_eq!(top(&vis, &mut picker).as_deref(), Some(WALL));
    assert_eq!(vis.lists(&snapshot).hidden.len(), 0);
}

#[test]
fn select_all_of_a_kind_the_filter_the_chain_and_the_previous_selection() {
    let rig = Rig::new();
    let walls = rig.room();
    let snapshot = rig.snapshot();
    let vis = Visibility::default();
    let door = spatial::objects(&snapshot, DOOR)[0].id;
    // The kind of one wall is all four walls; of a wall and the door, walls and the door.
    let of_kind = selection::all_of_kinds_in(&snapshot, &walls[..1], &vis);
    assert_eq!(of_kind.len(), 4);
    assert!(of_kind.iter().all(|id| kind(&snapshot, *id) == WALL));
    let mixed = selection::all_of_kinds_in(&snapshot, &[walls[0], door], &vis);
    assert_eq!(mixed.len(), 5);
    assert_eq!(selection::all(&snapshot, &vis).len(), 6);
    // The filter keeps one kind of a mixed selection, with counts to offer it by.
    let sel = [walls[0], walls[1], door];
    assert_eq!(
        selection::kind_counts(&snapshot, &sel),
        [(WALL, 2), (DOOR, 1)]
    );
    assert_eq!(selection::only(&snapshot, &sel, DOOR), [door]);
    // Hidden objects are skipped.
    let mut hiding = Visibility::default();
    hiding.hide(&snapshot, &walls[..1]);
    assert_eq!(
        selection::all_of_kinds_in(&snapshot, &walls[..1], &hiding).len(),
        3
    );
    // A double click on a wall selects the walls joined to it: a lone wall is its own chain.
    assert_eq!(selection::wall_chain(&snapshot, walls[0], &vis).len(), 4);
    rig.run(
        "archbuilder.wall.create",
        json!({"start": [20.0, 0.0], "end": [24.0, 0.0]}),
    )
    .unwrap();
    let snapshot = rig.snapshot();
    let lone = spatial::objects(&snapshot, WALL)
        .iter()
        .map(|o| o.id)
        .find(|id| !walls.contains(id))
        .unwrap();
    assert_eq!(selection::wall_chain(&snapshot, lone, &vis), [lone]);
    assert_eq!(selection::wall_chain(&snapshot, walls[2], &vis).len(), 4);
    // The base of the clipboard is the lower left corner of what was copied.
    let base = selection::base_point(&snapshot, &walls).unwrap();
    assert!(
        (base[0] + 0.1).abs() < 1e-9 && (base[1] + 0.1).abs() < 1e-9,
        "{base:?}"
    );
    // The previous selection comes back, and an empty selection never replaces it.
    let mut memory = Memory::default();
    memory.observe(&[walls[0]]);
    memory.observe(&[walls[1], walls[2]]);
    assert_eq!(memory.previous(&snapshot), [walls[0]]);
    memory.observe(&[]);
    assert_eq!(memory.previous(&snapshot), [walls[1], walls[2]]);
}

#[test]
fn the_default_table_names_only_commands_that_exist_and_a_custom_table_goes_through_a_file() {
    let rig = Rig::new();
    let interface = [
        ui::REPEAT,
        ui::SELECT_ALL,
        ui::SELECT_KIND,
        ui::SELECT_PREVIOUS,
        ui::COPY,
        ui::CUT,
        ui::HIDE,
        ui::ISOLATE,
        ui::HIDE_KIND,
        ui::ISOLATE_KIND,
        ui::RESET_VISIBILITY,
        ui::ZOOM_EXTENTS,
        ui::SNAP,
        arc_app::commands::EDIT_UNDO,
    ];
    let known = |c: &str| interface.contains(&c) || rig.registry.get(c).is_some();
    for (keys, command) in defaults() {
        assert!(known(command), "{keys} {command}");
    }
    // A customized table is written and read back through the registered commands, which reach
    // files only inside the session's roots.
    let dir = std::env::temp_dir().join(format!("ab-habits-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut session = Session::new(
        Surface::Ui,
        [
            Permission::Read,
            Permission::FileRead,
            Permission::FileWrite,
        ],
    );
    session.roots = Roots::new([&dir], [&dir]).unwrap();
    let invoke = |command: &str, params: Value| {
        Invoker::new(&rig.registry).invoke(
            &rig.document,
            &session,
            Invocation::new(command, params),
        )
    };
    let table = Table::with_overrides("ww=archbuilder.wall.create;wa=");
    let path = dir.join("keys.json");
    invoke(
        "archbuilder.shortcuts.export",
        json!({"path": path.to_string_lossy(), "text": table.to_json()}),
    )
    .unwrap();
    let read = invoke(
        "archbuilder.shortcuts.import",
        json!({"path": path.to_string_lossy()}),
    )
    .unwrap();
    let back = Table::from_json(read.value["text"].as_str().unwrap(), &known).unwrap();
    assert_eq!(back, table);
    assert_eq!(back.command("WW"), Some("archbuilder.wall.create"));
    assert_eq!(back.command("WA"), None);
    // A path outside the roots is refused.
    let outside = std::env::temp_dir().join("ab-habits-outside.json");
    let denied = invoke(
        "archbuilder.shortcuts.export",
        json!({"path": outside.to_string_lossy(), "text": "x"}),
    );
    assert!(denied.is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_clipboard_is_one_for_every_document_of_the_application() {
    // Copy in document A, paste in document B (another tab).
    let a = Rig::new();
    a.room();
    let s = a.snapshot();
    let ids: Vec<ObjectId> = [WALL, FLOOR, DOOR]
        .iter()
        .flat_map(|k| spatial::objects(&s, k))
        .map(|o| o.id)
        .collect();
    let content = archbuilder_commands::clip::capture(&s, &ids).unwrap();
    archbuilder::clipboard::set(content, [0.0, 0.0]);
    drop(a);
    let b = Rig::new();
    let (content, base) = archbuilder::clipboard::get().expect("the clipboard is shared");
    assert_eq!(base, [0.0, 0.0]);
    let sb = b.snapshot();
    let storey = spatial::storeys(&sb, spatial::buildings(&sb)[0].id)[0]
        .storey
        .id;
    b.run(
        "archbuilder.clipboard.paste",
        json!({"content": content, "storey": storey}),
    )
    .unwrap();
    assert_eq!(spatial::objects(&b.snapshot(), WALL).len(), 4);
}
