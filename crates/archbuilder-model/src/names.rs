//! Name comparison. Names are stored exactly as entered; every comparison of names (uniqueness,
//! the next free storey name) goes through [`fold`] and [`same`], so it is one rule: Unicode case
//! folding in which the four letters `i`, `ı`, `İ` and `I` form one equivalence class. The rule
//! knows no language or machine locale, so a document behaves the same on every machine.

/// The key `name` is compared by: surrounding white space removed, then case folded, with
/// `i`, `ı`, `İ` and `I` all folding to `i`.
// shortcut: no Unicode normalization (NFC) of composed and decomposed letters, which would need a
// dependency; add it when a normalization crate is allowed.
#[must_use]
pub fn fold(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.trim().chars() {
        match c {
            'i' | 'ı' | 'İ' | 'I' => out.push('i'),
            // Full case folding: sharp s is "ss" and both sigmas fold to one.
            'ß' | 'ẞ' => out.push_str("ss"),
            'ς' => out.push('σ'),
            _ => out.extend(c.to_lowercase()),
        }
    }
    out
}

/// Whether two names are the same name.
#[must_use]
pub fn same(a: &str, b: &str) -> bool {
    fold(a) == fold(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_i_letters_are_one_letter_in_every_language() {
        // Lower-casing alone folds I to i, which is wrong for ı and İ; here all four agree.
        for a in ["i", "ı", "İ", "I"] {
            for b in ["i", "ı", "İ", "I"] {
                assert!(same(a, b), "{a} {b}");
            }
        }
        assert!(same("Bodrum katı", "BODRUM KATI"));
        assert!(same("Bodrum Katı", "bodrum kati"));
        assert!(same("  Zemin Kat ", "ZEMİN KAT"));
        assert!(!same("Kat 1", "Kat 2"));
        assert!(same("Straße", "STRASSE"));
    }
}
