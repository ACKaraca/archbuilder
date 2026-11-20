//! The ArchBuilder document model, headless: the object types of the spatial structure
//! ([`types`]) with their validation rules ([`validate`]), readers over a snapshot
//! ([`spatial`]), the project length unit ([`units`]), the wall with its derived solid
//! ([`wall`]) and the doors and windows a wall hosts ([`insert`]).
//!
//! The structure follows the domain model document: a project contains a site, the site
//! contains buildings, a building contains storeys, and the levels of a building are datums that
//! storeys refer to. Records store authored data only, lengths in meters.
//!
//! No function in this crate panics.

pub mod error;
pub mod floor;
pub mod insert;
pub mod join;
pub mod names;
pub mod spatial;
pub mod types;
pub mod units;
pub mod validate;
pub mod wall;

use arc_base::Result;
use arc_doc::TypeRegistry;

/// The names of the objects a new project starts with, in the interface language of the person
/// who creates it. They are document content, written once by the new-project command and edited
/// by the user afterwards, not interface text, so they are stored as created.
pub mod defaults {
    /// The project (English).
    pub const PROJECT: &str = "Untitled Project";
    /// The site (English).
    pub const SITE: &str = "Site";
    /// The building (English).
    pub const BUILDING: &str = "Building";
    /// The storeys a new project starts with, from the ground up (English).
    pub const STOREYS: [&str; 3] = ["Ground Floor", "Floor 1", "Floor 2"];
    /// The distance between the storeys of a new project and the default distance of a storey
    /// added above the highest one, in meters.
    pub const STOREY_HEIGHT: f64 = 3.0;

    /// The names of a new project in one language.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Names {
        /// The project.
        pub project: &'static str,
        /// The site.
        pub site: &'static str,
        /// The building.
        pub building: &'static str,
        /// The storeys, from the ground up.
        pub storeys: [&'static str; 3],
    }

    const ENGLISH: Names = Names {
        project: PROJECT,
        site: SITE,
        building: BUILDING,
        storeys: STOREYS,
    };

    const TURKISH: Names = Names {
        project: "Adsız Proje",
        site: "Arsa",
        building: "Bina",
        storeys: ["Zemin Kat", "1. Kat", "2. Kat"],
    };

    fn turkish(language: &str) -> bool {
        language
            .get(..2)
            .is_some_and(|l| l.eq_ignore_ascii_case("tr"))
    }

    /// The names of a new project for the language tag `language` (`tr`, `tr-TR`, `en`, ...);
    /// English for any other language and for none.
    #[must_use]
    pub fn names(language: &str) -> Names {
        if turkish(language) { TURKISH } else { ENGLISH }
    }

    /// The name of the wall numbered `n` on a storey in `language`: `Wall 3`, `Duvar 3`.
    #[must_use]
    pub fn wall(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Duvar {n}")
        } else {
            format!("Wall {n}")
        }
    }

    /// The name of the door or window numbered `n` in `language`: `Door 3`, `Kapı 3`.
    #[must_use]
    pub fn insert(language: &str, window: bool, n: usize) -> String {
        match (turkish(language), window) {
            (true, false) => format!("Kapı {n}"),
            (true, true) => format!("Pencere {n}"),
            (false, false) => format!("Door {n}"),
            (false, true) => format!("Window {n}"),
        }
    }

    /// The name of the floor slab numbered `n` on a storey in `language`: `Slab 3`, `Döşeme 3`.
    #[must_use]
    pub fn slab(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Döşeme {n}")
        } else {
            format!("Slab {n}")
        }
    }

    /// The name of the storey numbered `n` above the ground floor in `language`: `Floor 3`,
    /// `3. Kat`.
    #[must_use]
    pub fn floor(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("{n}. Kat")
        } else {
            format!("Floor {n}")
        }
    }
}

/// Registers the object types of the model in `registry`.
///
/// # Errors
/// The registry's errors; none occur for these declarations.
pub fn register_types(registry: &mut TypeRegistry) -> Result<()> {
    types::register(registry)
}

/// The document type set of the model.
///
/// # Errors
/// The registry's errors; none occur for these declarations.
pub fn types() -> Result<TypeRegistry> {
    let mut registry = TypeRegistry::new();
    register_types(&mut registry)?;
    Ok(registry)
}
