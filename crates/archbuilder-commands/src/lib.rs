//! The commands and document type set of ArchBuilder, headless.
//!
//! [`types`] is the document type set: the spatial structure types of `archbuilder-model`.
//! [`registry`] is the command set ([`structure`]): `archbuilder.project.new`,
//! `archbuilder.storey.add`, `archbuilder.storey.rename`, `archbuilder.storey.delete`,
//! `archbuilder.building.rename`, `archbuilder.level.set_elevation`, `archbuilder.object.rename`,
//! `archbuilder.object.select` (selection only, no transaction) and `archbuilder.property.set`;
//! [`wall`]: `archbuilder.wall.create`, `archbuilder.wall.delete`, `archbuilder.wall.join_style`,
//! `archbuilder.wall.disallow_join`, `archbuilder.wall.allow_join` and `archbuilder.object.delete`
//! (the selected walls and inserts, else the selected storey); [`insert`]: `archbuilder.door.place`,
//! `archbuilder.window.place`, `archbuilder.insert.flip_hand` and `archbuilder.insert.flip_facing`; [`floor`]:
//! `archbuilder.floor.create` and `archbuilder.floor.add_hole`; [`modify`]: `archbuilder.modify.move`,
//! `copy`, `rotate` and `mirror`; [`edit`]: align, offset, trim, split, merge, arrays, pin and unpin; [`room`]: `archbuilder.room.place` and `archbuilder.room.place_all`; [`stair`]: `archbuilder.stair.create`; [`roof`]: `archbuilder.roof.create`, `set_edge`, `move_vertex`, `move_edge`, `archbuilder.wall.attach_top` and `detach_top`; [`ceiling`]: `archbuilder.ceiling.create` and `set_slope`; [`opening`]: `archbuilder.shaft.create` and `archbuilder.vertical_opening.create` (`archbuilder.opening.place` is in [`insert`]); [`element_type`]: `archbuilder.type.set`, `duplicate`,
//! `delete`, `assign`, `match` and `upgrade`; [`view`]: `archbuilder.view.duplicate`, `rename`, `delete`,
//! `hide_elements`, `show_elements` and `ensure_defaults` (plan views); [`selection`]:
//! `archbuilder.set.create`, `update`, `rename`, `delete` and `select` (selection sets); [`stats`]:
//! `archbuilder.model.statistics` (counts of the model, derived).
//! Every change reaches a document through one of them, in one
//! transaction, with the model's validation rules applied.
//!
//! No function in this crate panics.

pub mod ceiling;
pub mod clip;
pub mod edit;
pub mod element_type;
pub mod error;
pub mod floor;
pub mod floor_edge;
pub mod floor_slope;
pub mod insert;
pub mod modify;
pub mod opening;
pub mod roof;
pub mod roof_extrusion;
pub mod room;
pub mod selection;
pub mod stair;
pub mod stats;
pub mod structure;
pub mod view;
pub mod wall;

use std::sync::Arc;

use arc_base::Result;
use arc_command::headless::Product;
use arc_command::{Clock, Registry};
use arc_doc::graph::Evaluators;
use arc_doc::package::LockOwner;
use arc_doc::{Document, TypeRegistry};

/// A length parameter as a quantity. A bare number, a JSON number or a text without a unit
/// such as `"0.35"`, is meters on every surface; a text with a unit says its own.
///
/// # Errors
/// The errors of [`arc_command::Context::quantity`] for any other text.
pub(crate) fn length(
    ctx: &arc_command::Context<'_>,
    param: &arc_command::QuantityParam,
) -> Result<arc_base::Quantity> {
    if let arc_command::QuantityParam::Text(text) = param
        && let Ok(meters) = text.trim().parse::<f64>()
        && meters.is_finite()
    {
        return Ok(arc_base::Quantity::new(meters, arc_base::Dimension::LENGTH));
    }
    ctx.quantity(param, arc_base::Dimension::LENGTH)
}

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
    wall::register(&mut registry)?;
    insert::register(&mut registry)?;
    floor::register(&mut registry)?;
    floor_edge::register(&mut registry)?;
    floor_slope::register(&mut registry)?;
    modify::register(&mut registry)?;
    edit::register(&mut registry)?;
    room::register(&mut registry)?;
    stair::register(&mut registry)?;
    roof::register(&mut registry)?;
    roof_extrusion::register(&mut registry)?;
    ceiling::register(&mut registry)?;
    opening::register(&mut registry)?;
    element_type::register(&mut registry)?;
    view::register(&mut registry)?;
    selection::register(&mut registry)?;
    stats::register(&mut registry)?;
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
