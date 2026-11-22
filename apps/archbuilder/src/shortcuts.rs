//! Two-letter command shortcuts, typed over the viewport or in the command line.
//!
//! Typing over the viewport goes to the command line, where the text shows as it accumulates. With no
//! command running, the text is resolved like a command name: a complete shortcut starts its command
//! at once ([`immediate`]) unless a longer command name starts with it (`wa` could become `wall`,
//! so it waits); anything else waits for Enter or Space, which start the shortcut it is, else the
//! command name or alias it names.
//!
//! The table is [`defaults`] with the overrides of the setting [`SETTING`] applied. The setting is
//! text: bindings separated by `;`, each `XX=command`; an empty command removes the default
//! binding of `XX`. The same table is exported and imported as a small JSON file
//! ([`Table::to_json`], [`Table::from_json`]).

use std::collections::{BTreeMap, BTreeSet};

use arc_base::{Error, Result};

/// The setting that holds the overrides of the default table.
pub const SETTING: &str = "archbuilder.shortcuts";

/// The most bindings a table or an imported file holds.
const MAX_BINDINGS: usize = 1024;

/// The format name of the exported file.
const FORMAT: &str = "archbuilder-shortcuts";

/// What the commands named by the default table are: registered commands and the interface
/// commands the controller handles itself (see [`crate::interaction`]).
pub mod ui {
    /// Repeats the last command.
    pub const REPEAT: &str = "archbuilder.view.repeat";
    /// Selects every object of the active storey that is not hidden.
    pub const SELECT_ALL: &str = "archbuilder.select.all";
    /// Selects every object of the kinds in the selection.
    pub const SELECT_KIND: &str = "archbuilder.select.all_of_kind";
    /// Restores the previous selection.
    pub const SELECT_PREVIOUS: &str = "archbuilder.select.previous";
    /// Keeps only the walls (or doors, windows, floors, rooms) of the selection; the kind is the
    /// `kind` parameter.
    pub const SELECT_ONLY: &str = "archbuilder.select.only";
    /// Selects objects by identifier; the identifiers are the `objects` parameter or typed.
    pub const SELECT_ID: &str = "archbuilder.select.by_id";
    /// Copies the selection to the clipboard.
    pub const COPY: &str = "archbuilder.clipboard.copy";
    /// Copies the selection to the clipboard and deletes it (one undo step).
    pub const CUT: &str = "archbuilder.clipboard.cut";
    /// Hides the selected objects.
    pub const HIDE: &str = "archbuilder.view.hide";
    /// Shows only the selected objects.
    pub const ISOLATE: &str = "archbuilder.view.isolate";
    /// Hides every object of the kinds in the selection.
    pub const HIDE_KIND: &str = "archbuilder.view.hide_category";
    /// Shows only the objects of the kinds in the selection.
    pub const ISOLATE_KIND: &str = "archbuilder.view.isolate_category";
    /// Ends the temporary hide and isolation.
    pub const RESET_VISIBILITY: &str = "archbuilder.view.reset";
    /// Frames the whole drawing.
    pub const ZOOM_EXTENTS: &str = "archbuilder.view.zoom_extents";
    /// Switches snapping on or off (the `on` parameter, else the opposite of now).
    pub const SNAP: &str = "archbuilder.view.snap";
    /// Writes the shortcut table to a file (the `path` parameter, else next to the document).
    pub const EXPORT_SHORTCUTS: &str = "archbuilder.shortcuts.export";
    /// Reads a shortcut table from a file (the `path` parameter).
    pub const IMPORT_SHORTCUTS: &str = "archbuilder.shortcuts.import";
}

/// The default bindings: two letters and the command each starts.
#[must_use]
pub fn defaults() -> Vec<(&'static str, &'static str)> {
    use archbuilder_commands::floor::FLOOR_CREATE;
    use archbuilder_commands::insert::{DOOR_PLACE, WINDOW_PLACE};
    use archbuilder_commands::modify::{
        CLIPBOARD_PASTE, CLIPBOARD_PASTE_TO_STOREY, MODIFY_COPY, MODIFY_MIRROR, MODIFY_MOVE,
        MODIFY_ROTATE,
    };
    use archbuilder_commands::room::ROOM_PLACE;
    use archbuilder_commands::wall::{OBJECT_DELETE, WALL_CREATE};
    vec![
        ("WA", WALL_CREATE),
        ("DR", DOOR_PLACE),
        ("WN", WINDOW_PLACE),
        ("FL", FLOOR_CREATE),
        ("RM", ROOM_PLACE),
        ("ST", archbuilder_commands::stair::STAIR_CREATE),
        ("MV", MODIFY_MOVE),
        ("CP", MODIFY_COPY),
        ("RT", MODIFY_ROTATE),
        ("MR", MODIFY_MIRROR),
        ("DL", OBJECT_DELETE),
        ("CB", ui::COPY),
        ("CU", ui::CUT),
        ("PS", CLIPBOARD_PASTE),
        ("PT", CLIPBOARD_PASTE_TO_STOREY),
        ("RR", ui::REPEAT),
        ("SA", ui::SELECT_ALL),
        ("SK", ui::SELECT_KIND),
        ("SP", ui::SELECT_PREVIOUS),
        ("HD", ui::HIDE),
        ("IS", ui::ISOLATE),
        ("HK", ui::HIDE_KIND),
        ("IK", ui::ISOLATE_KIND),
        ("RV", ui::RESET_VISIBILITY),
        ("ZE", ui::ZOOM_EXTENTS),
        ("SN", ui::SNAP),
        ("UD", arc_app::commands::EDIT_UNDO),
    ]
}

/// A table of bindings, by upper-case letter pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    map: BTreeMap<String, String>,
}

/// `keys` as a binding key: two ASCII letters, upper case.
fn key_of(keys: &str) -> Option<String> {
    let k = keys.trim().to_ascii_uppercase();
    (k.len() == 2 && k.bytes().all(|b| b.is_ascii_uppercase())).then_some(k)
}

impl Default for Table {
    fn default() -> Self {
        Self {
            map: defaults()
                .into_iter()
                .map(|(k, c)| (k.to_owned(), c.to_owned()))
                .collect(),
        }
    }
}

impl Table {
    /// The default table with the overrides of the setting text applied; malformed parts are
    /// skipped.
    #[must_use]
    pub fn with_overrides(text: &str) -> Self {
        let mut table = Self::default();
        for part in text.split(';').take(MAX_BINDINGS) {
            let Some((keys, command)) = part.split_once('=') else {
                continue;
            };
            let Some(key) = key_of(keys) else {
                continue;
            };
            let command = command.trim();
            if command.is_empty() {
                table.map.remove(&key);
            } else {
                table.map.insert(key, command.to_owned());
            }
        }
        table
    }

    /// The setting text that turns the default table into this one.
    #[must_use]
    pub fn overrides(&self) -> String {
        let base = Self::default();
        let mut parts: Vec<String> = self
            .map
            .iter()
            .filter(|(k, c)| base.map.get(*k) != Some(*c))
            .map(|(k, c)| format!("{k}={c}"))
            .collect();
        parts.extend(
            base.map
                .keys()
                .filter(|k| !self.map.contains_key(*k))
                .map(|k| format!("{k}=")),
        );
        parts.join(";")
    }

    /// The command bound to `keys` (two letters, any case).
    #[must_use]
    pub fn command(&self, keys: &str) -> Option<&str> {
        self.map.get(&key_of(keys)?).map(String::as_str)
    }

    /// Whether a binding starts with the letter `first`.
    #[must_use]
    pub fn begins_with(&self, first: char) -> bool {
        let f = first.to_ascii_uppercase();
        self.map.keys().any(|k| k.starts_with(f))
    }

    /// The bindings in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.map.iter().map(|(k, c)| (k.as_str(), c.as_str()))
    }

    /// The table as the text of a small JSON file.
    #[must_use]
    pub fn to_json(&self) -> String {
        let bindings: serde_json::Map<String, serde_json::Value> = self
            .map
            .iter()
            .map(|(k, c)| (k.clone(), serde_json::Value::String(c.clone())))
            .collect();
        let file = serde_json::json!({"format": FORMAT, "version": 1, "bindings": bindings});
        serde_json::to_string_pretty(&file).unwrap_or_default()
    }

    /// The table of an exported file. `known` says whether a command identifier can be started.
    ///
    /// # Errors
    /// `ab.cmd.shortcuts_invalid` when the text is not an exported table, has a key that is not
    /// two letters, or names a command that `known` refuses.
    pub fn from_json(text: &str, known: &dyn Fn(&str) -> bool) -> Result<Self> {
        let invalid = || Error::new(archbuilder_commands::error::codes::SHORTCUTS_INVALID);
        let value: serde_json::Value = serde_json::from_str(text).map_err(|_| invalid())?;
        if value.get("format").and_then(serde_json::Value::as_str) != Some(FORMAT) {
            return Err(invalid());
        }
        let bindings = value
            .get("bindings")
            .and_then(serde_json::Value::as_object)
            .filter(|b| b.len() <= MAX_BINDINGS)
            .ok_or_else(invalid)?;
        let mut map = BTreeMap::new();
        for (keys, command) in bindings {
            let command = command.as_str().filter(|c| known(c)).ok_or_else(invalid)?;
            map.insert(key_of(keys).ok_or_else(invalid)?, command.to_owned());
        }
        Ok(Self { map })
    }
}

/// The words that take arguments, so Space after them continues the text.
pub const ARGUMENT_NAMES: &[&str] = &[
    "selectid",
    "selid",
    "exportkeys",
    "importkeys",
    "pastestorey",
];

/// Words the line understands that no registered command carries.
const TYPED_WORDS: &[&str] = &[
    "selectall",
    "unhide",
    "slab",
    "doseme",
    "döşeme",
    "kapi",
    "tasi",
    "taşı",
    "dondur",
    "erase",
    "sil",
    "cut",
    "repeat",
    "snap",
];

/// The command names and aliases a typed text could be the start of, read from the commands
/// themselves: every dot-separated part of an identifier, the user aliases, the labels in each
/// localizer (the whole label and each of its words), the identifiers of the shortcut table, and a
/// few words only the line knows. A command added to the product is in it without a list to keep.
#[must_use]
pub fn command_names(
    registries: &[&arc_command::Registry],
    localizers: &[&arc_app::locale::Localizer],
    table: &Table,
) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    let add_id = |id: &str, names: &mut BTreeSet<String>| {
        names.extend(id.split('.').map(str::to_lowercase));
    };
    for registry in registries {
        for spec in registry.iter() {
            add_id(spec.id(), &mut names);
            for localizer in localizers {
                let label = localizer.format(spec.label(), &[]).text.to_lowercase();
                names.extend(label.split_whitespace().map(str::to_owned));
                names.insert(label);
            }
        }
        for (alias, _) in registry.user_aliases() {
            names.insert(alias.to_lowercase());
        }
    }
    for (_, command) in table.iter() {
        add_id(command, &mut names);
    }
    names.extend(
        TYPED_WORDS
            .iter()
            .chain(ARGUMENT_NAMES)
            .map(|w| (*w).to_owned()),
    );
    names
}

/// The command a complete shortcut starts as soon as it is typed: `text` is exactly a shortcut and
/// no command name in `names` that is longer begins with it.
#[must_use]
pub fn immediate<'t>(table: &'t Table, names: &BTreeSet<String>, text: &str) -> Option<&'t str> {
    let lower = text.to_lowercase();
    let command = table.command(&lower)?;
    let longer = names
        .range(lower.clone()..)
        .take_while(|n| n.starts_with(&lower))
        .any(|n| n.len() > lower.len());
    (!longer).then_some(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> BTreeSet<String> {
        let mut shell = arc_command::Registry::new();
        arc_app::commands::register(&mut shell).unwrap();
        let mut product = archbuilder_commands::registry().unwrap();
        archbuilder_io::register(&mut product).unwrap();
        let english = crate::localizer(&["en-US".to_owned()]).unwrap();
        command_names(&[&product, &shell], &[&english], &Table::default())
    }

    #[test]
    fn a_complete_shortcut_starts_at_once_unless_a_longer_command_name_begins_with_it() {
        let table = Table::default();
        let names = names();
        // `mv` begins no other name: it starts at once; the others could become a longer name.
        assert_eq!(
            immediate(&table, &names, "mv"),
            Some(archbuilder_commands::modify::MODIFY_MOVE)
        );
        assert_eq!(
            immediate(&table, &names, "MV"),
            immediate(&table, &names, "mv")
        );
        for keys in ["wa", "sn", "is", "fl", "sa", "w", "wall"] {
            assert_eq!(immediate(&table, &names, keys), None, "{keys}");
        }
        // `save` is a shell command read from the registry, not from a list: `sa` waits for it.
        assert!(names.contains("save"));
        let without: BTreeSet<String> = BTreeSet::new();
        assert!(immediate(&table, &without, "sa").is_some());
        // The full names and the aliases are names, not shortcuts.
        for word in [
            "wall", "move", "m", "copy", "door", "window", "floor", "room", "delete",
        ] {
            assert!(
                table.command(word).is_none() && names.contains(word),
                "{word}"
            );
        }
        let custom = Table::with_overrides("ww=archbuilder.wall.create");
        assert_eq!(
            immediate(&custom, &names, "ww"),
            Some("archbuilder.wall.create")
        );
    }

    #[test]
    fn a_customized_binding_survives_export_and_import_and_the_setting_text() {
        let known = |c: &str| defaults().iter().any(|(_, d)| *d == c);
        let table =
            Table::with_overrides("ww=archbuilder.wall.create;wa=;dl=archbuilder.modify.move");
        assert_eq!(table.command("WW"), Some("archbuilder.wall.create"));
        assert_eq!(table.command("WA"), None);
        assert_eq!(table.command("dl"), Some("archbuilder.modify.move"));
        let file = table.to_json();
        let back = Table::from_json(&file, &known).unwrap();
        assert_eq!(back, table);
        // The setting text that rebuilds it holds only the differences.
        assert_eq!(Table::with_overrides(&back.overrides()), table);
        assert_eq!(Table::default().overrides(), "");
        // Bad files are refused: not ours, a bad key, an unknown command.
        for bad in [
            "{}",
            "not json",
            r#"{"format":"archbuilder-shortcuts","version":1,"bindings":{"W":"archbuilder.wall.create"}}"#,
            r#"{"format":"archbuilder-shortcuts","version":1,"bindings":{"WA":"no.such"}}"#,
        ] {
            assert_eq!(
                Table::from_json(bad, &known).unwrap_err().code(),
                "ab.cmd.shortcuts_invalid"
            );
        }
    }

    #[test]
    fn the_default_table_has_unique_two_letter_keys_and_no_prefix_trap() {
        let keys: Vec<&str> = defaults().iter().map(|(k, _)| *k).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len());
        assert!(keys.iter().all(|k| key_of(k).as_deref() == Some(*k)));
    }
}
