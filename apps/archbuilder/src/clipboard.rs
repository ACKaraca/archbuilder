//! The clipboard of the application: one for every tab and document of the product, so a copy in
//! one document pastes into another. It holds the content made by
//! [`archbuilder_commands::clip::capture`] and the base point it is pasted from.

use std::sync::Mutex;

use serde_json::Value;

static CLIP: Mutex<Option<(Value, [f64; 2])>> = Mutex::new(None);

/// Puts `content` and its base point on the clipboard, replacing what it held.
pub fn set(content: Value, base: [f64; 2]) {
    if let Ok(mut clip) = CLIP.lock() {
        *clip = Some((content, base));
    }
}

/// What the clipboard holds: the content and its base point.
#[must_use]
pub fn get() -> Option<(Value, [f64; 2])> {
    CLIP.lock().ok().and_then(|clip| clip.clone())
}
