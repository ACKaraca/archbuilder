//! Family instances in the application: the 3D extraction draws a mesh per solid the definition
//! shows in 3D, the plan view draws the symbolic geometry the definition shows at its detail
//! level, and the extractor is registered with the product.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::collections::BTreeMap;
use std::sync::Arc;

use arc_base::id::Identifier;
use arc_base::{Cancel, ObjectId};
use arc_command::{FixedClock, Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::package::LockOwner;
use arc_doc::{Document, DocumentConfig, Header, Snapshot};
use archbuilder_model::family::fixtures::door;
use archbuilder_model::family::loaded::{Component, FamilyType, Loaded};
use archbuilder_model::family::package::publish;
use archbuilder_model::family::{Version, def::Detail};
use archbuilder_model::spatial;
use archbuilder_model::types::{COMPONENT, FAMILY, FAMILY_TYPE, PLAN_VIEW, STOREY};
use serde_json::json;

fn project_with_door() -> (Document, ObjectId) {
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
    let session = Session::new(Surface::Ui, [Permission::Read, Permission::Edit]);
    Invoker::new(&registry)
        .invoke(
            &document,
            &session,
            Invocation {
                seed: 9_090,
                ..Invocation::new("archbuilder.project.new", json!({}))
            },
        )
        .unwrap();
    let snapshot = document.snapshot();
    let project = spatial::project(&snapshot).unwrap().id;
    let storey = spatial::objects(&snapshot, STOREY)[0].id;
    let definition = door();
    let published = publish(
        &definition,
        None,
        Version {
            major: 1,
            minor: 0,
            patch: 0,
        },
    )
    .unwrap();
    let values = definition.type_named("0900x2100").unwrap().values.clone();
    let (family, ty, comp) = (
        ObjectId::from_u128(0xA1),
        ObjectId::from_u128(0xA2),
        ObjectId::from_u128(0xA3),
    );
    document
        .submit(move |txn| {
            let owned = |owner| Header {
                owner: Some(owner),
                ..Header::default()
            };
            txn.create(
                FAMILY,
                family,
                owned(project),
                Loaded::values(&published, &definition),
            )?;
            txn.create(
                FAMILY_TYPE,
                ty,
                owned(project),
                FamilyType::record("0900x2100", family, &values),
            )?;
            let c = Component {
                id: comp,
                storey: Some(storey),
                name: "D1".into(),
                family_type: ty,
                offset: 0.0,
                origin: [2.0, 1.0],
                rotation: 0.0,
                mirror: false,
                values: BTreeMap::new(),
            };
            txn.create(COMPONENT, comp, owned(storey), c.record())
        })
        .wait()
        .unwrap();
    (document, comp)
}

#[test]
fn the_viewport_draws_a_mesh_per_solid_shown_in_3d() {
    let (document, comp) = project_with_door();
    let snapshot = document.snapshot();
    let object = spatial::get(&snapshot, comp).unwrap();
    let extraction =
        archbuilder::extract_component::extract_component(&snapshot, &object, &Cancel::new())
            .unwrap();
    assert_eq!(extraction.items.len(), 2, "frame and leaf");
}

#[test]
fn the_plan_view_draws_the_symbols_its_detail_level_shows() {
    let (document, comp) = project_with_door();
    let snapshot = document.snapshot();
    let placed = archbuilder_model::family::loaded::place(
        &snapshot,
        &Component::of(&spatial::get(&snapshot, comp).unwrap()).unwrap(),
    )
    .unwrap();
    // The definition shows the swing at medium and high detail only.
    let swing = placed.plan.iter().find(|s| s.key == "swing").unwrap();
    assert!(
        swing
            .shown
            .at(Detail::High, archbuilder_model::family::def::ViewKind::Plan)
    );
    assert!(
        !swing
            .shown
            .at(Detail::Low, archbuilder_model::family::def::ViewKind::Plan)
    );
    // The plan view holds primitives for the component.
    let view = spatial::objects(&snapshot, PLAN_VIEW)
        .into_iter()
        .next()
        .unwrap();
    let built = archbuilder_model::plan::build(
        &snapshot,
        &view,
        &archbuilder_model::plan::Filter::default(),
        &Cancel::new(),
    )
    .unwrap();
    assert!(
        !built.primitives_of(comp).is_empty(),
        "{:?}",
        built.diagnostics()
    );
}

#[test]
fn the_component_extractor_is_registered() {
    let owner = LockOwner {
        user: "t".into(),
        host: "t".into(),
        application: "t".into(),
        time_unix_ms: 0,
    };
    let product = archbuilder::product(Arc::new(FixedClock(0)), owner).unwrap();
    assert!(
        product
            .extractors()
            .iter()
            .any(|e| e.id == "archbuilder.component")
    );
}
