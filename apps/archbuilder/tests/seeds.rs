//! The identifiers of commands run one after another from the window never repeat: the seed of
//! each invocation is mixed, so the revision the context mixes in cannot line it up with the seed
//! of another invocation.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

mod common;

use archbuilder_model::types::{GRID, RAILING};
use common::Ui;
use serde_json::json;

#[test]
fn commands_run_in_a_row_never_reuse_an_identifier() {
    let mut ui = Ui::new("seeds");
    for k in 0..12_u32 {
        let y = f64::from(k) * 2.0;
        ui.invoke(
            "archbuilder.railing.create",
            json!({"points": [[0.0, y], [3.0, y]]}),
        );
        ui.invoke(
            "archbuilder.grid.create",
            json!({"start": [10.0 + y, 0.0], "end": [10.0 + y, 5.0]}),
        );
    }
    let text = ui.labels();
    assert!(!text.contains("already exists"), "{text}");
    assert_eq!((ui.count(RAILING), ui.count(GRID)), (12, 12));
}
