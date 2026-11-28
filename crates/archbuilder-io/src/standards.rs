//! The commands that read another ArchBuilder document: `archbuilder.file.new_from_template`
//! starts the project of the open document from a template file (AB-DATUM-001) and
//! `archbuilder.file.transfer_standards` copies element types and display unit settings from a
//! document file (AB-DATUM-005).
//!
//! The other file is opened read-only through the session's read roots, loaded with this
//! product's types and never changed; the copying is in `archbuilder_commands::transfer`, so
//! each command is one undo step of the open document. An unsaved document that is open in the
//! application is reached by saving it first.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use arc_base::{Error, Result};
use arc_command::{CommandSpec, Context, Permission, Registry};
use arc_doc::LoadOptions;
use arc_doc::Snapshot;
use arc_doc::package::{Mode, Package};
use archbuilder_commands::transfer::{
    Conflict, TemplateApplied, TransferOptions, Transferred, from_template, transfer,
};
use schemars::JsonSchema;
use serde::Deserialize;

/// The identifier of the project-from-template command.
pub const NEW_FROM_TEMPLATE: &str = "archbuilder.file.new_from_template";
/// The identifier of the standards transfer command.
pub const TRANSFER_STANDARDS: &str = "archbuilder.file.transfer_standards";

/// A message of this crate.
struct Message {
    message_id: &'static str,
}

/// Parameters of `archbuilder.file.new_from_template`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateParams {
    /// The template: an `.archb` document, inside the session's read roots.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
}

/// Parameters of `archbuilder.file.transfer_standards`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransferParams {
    /// The document to copy from: an `.archb` file, inside the session's read roots.
    #[schemars(length(min = 1, max = 4095))]
    pub path: String,
    /// The kinds of type to copy, named like `wall_type`, `floor_type`, `door_type`,
    /// `window_type`, `roof_type`, `ceiling_type`, `railing_type`, `ramp_type`; all when absent.
    #[serde(default)]
    #[schemars(length(max = 16))]
    pub kinds: Option<Vec<String>>,
    /// The names of the types to copy; all when absent.
    #[serde(default)]
    #[schemars(length(max = 1000))]
    pub names: Option<Vec<String>>,
    /// What to do with a type whose name this document has: `keep` (default), `overwrite` (an
    /// unused type only) or `rename` (copy under a free name).
    #[serde(default)]
    pub conflict: Option<String>,
    /// The conflict of single types by name, over `conflict`.
    #[serde(default)]
    pub resolutions: Option<BTreeMap<String, String>>,
    /// Whether the display unit settings are copied too; true when absent.
    #[serde(default)]
    pub settings: Option<bool>,
}

/// The snapshot of the document file at `path`, loaded read-only with the types of `registry`.
fn read_document(ctx: &Context<'_>, path: &str) -> Result<Snapshot> {
    let path = Path::new(path);
    // Opening through the session's read roots decides whether the path may be read at all.
    drop(ctx.open_read(path)?);
    let limits = *ctx.limits();
    let package = Package::open(path, Mode::ReadOnly, &limits).map_err(|e| {
        Error::new(archbuilder_commands::error::codes::TEMPLATE_INVALID).with_source(e)
    })?;
    let options = LoadOptions {
        limits,
        ..LoadOptions::default()
    };
    let loaded = package
        .load(Arc::clone(ctx.snapshot().registry()), &options)
        .map_err(|e| {
            Error::new(archbuilder_commands::error::codes::TEMPLATE_INVALID).with_source(e)
        })?;
    Ok(loaded.snapshot)
}

fn new_from_template(ctx: &mut Context<'_>, p: TemplateParams) -> Result<TemplateApplied> {
    ctx.checkpoint()?;
    let source = read_document(ctx, &p.path)?;
    let file = Path::new(&p.path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    from_template(ctx, &source, &file)
}

fn transfer_standards(ctx: &mut Context<'_>, p: TransferParams) -> Result<Transferred> {
    ctx.checkpoint()?;
    let options = TransferOptions {
        kinds: p.kinds.unwrap_or_default(),
        names: p.names.unwrap_or_default(),
        conflict: p
            .conflict
            .as_deref()
            .map_or(Ok(Conflict::Keep), Conflict::parse)?,
        resolutions: p
            .resolutions
            .unwrap_or_default()
            .into_iter()
            .map(|(name, c)| Conflict::parse(&c).map(|c| (name, c)))
            .collect::<Result<_>>()?,
        settings: p.settings.unwrap_or(true),
    };
    let source = read_document(ctx, &p.path)?;
    transfer(ctx, &source, &options)
}

/// Registers `archbuilder.file.new_from_template` and `archbuilder.file.transfer_standards`
/// (permission classes `read`, `edit` and `file_read`; each one undo step) into `registry`.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    let permissions = [Permission::Read, Permission::Edit, Permission::FileRead];
    registry.register(
        CommandSpec::new(
            NEW_FROM_TEMPLATE,
            Message {
                message_id: "ab-io-command-new-from-template-label",
            }
            .message_id,
            Message {
                message_id: "ab-io-command-new-from-template-description",
            }
            .message_id,
            new_from_template,
        )
        .with_permissions(permissions),
    )?;
    registry.register(
        CommandSpec::new(
            TRANSFER_STANDARDS,
            Message {
                message_id: "ab-io-command-transfer-standards-label",
            }
            .message_id,
            Message {
                message_id: "ab-io-command-transfer-standards-description",
            }
            .message_id,
            transfer_standards,
        )
        .with_permissions(permissions),
    )
}
