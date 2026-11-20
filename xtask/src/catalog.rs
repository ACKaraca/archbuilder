//! The localization catalog gate: the message identifiers a crate's catalog defines and the
//! `message_id` strings its sources reference must be the same set.

use std::collections::BTreeSet;

/// The message identifiers defined by a Fluent catalog, in order, duplicates included. Only lines
/// of the form `identifier = text` at the start of a line count; comments (`#`), terms (`-name`),
/// attributes and continuation lines are skipped.
fn defined(catalog: &str) -> Vec<&str> {
    catalog
        .lines()
        .filter(|l| !l.starts_with(['#', ' ', '\t', '-', '.']))
        .filter_map(|l| l.split_once('=').map(|(id, _)| id.trim_end()))
        .filter(|id| {
            id.starts_with(|c: char| c.is_ascii_alphabetic())
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .collect()
}

/// The identifiers referenced as `message_id: "..."` in Rust source text. By convention the test
/// module closes a file, so everything from `#[cfg(test)]` on is ignored: tests may use
/// deliberately invalid identifiers. Comment lines are ignored.
pub fn referenced(source: &str) -> Vec<String> {
    let code: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = code.split("#[cfg(test)]").next().unwrap_or_default();
    source
        .match_indices("message_id:")
        .filter_map(|(at, key)| {
            let rest = source[at + key.len()..].trim_start().strip_prefix('"')?;
            rest.split_once('"').map(|(id, _)| id.to_owned())
        })
        .collect()
}

/// Compares one crate's catalog (`None` when it has none) with its referenced identifiers and
/// returns one message per problem.
pub fn check(krate: &str, catalog: Option<&str>, referenced: &[String]) -> Vec<String> {
    let Some(catalog) = catalog else {
        return if referenced.is_empty() {
            Vec::new()
        } else {
            vec![format!(
                "{krate}: references message identifiers but has no catalog locales/en-US/{krate}.ftl"
            )]
        };
    };
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for id in defined(catalog) {
        if !seen.insert(id) {
            out.push(format!(
                "{krate}: duplicate message identifier `{id}` in the catalog"
            ));
        }
    }
    let used: BTreeSet<&str> = referenced.iter().map(String::as_str).collect();
    for id in used.difference(&seen) {
        out.push(format!(
            "{krate}: message identifier `{id}` is referenced but not defined in the catalog"
        ));
    }
    for id in seen.difference(&used) {
        out.push(format!(
            "{krate}: message identifier `{id}` is defined but never referenced"
        ));
    }
    out
}

/// Compares the identifiers of a crate's Turkish catalog with its English one: the sets must be
/// equal, so a message is never missing in one language or left over in the other.
pub fn parity(krate: &str, english: &str, turkish: Option<&str>) -> Vec<String> {
    let Some(turkish) = turkish else {
        return vec![format!(
            "{krate}: has an English catalog but no locales/tr-TR/{krate}.ftl"
        )];
    };
    let en: BTreeSet<&str> = defined(english).into_iter().collect();
    let tr: BTreeSet<&str> = defined(turkish).into_iter().collect();
    let mut out: Vec<String> = en
        .difference(&tr)
        .map(|id| format!("{krate}: message identifier `{id}` is not translated in tr-TR"))
        .collect();
    out.extend(
        tr.difference(&en)
            .map(|id| format!("{krate}: tr-TR defines `{id}`, which en-US does not")),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str =
        "# comment\nfoo-a = Text { $x }\n  more text\n-term = T\nfoo-b = B\n    .attr = A\n";

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn matching_catalog_and_references_pass() {
        assert!(check("foo", Some(CATALOG), &ids(&["foo-a", "foo-b", "foo-a"])).is_empty());
    }

    #[test]
    fn missing_unused_and_duplicate_identifiers_are_reported() {
        let catalog = format!("{CATALOG}foo-a = again\n");
        let found = check("foo", Some(&catalog), &ids(&["foo-a", "foo-c"]));
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(
            found
                .iter()
                .any(|m| m.contains("duplicate") && m.contains("foo-a"))
        );
        assert!(
            found
                .iter()
                .any(|m| m.contains("referenced but not defined") && m.contains("foo-c"))
        );
        assert!(
            found
                .iter()
                .any(|m| m.contains("never referenced") && m.contains("foo-b"))
        );
    }

    #[test]
    fn the_two_languages_define_the_same_identifiers() {
        assert!(parity("foo", CATALOG, Some("foo-a = A\nfoo-b = B\n")).is_empty());
        assert_eq!(
            parity("foo", CATALOG, Some("foo-a = A\nfoo-c = C\n")).len(),
            2
        );
        assert_eq!(parity("foo", CATALOG, None).len(), 1);
    }

    #[test]
    fn references_without_a_catalog_fail_and_no_references_pass() {
        assert_eq!(check("foo", None, &ids(&["foo-a"])).len(), 1);
        assert!(check("foo", None, &[]).is_empty());
    }

    #[test]
    fn references_are_read_from_source_text() {
        let source = "message_id: \"a-b\",\n/// `message_id`, x\nmessage_id: &'static str,\nmessage_id:\"c\"\n#[cfg(test)]\nmessage_id: \"bad\"";
        assert_eq!(referenced(source), ids(&["a-b", "c"]));
    }
}
