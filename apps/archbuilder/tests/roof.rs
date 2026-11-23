//! The roof tool headless: a click inside a closed loop of walls makes a roof (one undo step),
//! the prompt names the slope; a document from before roofs opens, gets a roof with walls
//! attached to it, saves and opens again.

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
use archbuilder_model::types::{ROOF, ROOF_TYPE, WALL};
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
fn a_click_inside_the_walls_makes_a_roof_in_one_undo_step() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin_roof(env, run));
    assert!(rig.tool.running());
    let prompt = rig.tool.line().active().unwrap().prompt().unwrap().clone();
    assert_eq!(prompt.message.id, "ab-prompt-roof-pick");
    rig.step(|t, env, run| t.click(env, run, [5.0, 3.0]));
    let snapshot = rig.document.snapshot();
    let roofs = spatial::objects(&snapshot, ROOF);
    assert_eq!(roofs.len(), 1);
    let d = archbuilder_model::roof::Derived::of(&snapshot, &roofs[0]).unwrap();
    // The eaves sit on the walls (3 m); the edges follow the walls' exterior faces.
    assert!((d.surface.base - 3.0).abs() < 1e-9);
    assert_eq!(d.roof.walls().len(), 4);
    // The tool asks for the next roof; undo takes the roof back in one step.
    assert!(rig.tool.running());
    rig.tool.stop();
    rig.document.undo().wait().unwrap();
    assert!(spatial::objects(&rig.document.snapshot(), ROOF).is_empty());
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

#[test]
fn a_document_from_before_roofs_gets_a_roof_and_attached_walls_and_round_trips() {
    let dir = std::env::temp_dir().join(format!("archbuilder-roofs-{}", std::process::id()));
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
    assert!(spatial::objects(&snapshot, ROOF_TYPE).is_empty());
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
    let made = first
        .invoke(
            tab,
            &session,
            Invocation::new("archbuilder.roof.create", json!({"picks": [middle]})),
        )
        .unwrap()
        .value;
    let roof = made["roof"].as_str().unwrap().to_owned();
    let ids: Vec<String> = walls.iter().map(|w| w.id.to_string()).collect();
    first
        .invoke(
            tab,
            &session,
            Invocation::new("archbuilder.wall.attach_top", json!({"walls": ids})),
        )
        .unwrap();
    let snapshot = first.tab(tab).unwrap().document().snapshot();
    // The default roof types came with the first roof.
    assert_eq!(spatial::objects(&snapshot, ROOF_TYPE).len(), 2);
    let new = dir.join("with-roof.archb");
    first.save_as(tab, &new).unwrap();
    drop(first);
    let mut second = app(&dir);
    let tab = second.open(&new).unwrap();
    let reopened = second.tab(tab).unwrap().document().snapshot();
    let roofs = spatial::objects(&reopened, ROOF);
    assert_eq!(roofs.len(), 1);
    assert_eq!(roofs[0].id.to_string(), roof);
    assert!(archbuilder_model::roof::Derived::of(&reopened, &roofs[0]).is_ok());
    for wall in spatial::objects(&reopened, WALL) {
        assert_eq!(
            archbuilder_model::wall::attachment(&wall.values).map(|id| id.to_string()),
            Some(roof.clone())
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
