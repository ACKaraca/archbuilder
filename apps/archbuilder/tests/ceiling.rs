//! The ceiling tool headless: a click inside a room makes a ceiling (one undo step); a document from
//! before ceilings, openings and shafts opens, gets them, saves and opens again.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_app::application::{AppConfig, Application};
use arc_app::recent::RecentFiles;
use arc_base::{IdGenerator, Limits};
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::package::LockOwner;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder::floor_tool::FloorTool;
use archbuilder::wall_tool::Env;
use archbuilder_model::spatial;
use archbuilder_model::types::{
    CEILING, CEILING_TYPE, SHAFT, VERTICAL_OPENING, WALL, WALL_OPENING,
};
use archbuilder_model::units::LengthUnit;
use serde_json::json;

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
    tool: FloorTool,
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
        let mut session = Session::new(Surface::CommandLine, [Permission::Read, Permission::Edit]);
        session.units = vec![*LengthUnit::Meter.unit().unwrap()];
        let rig = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
            session,
            tool: FloorTool::new(),
            seed: 1,
        };
        rig.run("archbuilder.project.new", json!({}));
        let c = [[0.0, 0.0], [0.0, 6.0], [10.0, 6.0], [10.0, 0.0]];
        for k in 0..4 {
            rig.run(
                "archbuilder.wall.create",
                json!({"start": c[k], "end": c[(k + 1) % 4]}),
            );
        }
        rig
    }

    fn run(&self, command: &str, params: serde_json::Value) -> serde_json::Value {
        Invoker::new(&self.registry)
            .invoke(
                &self.document,
                &self.session,
                Invocation {
                    seed: 99,
                    ..Invocation::new(command, params)
                },
            )
            .unwrap()
            .value
    }

    fn step(
        &mut self,
        f: impl FnOnce(
            &mut FloorTool,
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
        let (document, session) = (&self.document, &self.session);
        f(&mut self.tool, &env, &mut |inv| {
            invoker.invoke(document, session, inv)
        })
        .unwrap();
    }
}

#[test]
fn a_click_inside_a_room_makes_a_ceiling_in_one_undo_step() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| {
        t.begin_for(
            env,
            run,
            archbuilder_commands::ceiling::CEILING_CREATE,
            None,
        )
    });
    assert!(rig.tool.running());
    let prompt = rig.tool.line().active().unwrap().prompt().unwrap().clone();
    assert_eq!(prompt.message.id, "ab-prompt-ceiling-pick");
    rig.step(|t, env, run| t.click(env, run, [5.0, 3.0]));
    let snapshot = rig.document.snapshot();
    let ceilings = spatial::objects(&snapshot, CEILING);
    assert_eq!(ceilings.len(), 1);
    let d = archbuilder_model::ceiling::derived(&snapshot, &ceilings[0]).unwrap();
    // The default height is 2.6 m and the boundary is the room's region: 9.8 x 5.8 m.
    assert!((d.plane.z - 2.6).abs() < 1e-9);
    assert!((d.area() - 9.8 * 5.8).abs() < 1e-9);
    // The tool asks for the next ceiling; undo takes it back in one step.
    assert!(rig.tool.running());
    rig.tool.stop();
    rig.document.undo().wait().unwrap();
    assert!(spatial::objects(&rig.document.snapshot(), CEILING).is_empty());
}

#[test]
fn the_shaft_and_vertical_opening_tools_take_clicked_points() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| {
        t.begin_for(env, run, archbuilder_commands::opening::SHAFT_CREATE, None)
    });
    for p in [[2.0, 2.0], [3.0, 2.0], [3.0, 3.0]] {
        rig.step(|t, env, run| t.click(env, run, p));
    }
    rig.step(|t, env, run| t.click(env, run, [2.0, 2.0]));
    let snapshot = rig.document.snapshot();
    assert_eq!(spatial::objects(&snapshot, SHAFT).len(), 1);
    // A floor to host a vertical opening, cut by clicks too.
    rig.step(|t, env, run| t.begin(env, run, None));
    rig.step(|t, env, run| t.click(env, run, [5.0, 3.0]));
    rig.tool.stop();
    let floor = spatial::objects(&rig.document.snapshot(), archbuilder_model::types::FLOOR)[0].id;
    rig.step(|t, env, run| {
        t.begin_for(
            env,
            run,
            archbuilder_commands::opening::VERTICAL_OPENING_CREATE,
            Some(floor),
        )
    });
    for p in [[6.0, 1.0], [7.0, 1.0], [7.0, 2.0]] {
        rig.step(|t, env, run| t.click(env, run, p));
    }
    rig.step(|t, env, run| t.click(env, run, [6.0, 1.0]));
    assert_eq!(
        spatial::objects(&rig.document.snapshot(), VERTICAL_OPENING).len(),
        1
    );
}

#[test]
fn a_new_ceiling_sketch_takes_an_arc_and_a_shaft_asks_for_its_levels() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| {
        t.begin_for(
            env,
            run,
            archbuilder_commands::ceiling::CEILING_CREATE,
            None,
        )
    });
    rig.step(|t, env, run| t.text(env, run, "sketch"));
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    rig.step(|t, env, run| t.text(env, run, "arc"));
    rig.step(|t, env, run| t.click(env, run, [3.0, 0.5]));
    rig.step(|t, env, run| t.click(env, run, [5.0, 1.0]));
    rig.step(|t, env, run| t.click(env, run, [5.0, 4.0]));
    rig.step(|t, env, run| t.click(env, run, [1.0, 4.0]));
    rig.step(|t, env, run| t.text(env, run, "close"));
    let s = rig.document.snapshot();
    let c = &spatial::objects(&s, CEILING)[0];
    let d = archbuilder_model::ceiling::derived(&s, c).unwrap();
    assert!(d.boundary.has_arcs());
    rig.tool.stop();
    rig.step(|t, env, run| {
        t.begin_for(env, run, archbuilder_commands::opening::SHAFT_CREATE, None)
    });
    rig.step(|t, env, run| t.text(env, run, "top"));
    rig.step(|t, env, run| t.text(env, run, "Floor 2"));
    rig.step(|t, env, run| t.text(env, run, "offset"));
    rig.step(|t, env, run| t.text(env, run, "1.5"));
    for p in [[7.0, 2.0], [8.0, 2.0], [8.0, 3.0]] {
        rig.step(|t, env, run| t.click(env, run, p));
    }
    rig.step(|t, env, run| t.text(env, run, "close"));
    let s = rig.document.snapshot();
    let span = archbuilder_model::opening::shaft_span(&s, &spatial::objects(&s, SHAFT)[0]).unwrap();
    assert!((span.z.1 - 7.5).abs() < 1e-9, "{:?}", span.z);
}

#[test]
fn a_document_from_before_ceilings_gets_them_openings_and_a_shaft_and_round_trips() {
    let dir = std::env::temp_dir().join(format!("archbuilder-ceilings-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let old = dir.join("pre-types.archb");
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/pre-types.archb"
        ),
        &old,
    )
    .unwrap();
    let mut first = app(&dir);
    let tab = first.open(&old).unwrap();
    let snapshot = first.tab(tab).unwrap().document().snapshot();
    // The old file has none of the new kinds and loads as it was.
    for key in [CEILING, WALL_OPENING, SHAFT, VERTICAL_OPENING, CEILING_TYPE] {
        assert!(spatial::objects(&snapshot, key).is_empty(), "{key}");
    }
    assert!(!spatial::objects(&snapshot, WALL).is_empty());
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    if let Some(invocation) = archbuilder::interaction::upgrade_invocation(&snapshot) {
        first.invoke(tab, &session, invocation).unwrap();
    }
    let walls = spatial::objects(&snapshot, WALL);
    let n = walls.len() as f64;
    let middle = walls.iter().fold([0.0, 0.0], |m, o| {
        let w = archbuilder_model::wall::Wall::from_values(&o.values).unwrap();
        [
            m[0] + (w.start[0] + w.end[0]) / 2.0 / n,
            m[1] + (w.start[1] + w.end[1]) / 2.0 / n,
        ]
    });
    let run = |command: &str, params: serde_json::Value| {
        first
            .invoke(tab, &session, Invocation::new(command, params))
            .unwrap()
            .value
    };
    let made = run("archbuilder.ceiling.create", json!({"picks": [middle]}));
    let ceiling = made["ceiling"].as_str().unwrap().to_owned();
    run(
        "archbuilder.opening.place",
        json!({"wall": walls[0].id.to_string(), "distance": 1.0, "width": 0.8, "height": 1.0, "rise": 0.3}),
    );
    run(
        "archbuilder.shaft.create",
        json!({"points": [[middle[0], middle[1]], [middle[0] + 0.5, middle[1]], [middle[0] + 0.5, middle[1] + 0.5]]}),
    );
    let new = dir.join("with-ceiling.archb");
    first.save_as(tab, &new).unwrap();
    drop(first);
    let mut second = app(&dir);
    let tab = second.open(&new).unwrap();
    let reopened = second.tab(tab).unwrap().document().snapshot();
    let ceilings = spatial::objects(&reopened, CEILING);
    assert_eq!(ceilings.len(), 1);
    assert_eq!(ceilings[0].id.to_string(), ceiling);
    assert!(archbuilder_model::ceiling::derived(&reopened, &ceilings[0]).is_some());
    assert_eq!(spatial::objects(&reopened, WALL_OPENING).len(), 1);
    assert_eq!(spatial::objects(&reopened, SHAFT).len(), 1);
    // The default ceiling types came with the first ceiling.
    assert_eq!(spatial::objects(&reopened, CEILING_TYPE).len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

fn app(dir: &std::path::Path) -> Application {
    let owner = LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "archbuilder tests".into(),
        time_unix_ms: 0,
    };
    let product = archbuilder::product(Arc::new(arc_command::FixedClock(0)), owner).unwrap();
    let mut next = 70;
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
