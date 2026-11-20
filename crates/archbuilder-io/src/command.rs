//! The command `archbuilder.file.export_ifc`: writes the document as an IFC4 file to a path.
//!
//! The file is reached only through [`Context::create_write`], so the session's write roots and
//! permission class decide where it may go. The command changes nothing in the document and is
//! not undoable.

use std::io::Write;
use std::path::Path;

use arc_base::{Error, Result};
use arc_command::{CommandSpec, Context, Permission, Registry, UndoBehavior};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ifc::{ExportOptions, export_ifc};
use crate::time::iso8601;

/// The identifier of the export command.
pub const EXPORT_IFC: &str = "archbuilder.file.export_ifc";

/// A message of this crate.
struct Message {
    message_id: &'static str,
}

const LABEL: Message = Message {
    message_id: "ab-io-command-export-ifc-label",
};
const DESCRIPTION: Message = Message {
    message_id: "ab-io-command-export-ifc-description",
};

/// Parameters of `archbuilder.file.export_ifc`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportParams {
    /// The file to write, inside the session's write roots.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
}

/// What `archbuilder.file.export_ifc` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Exported {
    /// The name of the file written, without its directory.
    pub file: String,
    /// The bytes written.
    pub bytes: u64,
    /// The walls, floors, doors and windows written.
    pub elements: u64,
    /// Objects or parts not exported, and approximations, counted by the loss report.
    pub dropped: u64,
    /// Occurrences exported with an approximation.
    pub approximated: u64,
    /// The loss report, in the form of `schemas/loss-report.schema.json`.
    pub report: serde_json::Value,
}

fn export(ctx: &mut Context<'_>, p: ExportParams) -> Result<Exported> {
    ctx.checkpoint()?;
    let path = Path::new(&p.path);
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    let options = ExportOptions {
        file_name: file.clone(),
        timestamp: iso8601(ctx.now_unix_millis()),
        application: format!("archbuilder {}", env!("CARGO_PKG_VERSION")),
        limits: *ctx.limits(),
        ..ExportOptions::default()
    };
    let export = export_ifc(ctx.snapshot(), &options)?;
    let report = serde_json::to_value(&export.report)
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    // shortcut: writes in place; an interrupted write leaves a partial file, upgrade to a
    // temporary file and `replace_file` as the application's save does.
    let mut target = ctx.create_write(path)?;
    target
        .write_all(&export.bytes)
        .and_then(|()| target.flush())
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    Ok(Exported {
        file,
        bytes: u64::try_from(export.bytes.len()).unwrap_or(u64::MAX),
        elements: u64::try_from(export.elements).unwrap_or(u64::MAX),
        dropped: export.report.summary.dropped,
        approximated: export.report.summary.approximated,
        report,
    })
}

/// Registers `archbuilder.file.export_ifc` (permission classes `read` and `file_write`, not
/// undoable) into `registry`.
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(EXPORT_IFC, LABEL.message_id, DESCRIPTION.message_id, export)
            .with_undo(UndoBehavior::NotUndoable)
            .with_permissions([Permission::Read, Permission::FileWrite]),
    )
}
