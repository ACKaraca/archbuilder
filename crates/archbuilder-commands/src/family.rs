//! The commands of family packages (ADR 0009, unit 6): `archbuilder.family.manifest` sets the
//! manifest of a `.archbfam` document, `archbuilder.family.publish` publishes a definition's
//! canonical text as a new version (all test cases run first), and `archbuilder.family.check`
//! reports what a package would be refused for. Each change is one transaction.

use arc_base::{Error, Result};
use arc_command::{CommandSpec, Context, IdParam, Permission, Registry, UndoBehavior};
use arc_doc::Snapshot;
use archbuilder_model::family::def::{Definition, Version};
use archbuilder_model::family::package::{self, Dependency, Manifest, Published, Refusal};
use archbuilder_model::spatial;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::codes;
use crate::structure::create;

/// A message of the catalog, named by identifier.
struct MessageId {
    message_id: &'static str,
}

/// The identifier of the manifest command.
pub const FAMILY_MANIFEST: &str = "archbuilder.family.manifest";
/// The identifier of the publish command.
pub const FAMILY_PUBLISH: &str = "archbuilder.family.publish";
/// The identifier of the check command (read-only).
pub const FAMILY_CHECK: &str = "archbuilder.family.check";

/// Parameters of `archbuilder.family.manifest`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetManifest {
    /// The package name.
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
    /// The package version, `major.minor.patch`.
    pub version: String,
    /// The description.
    #[serde(default)]
    #[schemars(length(max = 4096))]
    pub description: String,
    /// The SPDX license.
    #[serde(default)]
    #[schemars(length(max = 128))]
    pub license: String,
    /// The source.
    #[serde(default)]
    #[schemars(length(max = 1024))]
    pub source: String,
    /// The author.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub author: String,
    /// The least product version, `major.minor.patch`.
    #[serde(default)]
    pub min_product: String,
    /// Dependencies as `name>=major.minor.patch`.
    #[serde(default)]
    #[schemars(length(max = 256))]
    pub dependencies: Vec<String>,
}

/// Parameters of `archbuilder.family.publish`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublishFamily {
    /// The definition's text (the canonical text form).
    #[schemars(length(max = 4194304))]
    pub text: String,
    /// The version to publish, `major.minor.patch`.
    pub version: String,
}

/// What `archbuilder.family.publish` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PublishedFamily {
    /// The published record.
    pub record: IdParam,
    /// The family identifier.
    pub family: String,
    /// The version.
    pub version: String,
    /// The checksum of the published text.
    pub checksum: String,
}

/// Parameters of `archbuilder.family.check`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckPackage {
    /// Packages that are available to resolve dependencies, as `name@major.minor.patch`.
    #[serde(default)]
    pub available: Vec<String>,
}

/// One refusal in a report.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct RefusalRow {
    /// The package, or the definition's name and version.
    pub subject: String,
    /// The error code.
    pub code: String,
    /// The error's arguments as text.
    pub arguments: std::collections::BTreeMap<String, String>,
}

/// One definition in a report.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct FamilyRow {
    /// The family identifier.
    pub family: String,
    /// The name.
    pub name: String,
    /// The version.
    pub version: String,
    /// The checksum.
    pub checksum: String,
}

/// What `archbuilder.family.check` returns and the CLI prints.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PackageReport {
    /// The package name and version, when there is a manifest.
    pub package: Option<String>,
    /// Whether the package may be loaded.
    pub ok: bool,
    /// The definitions it holds.
    pub families: Vec<FamilyRow>,
    /// Why it is refused.
    pub refusals: Vec<RefusalRow>,
}

fn row(r: &Refusal) -> RefusalRow {
    RefusalRow {
        subject: r.subject.clone(),
        code: r.error.code().to_owned(),
        arguments: r
            .error
            .arguments()
            .iter()
            .map(|(k, v)| {
                let text = match v {
                    arc_base::error::Value::Text(t) => t.clone(),
                    other => format!("{other:?}"),
                };
                (k.to_string(), text)
            })
            .collect(),
    }
}

/// The report of the package `snapshot` against the packages `available`.
#[must_use]
pub fn report(snapshot: &Snapshot, available: &[(String, Version)]) -> PackageReport {
    let contents = package::read(snapshot);
    let refusals: Vec<RefusalRow> = package::verify(&contents, available)
        .iter()
        .map(row)
        .collect();
    PackageReport {
        package: contents
            .manifest
            .as_ref()
            .map(|m| format!("{} {}", m.name, m.version)),
        ok: refusals.is_empty(),
        families: contents
            .definitions
            .iter()
            .map(|p| FamilyRow {
                family: p.family.to_string(),
                name: p.name.clone(),
                version: p.version.to_string(),
                checksum: p.checksum.clone(),
            })
            .collect(),
        refusals,
    }
}

/// The `name@version` list of `available`.
///
/// # Errors
/// `ab.cmd.value_invalid` for an entry that is not `name@major.minor.patch`.
pub fn available_of(list: &[String]) -> Result<Vec<(String, Version)>> {
    list.iter()
        .map(|a| {
            let (n, v) = a
                .split_once('@')
                .ok_or_else(|| Error::new(codes::VALUE_INVALID))?;
            Ok((
                n.to_owned(),
                Version::parse(v).ok_or_else(|| Error::new(codes::VALUE_INVALID))?,
            ))
        })
        .collect()
}

fn version(text: &str) -> Result<Version> {
    Version::parse(text).ok_or_else(|| Error::new(codes::VALUE_INVALID))
}

fn set_manifest(ctx: &mut Context<'_>, p: SetManifest) -> Result<()> {
    if p.name.trim().is_empty() {
        return Err(Error::new(codes::VALUE_INVALID));
    }
    let mut dependencies = Vec::new();
    for d in &p.dependencies {
        dependencies.push(Dependency::parse(d).ok_or_else(|| Error::new(codes::VALUE_INVALID))?);
    }
    if !p.min_product.is_empty() {
        version(&p.min_product)?;
    }
    let manifest = Manifest {
        name: p.name.trim().to_owned(),
        version: version(&p.version)?,
        description: p.description,
        license: p.license,
        source: p.source,
        author: p.author,
        min_product: p.min_product,
        dependencies,
    };
    let old: Vec<_> = spatial::objects(ctx.snapshot(), package::MANIFEST)
        .iter()
        .map(|o| o.id)
        .collect();
    for id in old {
        ctx.txn()?.delete(id)?;
    }
    create(ctx, package::MANIFEST, None, manifest.values())?;
    Ok(())
}

/// The latest published definition of `family` in `snapshot`, with its record.
fn latest(snapshot: &Snapshot, family: arc_base::ObjectId) -> Option<Definition> {
    package::read(snapshot)
        .definitions
        .iter()
        .filter(|p| p.family == family)
        .max_by_key(|p| p.version)
        .and_then(|p| p.definition().ok())
}

fn publish(ctx: &mut Context<'_>, p: PublishFamily) -> Result<PublishedFamily> {
    let definition = Definition::from_text(&p.text)?;
    let version = version(&p.version)?;
    let previous = latest(ctx.snapshot(), definition.header.id);
    let done: Published = package::publish(&definition, previous.as_ref(), version)?;
    let id = create(ctx, package::DEFINITION, None, done.values())?;
    Ok(PublishedFamily {
        record: IdParam(id),
        family: done.family.to_string(),
        version: done.version.to_string(),
        checksum: done.checksum,
    })
}

fn check(ctx: &mut Context<'_>, p: CheckPackage) -> Result<PackageReport> {
    Ok(report(ctx.snapshot(), &available_of(&p.available)?))
}

/// Registers the commands of this module.
///
/// # Errors
/// `cmd.duplicate` when an identifier is already registered.
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.register(CommandSpec::new(
        FAMILY_MANIFEST,
        MessageId {
            message_id: "ab-cmd-family-manifest-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-family-manifest-description",
        }
        .message_id,
        set_manifest,
    ))?;
    registry.register(CommandSpec::new(
        FAMILY_PUBLISH,
        MessageId {
            message_id: "ab-cmd-family-publish-label",
        }
        .message_id,
        MessageId {
            message_id: "ab-cmd-family-publish-description",
        }
        .message_id,
        publish,
    ))?;
    registry.register(
        CommandSpec::new(
            FAMILY_CHECK,
            MessageId {
                message_id: "ab-cmd-family-check-label",
            }
            .message_id,
            MessageId {
                message_id: "ab-cmd-family-check-description",
            }
            .message_id,
            check,
        )
        .with_undo(UndoBehavior::NotUndoable)
        .with_permissions([Permission::Read]),
    )?;
    Ok(())
}
