//! Family documents (ADR 0009, section 3): the definition ([`def`]) with its validation and
//! limits, its canonical text and checksum ([`text`]), the evaluator ([`eval`]) and the
//! `.archbfam` package ([`package`]). Families are declarative: a definition holds authored data
//! and formulas of the bounded expression language, never executable code.

pub mod def;
pub mod eval;
pub mod fixtures;
pub mod package;
pub mod text;

pub use def::{Definition, Version};
pub use eval::{Evaluation, Inputs, evaluate};
pub use text::checksum;
