//! Family documents (ADR 0009, section 3): the definition ([`def`]) with its validation and
//! limits, its canonical text and checksum ([`text`]), the evaluator ([`eval`]) and the
//! `.archbfam` package ([`package`]). Families are declarative: a definition holds authored data
//! and formulas of the bounded expression language, never executable code.

pub mod def;
pub mod eval;
pub mod fixtures;
pub mod loaded;
pub mod package;
pub mod table;
pub mod templates;
pub mod text;

pub use def::{Definition, Version};
pub use eval::{Evaluation, HostCut, Inputs, SharedOut, evaluate};
pub use text::checksum;

/// The role of a shared nested child in [`nested_id`].
pub const NESTED_ROLE: u32 = 0x4641_4D31;

/// The identifier of the shared nested child `key` (`use` or `use#index` for an array copy) of
/// the instance `parent`: a function of the two alone, so it is the same after every
/// regeneration, and a version 8 identifier, which never equals a record's own.
#[must_use]
pub fn nested_id(parent: arc_base::ObjectId, key: &str) -> u128 {
    crate::curtain::child_id(parent, NESTED_ROLE, key)
}
