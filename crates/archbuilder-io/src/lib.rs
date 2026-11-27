//! The exchange mappings of ArchBuilder, headless: [`ifc`] writes a document as an IFC4 file with a
//! loss report, [`command`] is the export command (`archbuilder.file.export_ifc`) and [`time`]
//! formats the time stamp a caller gives the export. The IFC writer itself is `arc-io-ifc`; this
//! crate maps ArchBuilder objects to its neutral description and never reads a clock or a path.
//!
//! No function in this crate panics.

pub mod command;
pub mod error;
pub mod ifc;
pub mod mesh;
pub mod standards;
pub mod time;
mod validate;

pub use command::register;
pub use ifc::{Export, ExportOptions, export_ifc};
