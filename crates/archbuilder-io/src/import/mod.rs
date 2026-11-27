//! IFC import (AB-IO-007): `archbuilder.file.import_ifc` reads an IFC2X3, IFC4 or IFC4X3 file with
//! `arc-io-ifc` and maps it into the document in one transaction, so one undo step takes the
//! whole import back.
//!
//! - Spatial structure: the project's information, the site, buildings and storeys (as levels
//!   with plan views).
//! - Walls (an extruded rectangle with its layer set usage), doors and windows (hosted by the wall
//!   the file's void and fill relationships name), slabs (floors), coverings of type `CEILING`
//!   (ceilings), spaces (rooms) and grids.
//! - Everything else is a foreign element ([`archbuilder_model::types::FOREIGN_ELEMENT`]): the
//!   source class, the predefined type, the data of the source and its mesh. Property sets,
//!   quantity sets, classification and material of every object are kept beside it as a foreign
//!   data row, and the source `GlobalId` is its entry in the source map of the document
//!   (ADR 0007); the export writes them back.
//! - The original file and the loss report are kept as document-level foreign data rows.
//!
//! No geometry is matched by proximity: a door's wall is the one the file says. Re-importing a
//! file into a document that holds its products matches them by `GlobalId` and does not make them
//! again.

mod apply;
pub mod data;
mod geom;
mod map;
mod plan;
mod read;

use std::io::Read as _;
use std::path::Path;

use arc_base::{Error, Result};
use arc_command::{CommandSpec, Context, Permission, Registry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use map::MapOptions;
use read::open;

use crate::error::codes;

/// The identifier of the import command.
pub const IMPORT_IFC: &str = "archbuilder.file.import_ifc";

/// The largest original file kept in the document, in bytes.
pub const MAX_KEPT_SOURCE: usize = 64 * 1024 * 1024;

struct Message {
    message_id: &'static str,
}

const LABEL: Message = Message {
    message_id: "ab-io-command-import-ifc-label",
};
const DESCRIPTION: Message = Message {
    message_id: "ab-io-command-import-ifc-description",
};

/// Parameters of `archbuilder.file.import_ifc`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportParams {
    /// The IFC file (`.ifc`, or `.ifczip`), inside the session's read roots.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
    /// Keep the original file in the document (on by default; files over 64 MiB are never kept).
    #[serde(default)]
    pub keep_source: Option<bool>,
}

/// The counts of one entity class.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ClassCount {
    /// The class in the schema's spelling.
    pub class: String,
    /// Products of the class in the file.
    pub seen: u64,
    /// Mapped to native elements.
    pub native: u64,
    /// Kept as foreign elements.
    pub foreign: u64,
    /// Not carried: listed in the loss report as dropped.
    pub lost: u64,
    /// Matched to objects the document already has by `GlobalId`.
    pub matched: u64,
}

/// What `archbuilder.file.import_ifc` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Imported {
    /// The name of the file read.
    pub file: String,
    /// The schema of the file.
    pub schema: String,
    /// The products of the file.
    pub products: u64,
    /// Products mapped to native elements.
    pub native: u64,
    /// Products kept as foreign elements.
    pub foreign: u64,
    /// Products not carried.
    pub lost: u64,
    /// Products matched to objects the document already has.
    pub matched: u64,
    /// The buildings made.
    pub buildings: u64,
    /// The storeys made.
    pub storeys: u64,
    /// The types made.
    pub types: u64,
    /// The counts per class.
    pub classes: Vec<ClassCount>,
    /// The loss report, in the form of `schemas/loss-report.schema.json`.
    pub report: serde_json::Value,
}

/// Maps `bytes` and writes the result into the open transaction of `ctx`.
///
/// # Errors
/// `ab.io.import_unreadable` for a file the reader refuses, and the errors of the transaction.
pub fn import_bytes(
    ctx: &mut Context<'_>,
    bytes: &[u8],
    file: &str,
    keep_source: bool,
) -> Result<Imported> {
    let limits = *ctx.limits();
    let cancel = arc_base::Cancel::new();
    let source = open(bytes, &limits, &cancel).map_err(|e| {
        if e.code().starts_with("io.") {
            Error::new(codes::IMPORT_UNREADABLE).with_source(e)
        } else {
            e
        }
    })?;
    // Without the tables of the file's schema the views are empty and nothing would be mapped.
    if source.file.tables().is_none() {
        return Err(Error::new(codes::IMPORT_TABLES_MISSING)
            .with_arg("schema", source.file.schema.identifier().to_owned()));
    }
    ctx.checkpoint()?;
    let snapshot = ctx.snapshot().clone();
    let bound = apply::bound(&snapshot);
    let known = |gid: &str| apply::is_known(&snapshot, &bound, gid);
    let plan = map::map(
        &source,
        &MapOptions {
            limits,
            cancel,
            known: &known,
        },
    )?;
    ctx.checkpoint()?;
    let mut report = plan.report.clone();
    report.header.file_name = Some(file.to_owned());
    let report_json = serde_json::to_value(&report)
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    let report_bytes = serde_json::to_vec(&report_json).unwrap_or_default();
    let original = (keep_source && bytes.len() <= MAX_KEPT_SOURCE).then_some(bytes);
    let applied = apply::apply(ctx, &plan, file, original, &report_bytes)?;
    let mut classes: Vec<ClassCount> = plan
        .tally
        .iter()
        .map(|(class, t)| ClassCount {
            class: source
                .file
                .tables()
                .and_then(|tables| tables.entity(class))
                .map_or_else(|| class.clone(), |e| e.name.to_owned()),
            seen: t.seen,
            native: t.native,
            foreign: t.foreign,
            lost: t.lost,
            matched: t.matched,
        })
        .collect();
    classes.sort_by(|a, b| a.class.cmp(&b.class));
    let sum = |f: fn(&plan::Tally) -> u64| plan.tally.values().map(f).sum::<u64>();
    let count = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
    Ok(Imported {
        file: file.to_owned(),
        schema: plan.schema.clone(),
        products: sum(|t| t.seen),
        native: sum(|t| t.native),
        foreign: sum(|t| t.foreign),
        lost: sum(|t| t.lost),
        matched: sum(|t| t.matched),
        buildings: count(applied.buildings),
        storeys: count(applied.storeys),
        types: count(applied.types),
        classes,
        report: report_json,
    })
}

fn import_command(ctx: &mut Context<'_>, p: ImportParams) -> Result<Imported> {
    ctx.checkpoint()?;
    let path = Path::new(&p.path);
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    let handle = ctx.open_read(path)?;
    let limit = ctx.limits().input_bytes;
    let mut bytes = Vec::new();
    handle
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(Error::new(codes::IMPORT_UNREADABLE));
    }
    import_bytes(ctx, &bytes, &file, p.keep_source.unwrap_or(true))
}

/// Registers `archbuilder.file.import_ifc` (permission classes `read`, `edit` and `file_read`;
/// undoable: one undo step for the whole import) into `registry`.
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
            IMPORT_IFC,
            LABEL.message_id,
            DESCRIPTION.message_id,
            import_command,
        )
        .with_permissions([Permission::Read, Permission::Edit, Permission::FileRead]),
    )
}
