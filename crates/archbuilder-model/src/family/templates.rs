//! Starter definitions per category (ADR 0009, unit 12): the canonical text of a definition that
//! the family editor opens as a new family, so a person edits a working family instead of an empty
//! page. Each text reads and publishes as it stands (a test checks it).

use super::def::Category;

/// The starter text of `category`, when the editor has one: doors and windows are hosted by
/// walls, furniture, casework and generic models stand freely.
#[must_use]
pub fn starter(category: Category) -> Option<&'static str> {
    match category {
        Category::Door => Some(include_str!("templates/door.txt")),
        Category::Window => Some(include_str!("templates/window.txt")),
        Category::Furniture => Some(include_str!("templates/furniture.txt")),
        Category::Casework => Some(include_str!("templates/casework.txt")),
        Category::GenericModel => Some(include_str!("templates/generic.txt")),
        _ => None,
    }
}

/// The categories with a starter, in the order the editor lists them.
pub const STARTER_CATEGORIES: [Category; 5] = [
    Category::Door,
    Category::Window,
    Category::Furniture,
    Category::Casework,
    Category::GenericModel,
];

#[cfg(test)]
mod tests {
    use super::super::def::{Definition, Version};
    use super::super::package::publish;
    use super::{STARTER_CATEGORIES, starter};

    #[test]
    fn every_starter_reads_publishes_and_round_trips() {
        for category in STARTER_CATEGORIES {
            let text = starter(category).expect("a starter per listed category");
            let definition = Definition::from_text(text).expect("the starter reads");
            assert_eq!(definition.header.category, category);
            let version = Version {
                major: 1,
                minor: 0,
                patch: 0,
            };
            let published = publish(&definition, None, version).expect("publishes");
            let again = Definition::from_text(&published.text).expect("re-reads");
            assert_eq!(again.to_text().unwrap(), published.text);
        }
    }
}
