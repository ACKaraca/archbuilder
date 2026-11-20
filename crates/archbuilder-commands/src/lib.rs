//! The commands and document type set of ArchBuilder, headless.
//!
//! [`types`] is the document type set: the spatial structure types of `archbuilder-model`.
//! [`registry`] is the command set ([`structure`]): `archbuilder.project.new`,
//! `archbuilder.storey.add`, `archbuilder.storey.rename`, `archbuilder.storey.delete`,
//! `archbuilder.building.rename`, `archbuilder.level.set_elevation`, `archbuilder.object.rename`,
//! `archbuilder.object.select` (selection only, no transaction) and `archbuilder.property.set`. Every change reaches a document through one of them, in one
//! transaction, with the model's validation rules applied.
//!
//! No function in this crate panics.

pub mod error;
pub mod structure;

use std::sync::Arc;

use arc_base::Result;
use arc_command::headless::Product;
use arc_command::{Clock, Registry};
use arc_doc::graph::Evaluators;
use arc_doc::package::LockOwner;
use arc_doc::{Document, TypeRegistry};

/// The name the product's headless runner reports.
pub const PRODUCT_NAME: &str = "archbuilder";

/// The document type set.
///
/// # Errors
/// The registry's errors; none occur for these declarations.
pub fn types() -> Result<TypeRegistry> {
    archbuilder_model::types()
}

/// The command set.
///
/// # Errors
/// `cmd.duplicate` when a command is registered twice; it is not for this set.
pub fn registry() -> Result<Registry> {
    let mut registry = Registry::new();
    structure::register(&mut registry)?;
    Ok(registry)
}

/// The product of the headless runner: the type set and the command set.
///
/// # Errors
/// The errors of [`types`] and [`registry`].
pub fn product(clock: Arc<dyn Clock>, owner: LockOwner) -> Result<Product> {
    Ok(Product {
        name: PRODUCT_NAME.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        types: Arc::new(types()?),
        evaluators: Arc::new(Evaluators::new()),
        registry: registry()?,
        clock,
        owner,
        last: None,
    })
}

/// Adds the model's commit validator to `document`. The headless runner and the tests install it;
/// the application's own documents rely on the commands running the same rules.
pub fn install(document: &Document) {
    archbuilder_model::validate::install(document);
}
