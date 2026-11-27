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

/// An enumeration stored as a code in a record: its variants, names, codes and reader.
macro_rules! coded {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $code:literal, $text:literal;)+ }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
        }

        impl $name {
            /// Every variant, in code order.
            pub const ALL: [Self; coded!(@count $($variant)+)] = [$(Self::$variant),+];

            /// The name of the variant in the record.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self { $( Self::$variant => $text, )+ }
            }

            /// The code stored in the record.
            #[must_use]
            pub const fn code(self) -> u32 {
                match self { $( Self::$variant => $code, )+ }
            }

            /// The variant of a stored code; the default for a code that names none.
            #[must_use]
            pub fn from_code(code: u32) -> Self {
                Self::ALL.into_iter().find(|v| v.code() == code).unwrap_or_default()
            }

            /// The variant of a stored value.
            #[allow(dead_code, reason = "not every coded enum is read from a field")]
            pub(crate) fn of_field(values: &[Option<::arc_doc::Value>], at: usize) -> Self {
                match values.get(at) {
                    Some(Some(::arc_doc::Value::Enum(code))) => Self::from_code(*code),
                    _ => Self::default(),
                }
            }
        }
    };
    (@count) => { 0 };
    (@count $head:ident $($tail:ident)*) => { 1 + coded!(@count $($tail)*) };
}

pub mod ceiling;
pub mod cleanup;
pub mod coordinates;
pub mod curtain;
pub mod datum;
pub mod dormer;
pub mod element_type;
pub mod elevator;
pub mod error;
pub mod expr;
pub mod floor;
pub mod floor_edge;
pub mod headroom;
pub mod insert;
pub mod join;
pub mod layered;
pub mod names;
pub mod opening;
pub mod pick;
pub mod plan;
pub mod railing;
pub mod ramp;
pub mod roof;
pub mod roof_edge;
pub mod roof_extrusion;
pub mod room;
pub mod selection;
pub mod sketch;
pub mod spatial;
pub mod stair;
pub mod terrain;
pub mod transform;
pub mod types;
pub mod units;
pub mod validate;
pub mod view;
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

    /// The name of the stair numbered `n` on a storey in `language`: `Stair 3`, `Merdiven 3`.
    #[must_use]
    pub fn stair(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Merdiven {n}")
        } else {
            format!("Stair {n}")
        }
    }

    /// The name of the railing numbered `n` on a storey in `language`: `Railing 3`, `Korkuluk 3`.
    #[must_use]
    pub fn railing(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Korkuluk {n}")
        } else {
            format!("Railing {n}")
        }
    }

    /// The name of the ramp numbered `n` on a storey in `language`: `Ramp 3`, `Rampa 3`.
    #[must_use]
    pub fn ramp(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Rampa {n}")
        } else {
            format!("Ramp {n}")
        }
    }

    /// The name of the default railing type in `language`.
    #[must_use]
    pub fn railing_type(language: &str) -> &'static str {
        if turkish(language) {
            "Korkuluk 900"
        } else {
            "Railing 900"
        }
    }

    /// The name of the default ramp type in `language`.
    #[must_use]
    pub fn ramp_type(language: &str) -> &'static str {
        if turkish(language) {
            "Rampa 1:12"
        } else {
            "Ramp 1:12"
        }
    }

    /// The name of the roof numbered `n` on a storey in `language`: `Roof 3`, `Çatı 3`.
    #[must_use]
    pub fn roof(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Çatı {n}")
        } else {
            format!("Roof {n}")
        }
    }

    /// The name of the ceiling numbered `n` on a storey in `language`: `Ceiling 3`, `Tavan 3`.
    #[must_use]
    pub fn ceiling(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Tavan {n}")
        } else {
            format!("Ceiling {n}")
        }
    }

    /// The name of the reference plane numbered `n` in `language`: `Reference plane 3`,
    /// `Referans düzlemi 3`.
    #[must_use]
    pub fn reference_plane(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Referans düzlemi {n}")
        } else {
            format!("Reference plane {n}")
        }
    }

    /// The name of the work plane numbered `n` in `language`: `Work plane 3`, `Çalışma düzlemi 3`.
    #[must_use]
    pub fn work_plane(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Çalışma düzlemi {n}")
        } else {
            format!("Work plane {n}")
        }
    }

    /// The name of the wall opening numbered `n` in `language`: `Opening 3`, `Boşluk 3`.
    #[must_use]
    pub fn wall_opening(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Boşluk {n}")
        } else {
            format!("Opening {n}")
        }
    }

    /// The name of the shaft numbered `n` in `language`: `Shaft 3`, `Şaft 3`.
    #[must_use]
    pub fn shaft(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Şaft {n}")
        } else {
            format!("Shaft {n}")
        }
    }

    /// The name of the vertical opening numbered `n` in `language`: `Cut 3`, `Kesik 3`.
    #[must_use]
    pub fn vertical_opening(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Kesik {n}")
        } else {
            format!("Cut {n}")
        }
    }

    /// The name of the dormer opening numbered `n` in `language`: `Dormer 3`, `Çatı çıkması 3`.
    #[must_use]
    pub fn dormer_opening(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Çatı çıkması {n}")
        } else {
            format!("Dormer {n}")
        }
    }

    /// The name of the roof by extrusion numbered `n` in `language`: `Extruded roof 3`,
    /// `Ekstrüzyon çatı 3`.
    #[must_use]
    pub fn roof_extrusion(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Ekstrüzyon çatı {n}")
        } else {
            format!("Extruded roof {n}")
        }
    }

    /// The name of the elevator numbered `n` in `language`: `Elevator 2`, `Asansör 2`.
    #[must_use]
    pub fn elevator(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Asansör {n}")
        } else {
            format!("Elevator {n}")
        }
    }

    /// The name of the dormer numbered `n` in `language`: `Dormer 2`, `Çatı penceresi 2`.
    #[must_use]
    pub fn dormer(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Çatı penceresi {n}")
        } else {
            format!("Dormer {n}")
        }
    }

    /// The name of the roof edge element of `role` numbered `n` in `language`: `Fascia 1`,
    /// `Saçak silmesi 1`, `Gutter 1`, `Oluk 1`.
    #[must_use]
    pub fn roof_edge(language: &str, gutter: bool, n: usize) -> String {
        match (turkish(language), gutter) {
            (true, true) => format!("Oluk {n}"),
            (true, false) => format!("Saçak silmesi {n}"),
            (false, true) => format!("Gutter {n}"),
            (false, false) => format!("Fascia {n}"),
        }
    }

    /// The name of the floor edge numbered `n` in `language`: `Slab edge 3`, `Döşeme kenarı 3`.
    #[must_use]
    pub fn floor_edge(language: &str, n: usize) -> String {
        if turkish(language) {
            format!("Döşeme kenarı {n}")
        } else {
            format!("Slab edge {n}")
        }
    }

    /// The name of a room in `language`: `Room`, `Oda`. Rooms are told apart by number.
    #[must_use]
    pub fn room(language: &str) -> &'static str {
        if turkish(language) { "Oda" } else { "Room" }
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
