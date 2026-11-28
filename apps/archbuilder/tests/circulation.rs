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

#[test]
fn create_similar_starts_the_tool_with_the_type_of_the_selected_railing_or_ramp() {
    let mut rig = Rig::new("en-US");
    let run = |rig: &Rig, command: &str, params: serde_json::Value| {
        Invoker::new(&rig.registry)
            .invoke(
                &rig.document,
                &rig.session,
                Invocation {
                    seed: 5,
                    ..Invocation::new(command, params)
                },
            )
            .unwrap()
    };
    let tall = run(
        &rig,
        "archbuilder.railing_type.define",
        json!({"name": "Tall", "height": 1.1}),
    );
    let steep = run(
        &rig,
        "archbuilder.ramp_type.define",
        json!({"name": "Steep", "max_slope": 0.2}),
    );
    let id = |o: &arc_command::Outcome| -> arc_base::ObjectId {
        o.value["type"].as_str().unwrap().parse().unwrap()
    };
    let (tall, steep) = (id(&tall), id(&steep));
    // What the selection of a railing or a ramp gives to create similar.
    let railing = run(
        &rig,
        "archbuilder.railing.create",
        json!({"points": [[0.0, 5.0], [2.0, 5.0]], "railing_type": "Tall"}),
    );
    let railing: arc_base::ObjectId = railing.value["railing"].as_str().unwrap().parse().unwrap();
    let (kind, found) =
        archbuilder::circ_types::CircKind::of_object(&rig.document.snapshot(), railing).unwrap();
    assert_eq!(
        (kind, found),
        (archbuilder::circ_types::CircKind::Railing, Some(tall))
    );
    rig.step(|t, env, run| t.begin_similar(env, run, RAILING_CREATE, "railing_type", tall));
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    rig.step(|t, env, run| t.click(env, run, [4.0, 1.0]));
    rig.step(|t, env, run| t.text(env, run, ""));
    let snapshot = rig.document.snapshot();
    let made = spatial::objects(&snapshot, RAILING);
    assert_eq!(made.len(), 2);
    let kind_of =
        |o: &arc_doc::Object| o.values[archbuilder_model::types::railing::RAILING_TYPE].clone();
    assert!(
        made.iter()
            .all(|o| kind_of(o) == Some(arc_doc::Value::Reference(tall)))
    );
    rig.step(|t, env, run| t.text(env, run, ""));
    // A ramp made the same way takes the ramp type; its railings keep the default type.
    rig.step(|t, env, run| t.begin_similar(env, run, RAMP_CREATE, "ramp_type", steep));
    rig.step(|t, env, run| t.click(env, run, [1.0, 8.0]));
    rig.step(|t, env, run| t.click(env, run, [7.0, 8.0]));
    let snapshot = rig.document.snapshot();
    let ramps = spatial::objects(&snapshot, RAMP);
    assert_eq!(ramps.len(), 1);
    assert_eq!(
        ramps[0].values[archbuilder_model::types::ramp::RAMP_TYPE],
        Some(arc_doc::Value::Reference(steep))
    );
    rig.step(|t, env, run| t.text(env, run, ""));
    rig.document.undo().wait().unwrap();
    assert_eq!(rig.count(RAMP), 0);
}

#[test]
fn the_keyword_sketch_makes_a_ramp_from_a_run_line_and_a_boundary() {
    let mut rig = Rig::new("en-US");
    rig.step(|t, env, run| t.begin(env, run, RAMP_CREATE));
    rig.step(|t, env, run| t.text(env, run, "sketch"));
    rig.step(|t, env, run| t.click(env, run, [1.0, 2.0]));
    rig.step(|t, env, run| t.click(env, run, [7.0, 2.0]));
    assert!(rig.prompt_text("en-US").contains("boundary"));
    for p in [[1.0, 1.0], [8.0, 1.0], [8.0, 3.0]] {
        rig.step(|t, env, run| t.click(env, run, p));
    }
    assert_eq!(rig.count(RAMP), 0, "the boundary is open until Enter");
    rig.step(|t, env, run| t.click(env, run, [1.0, 3.0]));
    rig.step(|t, env, run| t.text(env, run, ""));
    assert_eq!((rig.count(RAMP), rig.count(RAILING)), (1, 2));
    let snapshot = rig.document.snapshot();
    let made =
        archbuilder_model::ramp::Ramp::from_values(&spatial::objects(&snapshot, RAMP)[0].values)
            .unwrap();
    assert_eq!(made.boundary.len(), 4);
    assert_eq!(made.start, [1.0, 2.0]);
    rig.document.undo().wait().unwrap();
    assert_eq!((rig.count(RAMP), rig.count(RAILING)), (0, 0));
}

#[test]
fn the_tool_previews_the_railing_path_the_ramp_outline_and_the_sketch_as_the_pointer_moves() {
    let mut rig = Rig::new("en-US");
    // Nothing to draw before the first point.
    rig.step(|t, env, run| t.begin(env, run, RAILING_CREATE));
    assert!(rig.tool.sketch([1.0, 1.0]).is_none());
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    rig.step(|t, env, run| t.click(env, run, [4.0, 1.0]));
    // The path so far and the pointer.
    let sketch = rig.tool.sketch([4.0, 3.0]).unwrap();
    assert_eq!(sketch.lines, vec![vec![[1.0, 1.0], [4.0, 1.0], [4.0, 3.0]]]);
    rig.step(|t, env, run| t.text(env, run, ""));
    rig.step(|t, env, run| t.text(env, run, ""));
    // A ramp: after the foot, its outline (1.2 m wide) and walking line to the pointer.
    rig.step(|t, env, run| t.begin(env, run, RAMP_CREATE));
    assert!(rig.tool.sketch([1.0, 1.0]).is_none());
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    let sketch = rig.tool.sketch([7.0, 1.0]).unwrap();
    assert_eq!(sketch.lines.len(), 2);
    let outline = &sketch.lines[0];
    assert_eq!(outline.first(), outline.last());
    let (lo, hi) = outline.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p[1]), hi.max(p[1]))
    });
    assert!((hi - lo - 1.2).abs() < 1.0e-9 && (lo - 0.4).abs() < 1.0e-9);
    assert_eq!(sketch.lines[1], vec![[1.0, 1.0], [7.0, 1.0]]);
    // Too close to the foot, there is no ramp to draw.
    assert!(rig.tool.sketch([1.0, 1.02]).is_none());
    // The sketch: the run line, then the boundary with the pointer, closed.
    rig.step(|t, env, run| t.begin(env, run, RAMP_CREATE));
    rig.step(|t, env, run| t.text(env, run, "sketch"));
    rig.step(|t, env, run| t.click(env, run, [1.0, 2.0]));
    rig.step(|t, env, run| t.click(env, run, [7.0, 2.0]));
    rig.step(|t, env, run| t.click(env, run, [1.0, 1.0]));
    let sketch = rig.tool.sketch([8.0, 1.0]).unwrap();
    assert_eq!(sketch.lines[0], vec![[1.0, 2.0], [7.0, 2.0]]);
    assert_eq!(sketch.lines[1], vec![[1.0, 1.0], [8.0, 1.0], [1.0, 1.0]]);
    rig.step(|t, env, run| t.text(env, run, ""));
    assert!(rig.tool.sketch([8.0, 1.0]).is_none() || !rig.tool.running());
}

#[test]
fn a_ramp_writes_its_slope_at_the_middle_of_each_run_in_the_decimal_separator_of_the_language() {
    let rig = Rig::new("en-US");
    let run = |params: serde_json::Value| {
        Invoker::new(&rig.registry)
            .invoke(
                &rig.document,
                &rig.session,
                Invocation {
                    seed: 5,
                    ..Invocation::new("archbuilder.ramp.create", params)
                },
            )
            .unwrap()
    };
    let made = run(json!({
        "start": [0.0, 0.0], "to": [12.0, 0.0], "rise": 1.0, "landing_start": 1.0, "landing_end": 2.0,
        "runs": [{"turn": "90", "length": 6.0, "slope": 0.05}], "railings": [],
    }));
    let ramp: arc_base::ObjectId = made.value["ramp"].as_str().unwrap().parse().unwrap();
    let snapshot = rig.document.snapshot();
    let storey = spatial::get(&snapshot, ramp).unwrap().header.owner.unwrap();
    let texts = archbuilder::ramp_view::texts(&snapshot, storey, "en-US", &|_| false);
    // 1 m over 18 m of runs is 5.6 % on the first run, which has no slope of its own; the
    // second has 5 %.
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0].3, "5.6 %");
    assert_eq!(texts[1].3, "5.0 %");
    // The first run's middle: 1 m landing, then 6 m of 12: x = 7.
    assert!((texts[0].1[0] - 7.0).abs() < 1.0e-9 && texts[0].1[1].abs() < 1.0e-9);
    // In Turkish the separator is a comma; a run over the type's largest slope is marked.
    let tr = archbuilder::ramp_view::texts(&snapshot, storey, "tr-TR", &|_| false);
    assert_eq!(tr[0].3, "5,6 %");
    let steep = run(json!({"start": [0.0, 5.0], "to": [6.0, 5.0], "rise": 1.0, "railings": []}));
    let steep: arc_base::ObjectId = steep.value["ramp"].as_str().unwrap().parse().unwrap();
    let all = archbuilder::ramp_view::texts(&rig.document.snapshot(), storey, "en-US", &|_| false);
    assert!(all.iter().any(|t| t.0 == steep && t.3 == "16.7 % \u{26a0}"));
    // A hidden ramp has no text.
    let none =
        archbuilder::ramp_view::texts(&rig.document.snapshot(), storey, "en-US", &|id| id == steep);
    assert!(none.iter().all(|t| t.0 != steep));
}
