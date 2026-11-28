//! The environment of the wall shape export tests: a document with the commands, and the export
//! of it (self-validated, lossless), also written to `ARCB_IFC_OUT` when that is set.

#![allow(dead_code, clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use arc_base::ObjectId;
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::graph::Evaluators;
use arc_doc::{Document, DocumentConfig, Snapshot};
use archbuilder_io::{ExportOptions, export_ifc};
use archbuilder_model::cleanup;
use archbuilder_model::spatial;
use archbuilder_model::types::WALL;
use serde_json::{Value as Json, json};

pub struct Env {
    pub document: Document,
    pub session: Session,
    pub registry: arc_command::Registry,
}

impl Env {
    pub fn new() -> Self {
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
        let env = Self {
            document,
            session: Session::new(Surface::Ui, [Permission::Read, Permission::Edit]),
            registry: archbuilder_commands::registry().unwrap(),
        };
        env.run("archbuilder.project.new", json!({}));
        env
    }

    pub fn run(&self, command: &str, params: Json) -> Json {
        let invocation = Invocation {
            seed: 7,
            ..Invocation::new(command, params)
        };
        Invoker::new(&self.registry)
            .invoke(&self.document, &self.session, invocation)
            .unwrap()
            .value
    }

    pub fn wall(&self, params: Json) -> ObjectId {
        self.run("archbuilder.wall.create", params)["wall"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }
}

pub fn export(env: &Env, name: &str) -> f64 {
    let snapshot = env.document.snapshot();
    let exported = export_ifc(
        &snapshot,
        &ExportOptions {
            file_name: format!("{name}.ifc"),
            timestamp: "2026-01-02T03:04:05".into(),
            application: "archbuilder test".into(),
            ..ExportOptions::default()
        },
    )
    .unwrap();
    assert!(exported.valid, "{name}: {:?}", exported.report.items);
    assert!(!exported.report.has_loss(), "{name}: {:?}", exported.report);
    if let Ok(dir) = std::env::var("ARCB_IFC_OUT") {
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.ifc")),
            &exported.bytes,
        )
        .unwrap();
    }
    spatial::objects(&snapshot, WALL)
        .iter()
        .map(|w| cleanup::wall_net_volume(&snapshot, w).unwrap())
        .sum()
}

/// The export of curtain walls: valid, lossless, one plate per glazed panel and one member per
/// mullion.
pub fn export_curtain(env: &Env, name: &str) {
    let snapshot = env.document.snapshot();
    let exported = export_ifc(
        &snapshot,
        &ExportOptions {
            file_name: format!("{name}.ifc"),
            timestamp: "2026-01-02T03:04:05".into(),
            application: "archbuilder test".into(),
            ..ExportOptions::default()
        },
    )
    .unwrap();
    assert!(exported.valid, "{name}: {:?}", exported.report.items);
    assert!(!exported.report.has_loss(), "{name}: {:?}", exported.report);
    if let Ok(dir) = std::env::var("ARCB_IFC_OUT") {
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.ifc")),
            &exported.bytes,
        )
        .unwrap();
    }
}
