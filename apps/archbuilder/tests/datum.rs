//! What the application draws for datums: 3D extraction, plan and 3D overlays, picking.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::Cancel;
use arc_command::{Invocation, Invoker, Outcome, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_model::spatial;
use archbuilder_model::types::{GRID, WORK_PLANE};
use serde_json::{Value as Json, json};

/// Every environment draws its identifiers from its own seed: the derived caches are keyed by
/// identifier, and tests that run side by side must not share them.
static SEEDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

struct Env {
    seed: u64,
    document: Document,
    session: Session,
    registry: arc_command::Registry,
}

impl Env {
    fn new() -> Self {
        let config = DocumentConfig {
            workers: 0,
            ..DocumentConfig::default()
        };
        let document = Document::open(
            Snapshot::empty(Arc::new(archbuilder_commands::types().unwrap())),
            Arc::new(Evaluators::new()),
            config,
        )
        .unwrap();
        archbuilder_commands::install(&document);
        let env = Self {
            seed: SEEDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            document,
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
            registry: archbuilder_commands::registry().unwrap(),
        };
        env.run("archbuilder.project.new", json!({})).unwrap();
        env
    }

    fn run(&self, command: &str, params: Json) -> arc_base::Result<Outcome> {
        let invocation = Invocation {
            seed: self.seed,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry).invoke(&self.document, &self.session, invocation)
    }

    fn snapshot(&self) -> Snapshot {
        self.document.snapshot()
    }
}

#[test]
fn a_grid_is_extracted_in_3d_labeled_in_plan_and_3d_and_picked_near_its_line() {
    let env = Env::new();
    env.run(
        "archbuilder.grid.create",
        json!({"start": [0.0, 0.0], "end": [0.0, 10.0], "label": "A"}),
    )
    .unwrap();
    env.run(
        "archbuilder.work_plane.set",
        json!({"target": "Floor 1", "shown": true}),
    )
    .unwrap();
    let s = env.snapshot();
    let grid = spatial::objects(&s, GRID)[0].clone();
    let extraction = archbuilder::extract_datum::extract_grid(&s, &grid, &Cancel::new()).unwrap();
    assert_eq!(extraction.items.len(), 1);
    let plane = spatial::objects(&s, WORK_PLANE)[0].clone();
    let shown = archbuilder::extract_datum::extract_work_plane(&s, &plane, &Cancel::new()).unwrap();
    assert_eq!(shown.items.len(), 1);
    // Hidden planes draw nothing.
    env.run("archbuilder.work_plane.show", json!({"shown": false}))
        .unwrap();
    let s = env.snapshot();
    let plane = spatial::objects(&s, WORK_PLANE)[0].clone();
    assert!(
        archbuilder::extract_datum::extract_work_plane(&s, &plane, &Cancel::new())
            .unwrap()
            .items
            .is_empty()
    );
    // A plan view at the ground floor shows the line, both heads and the label twice;
    // a view at a level above the grid's extent shows nothing.
    let plan = archbuilder::datum_view::overlays(&s, Some(0.0), &|_| false);
    assert_eq!(plan.len(), 1 + 2 * 2);
    let space = archbuilder::datum_view::overlays(&s, None, &|_| false);
    assert_eq!(space.len(), 2 * 2);
    assert!(archbuilder::datum_view::overlays(&s, Some(0.0), &|_| true).is_empty());
    env.run(
        "archbuilder.datum.set_extent",
        json!({"objects": [grid.id.to_string()], "top_level": "Ground Floor"}),
    )
    .unwrap();
    let s = env.snapshot();
    assert_eq!(
        archbuilder::datum_view::overlays(&s, Some(6.0), &|_| false).len(),
        0
    );
    // Picking: within the aperture of the line, nothing farther away.
    assert_eq!(
        archbuilder_model::pick::stack(&s, &[], [0.05, 4.0], 0.1, None),
        [grid.id]
    );
    assert!(archbuilder_model::pick::stack(&s, &[], [3.0, 4.0], 0.1, None).is_empty());
}
