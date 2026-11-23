//! The insert tool and the application's handling of doors and windows, headless: inserts typed
//! and clicked in a row (one undo step each), the pointer's snapping along a wall, a refused click
//! that leaves the chain going, the shaded extraction of an insert (the glass apart from the frame),
//! the walls and inserts that a change makes extract again, and the messages in both catalogs.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder::insert_tool::InsertTool;
use archbuilder::wall_tool::Env;
use archbuilder_model::insert::Kind;
use archbuilder_model::spatial;
use archbuilder_model::types::{WALL, WALL_OPENING};
use archbuilder_model::units::LengthUnit;
use serde_json::json;

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
    tool: InsertTool,
    selection: Vec<ObjectId>,
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
        let mut rig = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
            session,
            tool: InsertTool::new(),
            selection: Vec::new(),
            seed: 1,
        };
        rig.invoke("archbuilder.project.new", json!({}));
        rig.invoke(
            "archbuilder.wall.create",
            json!({"start": [0.0, 0.0], "end": [10.0, 0.0], "height": 3.0}),
        );
        rig.selection = vec![rig.wall()];
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

    fn wall(&self) -> ObjectId {
        spatial::objects(&self.document.snapshot(), WALL)[0].id
    }

    fn step(
        &mut self,
        f: impl FnOnce(
            &mut InsertTool,
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
            selection: &self.selection,
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
fn the_opening_tool_keeps_the_typed_arch_rise_and_the_chain_carries_it() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin(env, run, Kind::Opening));
    rig.step(|t, env, run| t.text(env, run, "rise", None));
    rig.step(|t, env, run| t.text(env, run, "0.4", None));
    rig.step(|t, env, run| t.text(env, run, "3", None));
    let s = rig.document.snapshot();
    let o = spatial::objects(&s, WALL_OPENING);
    assert_eq!(o.len(), 1);
    let item = archbuilder_model::insert::Insert::of(&s, &o[0]).unwrap();
    assert!((item.rise - 0.4).abs() < 1e-9, "{}", item.rise);
    // The next opening of the chain has the same arch.
    rig.step(|t, env, run| t.text(env, run, "7", None));
    let s = rig.document.snapshot();
    assert_eq!(spatial::objects(&s, WALL_OPENING).len(), 2);
    for o in spatial::objects(&s, WALL_OPENING) {
        let item = archbuilder_model::insert::Insert::of(&s, &o).unwrap();
        assert!((item.rise - 0.4).abs() < 1e-9);
    }
}

#[test]
fn a_click_after_the_typed_arch_rise_places_an_arch() {
    let mut rig = Rig::new();
    rig.step(|t, env, run| t.begin(env, run, Kind::Opening));
    rig.step(|t, env, run| t.text(env, run, "rise", None));
    rig.step(|t, env, run| t.text(env, run, "0.4", None));
    let wall = rig.wall();
    rig.step(|t, env, run| t.click(env, run, wall, 4.0, false));
    let s = rig.document.snapshot();
    let o = spatial::objects(&s, WALL_OPENING);
    assert_eq!(o.len(), 1);
    let item = archbuilder_model::insert::Insert::of(&s, &o[0]).unwrap();
    assert!((item.rise - 0.4).abs() < 1e-9, "{}", item.rise);
}
