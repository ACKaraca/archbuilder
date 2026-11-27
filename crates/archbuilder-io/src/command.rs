//! The command `archbuilder.file.export_ifc`: writes the document as an IFC4 file to a path; and
//! the two commands that move the typed-shortcut table through a file,
//! `archbuilder.shortcuts.export` (writes the text it is given) and `archbuilder.shortcuts.import`
//! (returns the text of a file, at most [`MAX_SHORTCUT_BYTES`] bytes).
//!
//! The file is reached only through [`Context::create_write`], so the session's write roots and
//! permission class decide where it may go. The command changes nothing in the document and is
//! not undoable.

use std::io::{Read, Write};
use std::path::Path;

use arc_base::{Error, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ifc::{ExportOptions, export_ifc};
use crate::mesh::{MeshFormat, MeshOptions, Unit, export_mesh};
use crate::time::iso8601;

/// The identifier of the export command.
pub const EXPORT_IFC: &str = "archbuilder.file.export_ifc";

/// The identifier of the mesh export command.
pub const EXPORT_MESH: &str = "archbuilder.file.export_mesh";

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

const MESH_LABEL: Message = Message {
    message_id: "ab-io-command-export-mesh-label",
};
const MESH_DESCRIPTION: Message = Message {
    message_id: "ab-io-command-export-mesh-description",
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
    /// Whether the file passed self-validation; when it did not the file is incomplete and the
    /// report names each problem as a `failed` item.
    pub valid: bool,
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
        valid: export.valid,
        report,
    })
}

/// The format of a mesh file as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeshFormatParam {
    /// Wavefront OBJ with an MTL file beside it.
    Obj,
    /// Binary STL.
    Stl,
    /// glTF 2.0 JSON with the buffer embedded.
    Gltf,
    /// glTF 2.0 binary container.
    Glb,
}

/// The unit of a mesh file as a command parameter.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnitParam {
    /// Meters.
    Meter,
    /// Centimeters.
    Centimeter,
    /// Millimeters.
    Millimeter,
    /// Feet.
    Foot,
    /// Inches.
    Inch,
}

const fn yes() -> bool {
    true
}

/// Parameters of `archbuilder.file.export_mesh`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportMeshParams {
    /// The file to write, inside the session's write roots. An OBJ file with materials is
    /// written with an MTL file of the same name beside it.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
    /// The format; by default the one the extension of the path names.
    #[serde(default)]
    pub format: Option<MeshFormatParam>,
    /// The unit of the coordinates; meters by default. glTF is always in meters.
    #[serde(default)]
    pub unit: Option<UnitParam>,
    /// Write only the selected elements.
    #[serde(default)]
    pub selection: bool,
    /// Write only these elements (instead of the selection or the whole model).
    #[serde(default)]
    #[schemars(length(max = 100000))]
    pub elements: Vec<IdParam>,
    /// Leave out these elements: the ones the caller's view does not show.
    #[serde(default)]
    #[schemars(length(max = 100000))]
    pub hidden: Vec<IdParam>,
    /// One object for the whole model, split only by material.
    #[serde(default)]
    pub merge: bool,
    /// Whether colors are written (OBJ with MTL, glTF materials); on by default.
    #[serde(default = "yes")]
    pub materials: bool,
}

/// What `archbuilder.file.export_mesh` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MeshExported {
    /// The name of the file written, without its directory.
    pub file: String,
    /// The name of the MTL file written beside an OBJ file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub materials_file: Option<String>,
    /// The bytes written to the main file.
    pub bytes: u64,
    /// The elements written (1 when merged).
    pub elements: u64,
    /// The triangles written.
    pub triangles: u64,
    /// The bounding box of the positions, minimum then maximum, in the unit of the file.
    pub bounds: Option<[[f64; 3]; 2]>,
    /// Objects not exported and occurrences not carried, counted by the loss report.
    pub dropped: u64,
    /// Occurrences written with an approximation.
    pub approximated: u64,
    /// The loss report.
    pub report: serde_json::Value,
}

fn export_mesh_command(ctx: &mut Context<'_>, p: ExportMeshParams) -> Result<MeshExported> {
    ctx.checkpoint()?;
    let path = Path::new(&p.path);
    let file = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    let format = match p.format {
        Some(MeshFormatParam::Obj) => MeshFormat::Obj,
        Some(MeshFormatParam::Stl) => MeshFormat::Stl,
        Some(MeshFormatParam::Gltf) => MeshFormat::Gltf,
        Some(MeshFormatParam::Glb) => MeshFormat::Glb,
        None => path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(MeshFormat::of_extension)
            .ok_or_else(|| Error::new(crate::error::codes::MESH_FORMAT_UNKNOWN))?,
    };
    let only = if !p.elements.is_empty() {
        Some(p.elements.iter().map(|i| i.0).collect())
    } else if p.selection {
        let selected: std::collections::BTreeSet<_> = ctx.selection().iter().copied().collect();
        if selected.is_empty() {
            return Err(Error::new(crate::error::codes::MESH_SELECTION_EMPTY));
        }
        Some(selected)
    } else {
        None
    };
    let mtl_path = path.with_extension("mtl");
    let mtl_name = mtl_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    let options = MeshOptions {
        format,
        unit: match p.unit.unwrap_or(UnitParam::Meter) {
            UnitParam::Meter => Unit::Meter,
            UnitParam::Centimeter => Unit::Centimeter,
            UnitParam::Millimeter => Unit::Millimeter,
            UnitParam::Foot => Unit::Foot,
            UnitParam::Inch => Unit::Inch,
        },
        only,
        hidden: p.hidden.iter().map(|i| i.0).collect(),
        merge: p.merge,
        materials: p.materials,
        mtl_name,
        application: format!("archbuilder {}", env!("CARGO_PKG_VERSION")),
        timestamp: iso8601(ctx.now_unix_millis()),
        file_name: file.clone(),
        limits: *ctx.limits(),
        ..MeshOptions::default()
    };
    let export = export_mesh(ctx.snapshot(), &options)?;
    let report = serde_json::to_value(&export.report)
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    let io = |e| Error::new(arc_command::error::codes::PATH_IO).with_source(e);
    let mut target = ctx.create_write(path)?;
    target
        .write_all(&export.bytes)
        .and_then(|()| target.flush())
        .map_err(io)?;
    let materials_file = match &export.mtl {
        Some(mtl) => {
            let mut target = ctx.create_write(&mtl_path)?;
            target
                .write_all(mtl)
                .and_then(|()| target.flush())
                .map_err(io)?;
            Some(options.mtl_name.clone())
        }
        None => None,
    };
    let count = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
    Ok(MeshExported {
        file,
        materials_file,
        bytes: count(export.bytes.len()),
        elements: count(export.elements),
        triangles: count(export.triangles),
        bounds: export.bounds,
        dropped: export.report.summary.dropped,
        approximated: export.report.summary.approximated,
        report,
    })
}

/// Registers `archbuilder.file.export_ifc` (permission classes `read` and `file_write`, not
/// undoable) and the two shortcut file commands into `registry`.
///
/// # Errors
/// `cmd.duplicate` when the identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(EXPORT_IFC, LABEL.message_id, DESCRIPTION.message_id, export)
            .with_undo(UndoBehavior::NotUndoable)
            .with_permissions([Permission::Read, Permission::FileWrite]),
    )?;
    registry.register(
        CommandSpec::new(
            EXPORT_MESH,
            MESH_LABEL.message_id,
            MESH_DESCRIPTION.message_id,
            export_mesh_command,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read, Permission::FileWrite]),
    )?;
    register_shortcuts(registry)?;
    crate::import::register(registry)?;
    crate::standards::register(registry)
}

/// The identifier of the command that writes the shortcut table to a file.
pub const EXPORT_SHORTCUTS: &str = "archbuilder.shortcuts.export";
/// The identifier of the command that reads a shortcut table from a file.
pub const IMPORT_SHORTCUTS: &str = "archbuilder.shortcuts.import";
/// The largest shortcut file read or written, in bytes.
pub const MAX_SHORTCUT_BYTES: usize = 64 * 1024;

const EXPORT_SHORTCUTS_LABEL: Message = Message {
    message_id: "ab-io-command-export-shortcuts-label",
};
const EXPORT_SHORTCUTS_DESCRIPTION: Message = Message {
    message_id: "ab-io-command-export-shortcuts-description",
};
const IMPORT_SHORTCUTS_LABEL: Message = Message {
    message_id: "ab-io-command-import-shortcuts-label",
};
const IMPORT_SHORTCUTS_DESCRIPTION: Message = Message {
    message_id: "ab-io-command-import-shortcuts-description",
};

/// Parameters of `archbuilder.shortcuts.export`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportShortcuts {
    /// The file to write, inside the session's write roots.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
    /// The text of the table, as the application's exported file has it.
    #[schemars(length(max = 65536))]
    pub text: String,
}

/// Parameters of `archbuilder.shortcuts.import`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportShortcuts {
    /// The file to read, inside the session's read roots.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
}

/// What `archbuilder.shortcuts.import` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ImportedShortcuts {
    /// The text of the file.
    pub text: String,
}

fn export_shortcuts(ctx: &mut Context<'_>, p: ExportShortcuts) -> Result<()> {
    if p.text.len() > MAX_SHORTCUT_BYTES {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let mut target = ctx.create_write(Path::new(&p.path))?;
    target
        .write_all(p.text.as_bytes())
        .and_then(|()| target.flush())
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))
}

fn import_shortcuts(ctx: &mut Context<'_>, p: ImportShortcuts) -> Result<ImportedShortcuts> {
    let file = ctx.open_read(Path::new(&p.path))?;
    let mut bytes = Vec::new();
    // One byte more than the limit tells a file that is too long from one that fits.
    file.take(MAX_SHORTCUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error::new(arc_command::error::codes::PATH_IO).with_source(e))?;
    if bytes.len() > MAX_SHORTCUT_BYTES {
        return Err(Error::new(arc_command::error::codes::PARAMS_INVALID));
    }
    let text = String::from_utf8(bytes)
        .map_err(|e| Error::new(arc_command::error::codes::PARAMS_INVALID).with_source(e))?;
    Ok(ImportedShortcuts { text })
}

/// Registers `archbuilder.shortcuts.export` and `archbuilder.shortcuts.import` (not undoable;
/// they change nothing in the document) into `registry`.
fn register_shortcuts(registry: &mut Registry) -> Result<()> {
    registry.register(
        CommandSpec::new(
            EXPORT_SHORTCUTS,
            EXPORT_SHORTCUTS_LABEL.message_id,
            EXPORT_SHORTCUTS_DESCRIPTION.message_id,
            export_shortcuts,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read, Permission::FileWrite]),
    )?;
    registry.register(
        CommandSpec::new(
            IMPORT_SHORTCUTS,
            IMPORT_SHORTCUTS_LABEL.message_id,
            IMPORT_SHORTCUTS_DESCRIPTION.message_id,
            import_shortcuts,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read, Permission::FileRead]),
    )
}
