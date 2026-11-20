//! The wall tool and the application's handling of walls, headless: a chain of three walls closed
//! by a fourth with typed points and clicks (one undo step per wall), the options carried along the
//! chain, the keyword `close`, a wall created, undone, redone, saved and reopened with identical
//! content, the property command on a wall, and the shaded extraction of a wall.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_app::application::{AppConfig, Application};
use arc_app::recent::RecentFiles;
use arc_base::{Cancel, IdGenerator, Limits};
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::package::LockOwner;
use arc_doc::{Document, DocumentConfig, Snapshot};
use arc_gpu::RenderItem;
use arc_gpu::extract::Extractor;
use archbuilder::wall_tool::{Env, Tool};
use archbuilder_model::spatial;
use archbuilder_model::types::{WALL, text};
use archbuilder_model::units::LengthUnit;
use archbuilder_model::wall::Wall;
use serde_json::json;

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
    tool: Tool,
    seed: u64,
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
        let registry = archbuilder_commands::registry().unwrap();
        let mut session = Session::new(Surface::CommandLine, [Permission::Read, Permission::Edit]);
        session.units = vec![*LengthUnit::Meter.unit().unwrap()];
        let rig = Self {
            document,
            registry,
            session,
            tool: Tool::new(),
            seed: 1,
        };
        rig.invoke("archbuilder.project.new", json!({}));
        rig
    }

    fn invoke(&self, command: &str, params: serde_json::Value) {
        Invoker::new(&self.registry)
            .invoke(
                &self.document,
                &self.session,
                Invocation {
                    seed: 99,
                    ..Invocation::new(command, params)
                },
            )
            .unwrap();
    }

    /// One tool step with an invoker as the runner.
    fn step(
        &mut self,
        f: impl FnOnce(
            &mut Tool,
            &Env<'_>,
            &mut dyn FnMut(Invocation) -> arc_base::Result<arc_command::Outcome>,
        ) -> arc_base::Result<()>,
    ) {
        self.seed += 1;
        let invoker = Invoker::new(&self.registry);
        let env = Env {
            registry: &self.registry,
            document: &self.document,
            session: &self.session,
            selection: &[],
            seed: self.seed,
        };
        let document = &self.document;
        let session = &self.session;
        f(&mut self.tool, &env, &mut |inv| {
            invoker.invoke(document, session, inv)
        })
        .unwrap();
    }

    fn text(&mut self, text: &str) {
        self.step(|t, env, run| t.text(env, run, text));
    }

    fn click(&mut self, at: [f64; 2]) {
        self.step(|t, env, run| t.click(env, run, at));
    }

    /// The walls, in creation order of their names, as `(name, start, end)`.
    fn walls(&self) -> Vec<(String, [f64; 2], [f64; 2])> {
        let mut found: Vec<_> = spatial::objects(&self.document.snapshot(), WALL)
            .iter()
            .map(|w| {
                let d = Wall::from_values(&w.values).unwrap();
                (text(&w.values, 0).unwrap().to_owned(), d.start, d.end)
            })
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found
    }
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

#[test]
fn a_chain_of_three_walls_and_a_closing_wall_is_four_walls_and_four_undo_steps() {
    let mut rig = Rig::new();
    rig.text("archbuilder.wall.create");
    assert!(rig.tool.active(), "the start point is asked");
    rig.text("0,0");
    rig.text("4,0");
    // The first wall is placed and the chain continues from its end, with relative and polar input.
    assert_eq!(rig.walls().len(), 1);
    rig.text("@0,3");
    rig.click([0.0, 3.0]);
    assert_eq!(rig.walls().len(), 3);
    // Two or more walls are placed: the keyword closes the chain at its start.
    rig.text("c");
    let walls = rig.walls();
    assert_eq!(walls.len(), 4);
    assert!(near(walls[3].2, [0.0, 0.0]), "{walls:?}");
    for pair in walls.windows(2) {
        assert!(
            near(pair[0].2, pair[1].1),
            "each wall starts where the last ended"
        );
    }
    assert!(
        !rig.tool.active() && rig.tool.chain().is_none(),
        "the chain is over"
    );
    for left in (0..4).rev() {
        assert!(rig.document.undo().wait().unwrap().value);
        assert_eq!(rig.walls().len(), left, "one undo step per wall");
    }
}

#[test]
fn options_carry_along_the_chain_and_enter_or_escape_end_it_keeping_the_placed_walls() {
    let mut rig = Rig::new();
    rig.text("wall");
    rig.text("t");
    rig.text("0.3");
    rig.text("l");
    rig.text("exterior");
    rig.text("0,0");
    rig.text("5,0");
    rig.text("5,2");
    // Enter on the prompt ends the chain; the two walls stay.
    rig.text("");
    assert!(rig.tool.chain().is_none());
    let snapshot = rig.document.snapshot();
    let walls = spatial::objects(&snapshot, WALL);
    assert_eq!(walls.len(), 2);
    for wall in &walls {
        let d = Wall::from_values(&wall.values).unwrap();
        assert_eq!(d.thickness, 0.3);
        assert_eq!(d.location, archbuilder_model::wall::LocationLine::Exterior);
    }
    // Escape drops the segment in progress and keeps the walls.
    rig.text("wall");
    rig.text("0,5");
    rig.tool.stop();
    assert_eq!(rig.walls().len(), 2);
    assert!(!rig.tool.active());
    // A zero-length segment is rejected at the prompt and the prompt stays.
    rig.text("wall");
    rig.text("1,1");
    rig.text("1,1");
    assert!(rig.tool.active() && rig.tool.take_error().is_some());
    assert_eq!(rig.walls().len(), 2);
}

#[test]
fn enter_at_a_value_prompt_keeps_the_value_a_length_follows_the_pointer_and_escape_leaves_in_two_steps()
 {
    let mut rig = Rig::new();
    rig.text("wall");
    rig.text("0,0");
    rig.text("t");
    // Enter keeps the thickness in effect and returns to the point prompt: the chain goes on.
    rig.text("");
    assert!(rig.tool.active() && rig.tool.chain().is_some());
    // A bare number is a distance along the direction to the pointer.
    rig.step(|tool, _, _| {
        tool.aim([3.0, 0.0]);
        Ok(())
    });
    rig.text("2.5");
    let walls = rig.walls();
    assert_eq!(walls.len(), 1);
    assert!(near(walls[0].2, [2.5, 0.0]), "{walls:?}");
    // The first Escape drops the segment and waits for a start point; the second ends the tool.
    rig.step(|tool, env, run| tool.escape(env, run));
    assert!(rig.tool.active() && rig.tool.chain().is_some_and(|c| c.walls == 0));
    rig.step(|tool, env, run| tool.escape(env, run));
    assert!(!rig.tool.active());
    assert_eq!(rig.walls().len(), 1);
}

#[test]
fn flip_reverses_the_baseline_and_keeps_the_location_line_during_placement() {
    let mut rig = Rig::new();
    rig.text("wall");
    rig.text("l");
    rig.text("exterior");
    rig.text("0,0");
    rig.text("f");
    rig.text("4,0");
    let id = spatial::objects(&rig.document.snapshot(), WALL)[0].clone();
    let wall = Wall::from_values(&id.values).unwrap();
    assert_eq!(
        wall.location,
        archbuilder_model::wall::LocationLine::Exterior
    );
    assert_eq!((wall.start, wall.end), ([4.0, 0.0], [0.0, 0.0]));
    // The chain goes on from the end the user gave, and stays flipped.
    rig.text("4,3");
    assert_eq!(rig.walls().len(), 2);
}

#[test]
fn a_pointer_a_little_off_an_axis_snaps_to_it_so_a_typed_length_is_exact() {
    // 0.4 degrees off the x axis, as the pointer would be.
    let (to, degrees) =
        archbuilder::interaction::snap_direction([0.0, 0.0], [9.9998, 0.0705]).unwrap();
    assert_eq!((to, degrees), ([10.0, 0.0], 0));
    // Far from a 15 degree step it does not snap.
    assert!(archbuilder::interaction::snap_direction([0.0, 0.0], [10.0, 1.3]).is_none());
    let mut rig = Rig::new();
    rig.text("wall");
    rig.text("0,0");
    rig.step(|tool, _, _| {
        tool.aim(to);
        Ok(())
    });
    rig.text("10");
    assert_eq!(rig.walls()[0].2, [10.0, 0.0]);
}

fn app(dir: &std::path::Path, seed: u64) -> Application {
    let owner = LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "archbuilder tests".into(),
        time_unix_ms: 0,
    };
    let product = archbuilder::product(Arc::new(arc_command::FixedClock(0)), owner).unwrap();
    let mut next = seed;
    Application::new(
        product,
        AppConfig {
            data_dir: dir.to_path_buf(),
            limits: Limits::DEFAULT,
            id_generators: Box::new(move || {
                next += 1;
                IdGenerator::seeded(next)
            }),
            recent: RecentFiles::new(4),
        },
    )
}

fn dump(snapshot: &Snapshot) -> Vec<String> {
    let mut lines: Vec<String> = snapshot
        .iter()
        .map(|(_, o)| format!("{} {:?} {:?}", o.id, o.header.owner, o.values))
        .collect();
    lines.sort();
    lines
}

#[test]
fn walls_survive_undo_redo_save_and_reopen_unchanged_and_the_property_command_edits_them() {
    let dir = std::env::temp_dir().join(format!("archbuilder-walls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut first = app(&dir, 0);
    let tab = first.new_document().unwrap();
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    let run = |app: &Application, command: &str, params: serde_json::Value| {
        app.invoke(
            tab,
            &session,
            Invocation {
                seed: 5,
                ..Invocation::new(command, params)
            },
        )
        .unwrap()
    };
    let made = run(
        &first,
        "archbuilder.wall.create",
        json!({"start": [0.0, 0.0], "end": [4.0, 0.0], "thickness": "250 mm"}),
    );
    let id = made.value["wall"].as_str().unwrap().to_owned();
    // The property grid's command changes the thickness and refuses a hostile value.
    run(
        &first,
        "archbuilder.property.set",
        json!({"objects": [id], "field": "thickness", "value": "0.3 m"}),
    );
    let refused = first.invoke(
        tab,
        &session,
        Invocation::new(
            "archbuilder.property.set",
            json!({"objects": [id], "field": "end_x", "value": 0.0}),
        ),
    );
    assert_eq!(refused.unwrap_err().code(), "ab.model.wall_too_short");
    run(
        &first,
        "archbuilder.wall.create",
        json!({"start": [4.0, 0.0], "end": [4.0, 3.0], "location_line": "interior"}),
    );
    let before = dump(&first.tab(tab).unwrap().document().snapshot());
    assert!(first.undo(tab).unwrap());
    assert_eq!(
        spatial::objects(&first.tab(tab).unwrap().document().snapshot(), WALL).len(),
        1
    );
    assert!(first.redo(tab).unwrap());
    assert_eq!(dump(&first.tab(tab).unwrap().document().snapshot()), before);
    let file = dir.join("walls.archb");
    first.save_as(tab, &file).unwrap();
    first
        .close(tab, |_| arc_app::application::CloseAnswer::Discard)
        .unwrap();
    let mut second = app(&dir, 100);
    let reopened = second.open(&file).unwrap();
    let snapshot = second.tab(reopened).unwrap().document().snapshot();
    assert_eq!(dump(&snapshot), before);

    // The viewport's extraction of a wall: one shaded mesh of the solid, relative to its start.
    let extractor = *second
        .product()
        .extractors()
        .iter()
        .find(|e| e.id == "archbuilder.wall")
        .unwrap()
        .factory
        .create()
        .downcast::<Extractor>()
        .unwrap();
    for wall in spatial::objects(&snapshot, WALL) {
        let extraction = extractor(&snapshot, &wall, &Cancel::new()).unwrap();
        assert!(matches!(extraction.items.as_slice(), [RenderItem::Mesh(_)]));
        assert_eq!(extraction.geometries[0].0.triangle_count(), 12);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_of_a_level_extracts_the_walls_of_its_storey_again() {
    use arc_doc::events::{ChangeGroups, Changes};
    use arc_doc::schema::TypeKey;
    let rig = Rig::new();
    rig.invoke(
        "archbuilder.wall.create",
        json!({"start": [0.0, 0.0], "end": [4.0, 0.0]}),
    );
    let snapshot = rig.document.snapshot();
    let wall = spatial::objects(&snapshot, WALL)[0].id;
    let storey = spatial::lowest_storey(&snapshot).unwrap();
    let level = spatial::level_of(&snapshot, &storey).unwrap().id;
    let modified = |key: &str, id| {
        Changes::Listed(
            [(
                TypeKey::new(key).unwrap(),
                ChangeGroups {
                    modified: [id].into(),
                    ..ChangeGroups::default()
                },
            )]
            .into(),
        )
    };
    let dependents = archbuilder::extract_wall::dependents;
    assert_eq!(
        dependents(&snapshot, &modified("archbuilder.level", level)),
        [wall]
    );
    assert_eq!(
        dependents(&snapshot, &modified("archbuilder.storey", storey.id)),
        [wall]
    );
    // "Everything" needs no help.
    assert!(dependents(&snapshot, &Changes::All).is_empty());
}

#[test]
fn a_changed_or_deleted_wall_extracts_the_walls_it_joins_again() {
    use arc_doc::events::{ChangeGroups, Changes};
    use arc_doc::schema::TypeKey;
    let rig = Rig::new();
    for (a, b) in [([0.0, 0.0], [4.0, 0.0]), ([4.0, 0.0], [4.0, 3.0])] {
        rig.invoke("archbuilder.wall.create", json!({"start": a, "end": b}));
    }
    let snapshot = rig.document.snapshot();
    let ids: Vec<_> = spatial::objects(&snapshot, WALL)
        .iter()
        .map(|w| w.id)
        .collect();
    let group =
        |g: ChangeGroups| Changes::Listed([(TypeKey::new("archbuilder.wall").unwrap(), g)].into());
    let moved = group(ChangeGroups {
        modified: [ids[0]].into(),
        ..ChangeGroups::default()
    });
    let mut found = archbuilder::extract_wall::dependents(&snapshot, &moved);
    found.sort();
    let mut both = ids.clone();
    both.sort();
    assert_eq!(found, both);
    // A deleted wall names no storey, yet its neighbors are extracted again.
    let gone = group(ChangeGroups {
        deleted: [{
            use arc_base::id::Identifier as _;
            arc_base::ObjectId::from_u128(99)
        }]
        .into(),
        ..ChangeGroups::default()
    });
    let mut found = archbuilder::extract_wall::dependents(&snapshot, &gone);
    found.sort();
    assert_eq!(found, both);
}

#[test]
fn every_prompt_and_menu_message_of_the_wall_is_in_both_catalogs() {
    let ids = [
        "ab-prompt-wall-start",
        "ab-prompt-wall-end",
        "ab-prompt-wall-thickness",
        "ab-prompt-wall-height",
        "ab-prompt-wall-location",
        "ab-keyword-wall-close",
        "ab-cmd-object-delete-label",
        "ab-tool-new",
        "ab-tool-wall",
        "ab-kind-wall",
        "ab-prop-thickness",
        "ab-loc-exterior",
    ];
    for language in ["en-US", "tr-TR"] {
        let localizer = archbuilder::localizer(&[language.to_owned()]).unwrap();
        for id in ids {
            let text = localizer.format(id, &[]);
            assert_eq!(
                text.locale.as_ref().map(ToString::to_string).as_deref(),
                Some(language),
                "{id} in {language}"
            );
        }
    }
}
