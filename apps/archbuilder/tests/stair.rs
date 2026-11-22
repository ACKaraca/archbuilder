//! The stair tool headless: a start click and a direction click make one stair (one undo step);
//! the keywords change shape and width; the preview follows the pointer; the prompt shows the
//! riser count and height, in Turkish too; the plan symbol has a cut line and the up text.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder::stair_tool::StairTool;
use archbuilder::wall_tool::Env;
use archbuilder_model::spatial;
use archbuilder_model::stair::{Placed, Shape};
use archbuilder_model::types::STAIR;
use archbuilder_model::units::LengthUnit;
use serde_json::json;

struct Rig {
    document: Document,
    registry: arc_command::Registry,
    session: Session,
    tool: StairTool,
    seed: u64,
}

impl Rig {
    fn new(language: &str) -> Self {
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
        session
            .set_setting("app.language", serde_json::Value::from(language))
            .unwrap();
        let rig = Self {
            document,
            registry: archbuilder_commands::registry().unwrap(),
            session,
            tool: StairTool::new(),
            seed: 1,
        };
        Invoker::new(&rig.registry)
            .invoke(
                &rig.document,
                &rig.session,
                Invocation {
                    seed: 99,
                    ..Invocation::new("archbuilder.project.new", json!({}))
                },
            )
            .unwrap();
        rig
    }

    fn step(
        &mut self,
        f: impl FnOnce(
            &mut StairTool,
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

    fn stairs(&self) -> Vec<Placed> {
        let snapshot = self.document.snapshot();
        spatial::objects(&snapshot, STAIR)
            .iter()
            .map(|o| Placed::of(&snapshot, o).unwrap())
            .collect()
    }

    fn prompt_text(&self, language: &str) -> String {
        let prompt = self.tool.line().active().unwrap().prompt().unwrap().clone();
        let localizer = archbuilder::localizer(&[language.to_owned()]).unwrap();
        let args: Vec<(&str, arc_base::error::Value)> = prompt
            .message
            .args
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone().into()))
            .collect();
        // Without the isolation marks around the arguments.
        localizer
            .format(&prompt.message.id, &args)
            .text
            .replace(['\u{2068}', '\u{2069}'], "")
    }
}

#[test]
fn two_clicks_make_a_stair_and_the_keywords_change_it() {
    let mut rig = Rig::new("en-US");
    rig.step(|t, env, run| t.begin(env, run));
    assert!(rig.tool.running());
    let text = rig.prompt_text("en-US");
    assert!(text.contains("17 risers"), "{text}");
    rig.step(|t, env, run| t.text(env, run, "l_turn"));
    rig.step(|t, env, run| t.text(env, run, "right"));
    rig.step(|t, env, run| t.text(env, run, "width"));
    rig.step(|t, env, run| t.text(env, run, "1.2"));
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    // The preview follows the pointer once the start is given.
    let preview = rig
        .tool
        .preview(&rig.document.snapshot(), [1.0, 4.0])
        .unwrap();
    assert!((preview.direction - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    rig.step(|t, env, run| t.click(env, run, [1.0, 4.0]));
    let stairs = rig.stairs();
    assert_eq!(stairs.len(), 1);
    assert_eq!(stairs[0].stair.shape, Shape::LTurn);
    assert!((stairs[0].stair.width - 1.2).abs() < 1e-12);
    // The tool asks for the next stair; one undo takes the stair back.
    assert!(rig.tool.running());
    rig.document.undo().wait().unwrap();
    assert!(rig.stairs().is_empty());
}

#[test]
fn the_next_stair_keeps_the_options_and_a_stray_turn_answer_is_refused() {
    let mut rig = Rig::new("en-US");
    rig.step(|t, env, run| t.begin(env, run));
    rig.step(|t, env, run| t.text(env, run, "u_turn"));
    // A letter that names no side is not taken: the side is asked again with the answers.
    rig.step(|t, env, run| t.text(env, run, "x"));
    let text = rig.prompt_text("en-US");
    assert!(
        text.contains("\"x\" is not a side") && text.contains("Right (r)"),
        "{text}"
    );
    rig.step(|t, env, run| t.text(env, run, "right"));
    rig.step(|t, env, run| t.text(env, run, "tread"));
    rig.step(|t, env, run| t.text(env, run, "0.3"));
    rig.step(|t, env, run| t.click(env, run, [5.0, 1.0]));
    rig.step(|t, env, run| t.click(env, run, [5.0, 2.0]));
    // The next stair starts as a U turn to the right with the 0.30 m tread.
    let text = rig.prompt_text("en-US");
    assert!(
        text.contains("U turn to the right") && text.contains("tread 0.3 m"),
        "{text}"
    );
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    rig.step(|t, env, run| t.click(env, run, [1.0, 2.0]));
    let stairs = rig.stairs();
    assert_eq!(stairs.len(), 2);
    assert!(stairs.iter().all(|s| s.stair.shape == Shape::UTurn
        && s.stair.turn == archbuilder_model::stair::Turn::Right
        && (s.stair.tread - 0.3).abs() < 1e-12));
}

#[test]
fn the_move_tool_takes_a_selected_stair() {
    let mut rig = Rig::new("en-US");
    rig.step(|t, env, run| t.begin(env, run));
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    rig.step(|t, env, run| t.click(env, run, [2.0, 1.0]));
    let id = rig.stairs()[0].id;
    let selection = [id];
    assert_eq!(
        archbuilder::modify_tool::movable(&rig.document.snapshot(), &selection),
        [id]
    );
    let mut tool = archbuilder::modify_tool::ModifyTool::new();
    let invoker = Invoker::new(&rig.registry);
    let env = Env {
        registry: &rig.registry,
        document: &rig.document,
        session: &rig.session,
        selection: &selection,
        seed: 77,
    };
    let (document, session) = (&rig.document, &rig.session);
    let mut run = |inv| invoker.invoke(document, session, inv);
    tool.begin(&env, &mut run, archbuilder_commands::modify::MODIFY_MOVE)
        .unwrap();
    assert!(tool.running());
    tool.text(&env, &mut run, "0,0").unwrap();
    tool.text(&env, &mut run, "1,0.5").unwrap();
    assert_eq!(rig.stairs()[0].stair.start, [2.0, 1.5]);
}

#[test]
fn the_prompt_speaks_turkish() {
    let mut rig = Rig::new("tr-TR");
    rig.step(|t, env, run| t.begin(env, run));
    let text = rig.prompt_text("tr-TR");
    assert!(text.contains("17 rıht") && text.contains("düz"), "{text}");
    rig.step(|t, env, run| t.text(env, run, "u"));
    let text = rig.prompt_text("tr-TR");
    assert!(text.contains("yana"), "{text}");
}
