//! The railing and ramp tools headless (they run on the stair tool's machinery): clicks and Enter
//! make one railing, two clicks one ramp with its railings, each one undo step; the prompts are
//! localized.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder::stair_tool::StairTool;
use archbuilder::wall_tool::Env;
use archbuilder_commands::circulation::{RAILING_CREATE, RAMP_CREATE};
use archbuilder_model::spatial;
use archbuilder_model::types::{RAILING, RAMP};
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

    fn count(&self, key: &str) -> usize {
        spatial::objects(&self.document.snapshot(), key).len()
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
fn clicks_and_enter_make_a_railing_in_one_undo_step() {
    let mut rig = Rig::new("en-US");
    rig.step(|t, env, run| t.begin(env, run, RAILING_CREATE));
    assert!(rig.tool.running());
    assert!(rig.prompt_text("en-US").contains("first point"));
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    rig.step(|t, env, run| t.click(env, run, [4.0, 1.0]));
    // Two points make a path: Enter finishes it.
    assert!(rig.prompt_text("en-US").contains("2 points"));
    rig.step(|t, env, run| t.click(env, run, [4.0, 3.0]));
    rig.step(|t, env, run| t.text(env, run, ""));
    assert_eq!(rig.count(RAILING), 1);
    // The tool asks for the next railing.
    assert!(rig.tool.running());
    assert_eq!(rig.tool.command(), RAILING_CREATE);
    rig.step(|t, env, run| t.text(env, run, ""));
    assert!(!rig.tool.running());
    rig.document.undo().wait().unwrap();
    assert_eq!(rig.count(RAILING), 0);
}

#[test]
fn two_clicks_make_a_ramp_with_railings_and_the_prompts_are_turkish_too() {
    let mut rig = Rig::new("tr-TR");
    rig.step(|t, env, run| t.begin(env, run, RAMP_CREATE));
    assert!(rig.prompt_text("tr-TR").contains("ayağını"));
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    assert!(rig.prompt_text("tr-TR").contains("ucunu"));
    rig.step(|t, env, run| t.click(env, run, [7.0, 1.0]));
    assert_eq!((rig.count(RAMP), rig.count(RAILING)), (1, 2));
    assert_eq!(rig.tool.command(), RAMP_CREATE);
    rig.step(|t, env, run| t.text(env, run, ""));
    rig.document.undo().wait().unwrap();
    assert_eq!((rig.count(RAMP), rig.count(RAILING)), (0, 0));
}
