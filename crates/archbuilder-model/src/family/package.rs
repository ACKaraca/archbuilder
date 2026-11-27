//! `.archbfam` packages (ADR 0009, section 3): an Arc document package like `.archb` that holds
//! a manifest record and one record per published definition version. A published record keeps
//! the definition's canonical text and its checksum (the BLAKE3 hash of that text, which is
//! what an asset of the text would hash to); reading verifies the checksum, the text, the
//! function set, the test cases and the dependencies, and refuses with a report.
//!
//! shortcut: the published text is a string field of its record, not an asset of the package;
//! the checksum is the same, so moving it to an asset later changes one field.

use std::collections::BTreeMap;

use arc_base::{Error, ObjectId, Result};
use arc_doc::{
    Bound, FieldDecl, Object, Snapshot, TypeDecl, TypeKey, TypeRegistry, Value, ValueType,
};
use arc_doc::{Schema, SchemaVersion};

use super::def::{Definition, Version, limits};
use super::text::checksum;
use crate::error::codes;
use crate::spatial;
use crate::types::text;

/// The type key of the manifest record.
pub const MANIFEST: &str = "archbuilder.fam.manifest";
/// The type key of a published definition record.
pub const DEFINITION: &str = "archbuilder.fam.definition";

/// The most dependencies of a package.
pub const MAX_DEPENDENCIES: usize = 256;

/// Field positions of the manifest record.
pub mod manifest {
    /// The package name.
    pub const NAME: usize = 0;
    /// The package version.
    pub const VERSION: usize = 1;
    /// The description.
    pub const DESCRIPTION: usize = 2;
    /// The SPDX license.
    pub const LICENSE: usize = 3;
    /// The source.
    pub const SOURCE: usize = 4;
    /// The author.
    pub const AUTHOR: usize = 5;
    /// The least product version.
    pub const MIN_PRODUCT: usize = 6;
    /// The dependencies, `name>=version`.
    pub const DEPENDENCIES: usize = 7;
}

/// Field positions of a published definition record.
pub mod published {
    /// The family identifier.
    pub const FAMILY: usize = 0;
    /// The version.
    pub const VERSION: usize = 1;
    /// The name.
    pub const NAME: usize = 2;
    /// The category (for the index).
    pub const CATEGORY: usize = 3;
    /// The checksum.
    pub const CHECKSUM: usize = 4;
    /// The canonical text.
    pub const TEXT: usize = 5;
}

fn string(name: &str, max: u64, optional: bool) -> FieldDecl {
    let f = FieldDecl::new(name, ValueType::String)
        .with_bound(Bound::Length { max })
        .in_aspect("identity");
    if optional { f.optional() } else { f }
}

/// Registers the package records.
///
/// # Errors
/// The registry's errors; none occur for these declarations.
pub fn register(registry: &mut TypeRegistry) -> Result<()> {
    let manifest = vec![
        string("name", 256, false),
        string("version", 32, false),
        string("description", 4096, true),
        string("license", 128, true),
        string("source", 1024, true),
        string("author", 256, true),
        string("min_product", 32, true),
        FieldDecl::new("dependencies", ValueType::List(Box::new(ValueType::String)))
            .with_bound(Bound::Length {
                max: MAX_DEPENDENCIES as u64,
            })
            .optional()
            .in_aspect("identity"),
    ];
    let definition = vec![
        string("family", 64, false),
        string("version", 32, false),
        string("name", 256, false),
        string("category", 64, false),
        string("checksum", 64, false),
        string("text", limits::TEXT_BYTES as u64, false),
    ];
    for (key, fields) in [(MANIFEST, manifest), (DEFINITION, definition)] {
        registry.register(TypeDecl::new(
            TypeKey::new(key)?,
            Schema::new(SchemaVersion::new(1, 0), fields),
        ))?;
    }
    Ok(())
}

/// A dependency of a package: a package name and the least version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// The package name.
    pub name: String,
    /// The least version.
    pub minimum: Version,
}

impl Dependency {
    /// A dependency from `name>=major.minor.patch`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (name, v) = text.split_once(">=")?;
        let name = name.trim();
        (!name.is_empty()).then_some(())?;
        Some(Self {
            name: name.to_owned(),
            minimum: Version::parse(v.trim())?,
        })
    }
}

/// The manifest of a package.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Manifest {
    /// The package name.
    pub name: String,
    /// The package version.
    pub version: Version,
    /// The description.
    pub description: String,
    /// The SPDX license.
    pub license: String,
    /// The source.
    pub source: String,
    /// The author.
    pub author: String,
    /// The least product version, `major.minor.patch`; empty for any.
    pub min_product: String,
    /// The dependencies, in order.
    pub dependencies: Vec<Dependency>,
}

impl Manifest {
    /// The record values.
    #[must_use]
    pub fn values(&self) -> Vec<Option<Value>> {
        let s = |v: &str| Some(Value::String(v.to_owned()));
        vec![
            s(&self.name),
            s(&self.version.to_string()),
            s(&self.description),
            s(&self.license),
            s(&self.source),
            s(&self.author),
            s(&self.min_product),
            Some(Value::List(
                self.dependencies
                    .iter()
                    .map(|d| Value::String(format!("{}>={}", d.name, d.minimum)))
                    .collect(),
            )),
        ]
    }

    /// The manifest stored in `object`; `None` when its name or version is unreadable.
    #[must_use]
    pub fn of(object: &Object) -> Option<Self> {
        let v = &object.values;
        let t = |at| text(v, at).unwrap_or_default().to_owned();
        let dependencies = match v.get(manifest::DEPENDENCIES) {
            Some(Some(Value::List(items))) => items
                .iter()
                .take(MAX_DEPENDENCIES)
                .filter_map(|i| match i {
                    Value::String(s) => Dependency::parse(s),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        Some(Self {
            name: t(manifest::NAME),
            version: Version::parse(&t(manifest::VERSION))?,
            description: t(manifest::DESCRIPTION),
            license: t(manifest::LICENSE),
            source: t(manifest::SOURCE),
            author: t(manifest::AUTHOR),
            min_product: t(manifest::MIN_PRODUCT),
            dependencies,
        })
    }
}

/// One published version of a family.
#[derive(Debug, Clone, PartialEq)]
pub struct Published {
    /// The record, when it is read from a document.
    pub record: Option<ObjectId>,
    /// The family identifier.
    pub family: ObjectId,
    /// The version.
    pub version: Version,
    /// The name.
    pub name: String,
    /// The category.
    pub category: String,
    /// The checksum as stored.
    pub checksum: String,
    /// The canonical text.
    pub text: String,
}

impl Published {
    /// The record values.
    #[must_use]
    pub fn values(&self) -> Vec<Option<Value>> {
        let s = |v: &str| Some(Value::String(v.to_owned()));
        vec![
            s(&self.family.to_string()),
            s(&self.version.to_string()),
            s(&self.name),
            s(&self.category),
            s(&self.checksum),
            s(&self.text),
        ]
    }

    /// The published version stored in `object`.
    #[must_use]
    pub fn of(object: &Object) -> Option<Self> {
        let v = &object.values;
        let t = |at| text(v, at).unwrap_or_default().to_owned();
        Some(Self {
            record: Some(object.id),
            family: t(published::FAMILY).parse().ok()?,
            version: Version::parse(&t(published::VERSION))?,
            name: t(published::NAME),
            category: t(published::CATEGORY),
            checksum: t(published::CHECKSUM),
            text: t(published::TEXT),
        })
    }

    /// The definition the text holds.
    ///
    /// # Errors
    /// The errors of [`Definition::from_text`].
    pub fn definition(&self) -> Result<Definition> {
        Definition::from_text(&self.text)
    }
}

/// What a package holds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Contents {
    /// The manifest, when there is a readable one.
    pub manifest: Option<Manifest>,
    /// The published definitions, by family, then version.
    pub definitions: Vec<Published>,
}

/// The contents of the package document `snapshot`.
#[must_use]
pub fn read(snapshot: &Snapshot) -> Contents {
    let manifest = spatial::objects(snapshot, MANIFEST)
        .first()
        .and_then(|o| Manifest::of(o));
    let mut definitions: Vec<Published> = spatial::objects(snapshot, DEFINITION)
        .iter()
        .filter_map(|o| Published::of(o))
        .collect();
    definitions.sort_by_key(|p| (p.family, p.version));
    Contents {
        manifest,
        definitions,
    }
}

/// One reason a package or a definition in it is refused.
#[derive(Debug)]
pub struct Refusal {
    /// The package or `name version` of the definition it is about.
    pub subject: String,
    /// The error, with its arguments.
    pub error: Error,
}

/// The packages known to resolve dependencies: name and version.
pub type Available = [(String, Version)];

/// Checks the package: the manifest, the product version, the dependencies against `available`,
/// and each definition (checksum, text, function set, identity of record and text, tests, one
/// checksum per family and version). An empty report means the package may be loaded.
#[must_use]
pub fn verify(contents: &Contents, available: &Available) -> Vec<Refusal> {
    let mut out = Vec::new();
    let refuse = |out: &mut Vec<Refusal>, subject: &str, error: Error| {
        out.push(Refusal {
            subject: subject.to_owned(),
            error,
        });
    };
    let Some(m) = &contents.manifest else {
        refuse(
            &mut out,
            "package",
            Error::new(codes::FAMILY_PACKAGE).with_arg("reason", "manifest"),
        );
        return out;
    };
    if !m.min_product.is_empty() {
        let current = Version::parse(env!("CARGO_PKG_VERSION"));
        if Version::parse(&m.min_product).is_none() || Version::parse(&m.min_product) > current {
            refuse(
                &mut out,
                &m.name,
                Error::new(codes::FAMILY_PACKAGE).with_arg("reason", "product_version"),
            );
        }
    }
    for d in &m.dependencies {
        if !available
            .iter()
            .any(|(n, v)| *n == d.name && *v >= d.minimum)
        {
            refuse(
                &mut out,
                &m.name,
                Error::new(codes::FAMILY_DEPENDENCY).with_arg("name", d.name.as_str()),
            );
        }
    }
    let mut seen: BTreeMap<(ObjectId, Version), &str> = BTreeMap::new();
    for p in &contents.definitions {
        let subject = format!("{} {}", p.name, p.version);
        if checksum(&p.text) != p.checksum {
            refuse(
                &mut out,
                &subject,
                Error::new(codes::FAMILY_CHECKSUM).with_arg("family", p.name.as_str()),
            );
            continue;
        }
        if let Some(other) = seen.insert((p.family, p.version), &p.checksum)
            && other != p.checksum
        {
            refuse(
                &mut out,
                &subject,
                Error::new(codes::FAMILY_CONFLICT).with_arg("family", p.name.as_str()),
            );
        }
        match p.definition() {
            Err(e) => refuse(&mut out, &subject, e),
            Ok(d) => {
                if d.header.id != p.family || d.header.version != p.version {
                    refuse(
                        &mut out,
                        &subject,
                        Error::new(codes::FAMILY_PACKAGE).with_arg("reason", "record_text"),
                    );
                }
                if let Err(e) = d.run_tests() {
                    refuse(&mut out, &subject, e);
                }
            }
        }
    }
    out
}

/// How large a change between two versions of a definition is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    /// Geometry or metadata only.
    Patch,
    /// Parameters added.
    Minor,
    /// Parameters removed or retyped, or hosting changed.
    Major,
}

/// The step `new` is from `old`: major when a parameter is removed, retyped or renamed (by
/// identifier) or the hosting changed, minor when one is added, else patch.
#[must_use]
pub fn step(old: &Definition, new: &Definition) -> Step {
    let changed = old.header.hosting != new.header.hosting
        || old.parameters.iter().any(|o| {
            new.parameters
                .iter()
                .find(|n| n.id == o.id)
                .is_none_or(|n| n.data != o.data || n.key != o.key)
        });
    if changed {
        Step::Major
    } else if new
        .parameters
        .iter()
        .any(|n| !old.parameters.iter().any(|o| o.id == n.id))
    {
        Step::Minor
    } else {
        Step::Patch
    }
}

fn chosen(old: Version, new: Version) -> Option<Step> {
    if new <= old {
        None
    } else if new.major != old.major {
        Some(Step::Major)
    } else if new.minor != old.minor {
        Some(Step::Minor)
    } else {
        Some(Step::Patch)
    }
}

/// Publishes `definition` at `version`: validates it, runs all its test cases, refuses a version
/// that is not newer than `previous` or smaller than the change needs, and returns the
/// published text with its checksum.
///
/// # Errors
/// The errors of [`Definition::validate`] and [`Definition::run_tests`];
/// `ab.family.version_invalid` with the reason `not_newer` or `step`.
pub fn publish(
    definition: &Definition,
    previous: Option<&Definition>,
    version: Version,
) -> Result<Published> {
    let mut d = definition.clone();
    d.header.version = version;
    d.validate()?;
    d.run_tests()?;
    if let Some(old) = previous {
        let version_error = |reason: &str| {
            Error::new(codes::FAMILY_VERSION)
                .with_arg("version", version.to_string())
                .with_arg("reason", reason)
        };
        let Some(got) = chosen(old.header.version, version) else {
            return Err(version_error("not_newer"));
        };
        if got < step(old, &d) {
            return Err(version_error("step"));
        }
    }
    let text = d.to_text()?;
    Ok(Published {
        record: None,
        family: d.header.id,
        version,
        name: d.header.name.clone(),
        category: d.header.category.name().to_owned(),
        checksum: checksum(&text),
        text,
    })
}
