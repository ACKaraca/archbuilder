//! The application product, headless: a new document starts with its project (no undo step), saves
//! through the application's own path and opens again with the same structure; each level draws
//! its outline and has a label with its name and elevation.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use std::sync::Arc;

use arc_app::application::{AppConfig, Application};
use arc_app::recent::RecentFiles;
use arc_base::{Cancel, IdGenerator, Limits};
use arc_command::FixedClock;
use arc_doc::package::LockOwner;
use arc_gpu::extract::Extractor;
use archbuilder_model::spatial;
use archbuilder_model::types::{LEVEL, text};

fn app(dir: &std::path::Path, seed: u64) -> Application {
    let owner = LockOwner {
        user: "test".into(),
        host: "test".into(),
        application: "archbuilder tests".into(),
        time_unix_ms: 0,
    };
    let product = archbuilder::product(Arc::new(FixedClock(0)), owner).unwrap();
    let mut next = seed;
    Application::new(
        product,
        AppConfig {
            data_dir: dir.to_path_buf(),
            limits: Limits::DEFAULT,
            id_generators: Box::new(move || {
                next += 1;
                IdGenerator::seeded(next)
            }),
            recent: RecentFiles::new(4),
        },
    )
}

/// The names of the project and of the storeys, bottom up, with each level's elevation.
fn structure(app: &Application, tab: arc_app::application::TabId) -> (String, Vec<(String, f64)>) {
    let snapshot = app.tab(tab).unwrap().document().snapshot();
    let project = spatial::project(&snapshot).unwrap();
    let building = spatial::buildings(&snapshot)[0].id;
    let storeys = spatial::storeys(&snapshot, building)
        .iter()
        .map(|s| (text(&s.storey.values, 0).unwrap().to_owned(), s.elevation()))
        .collect();
    (text(&project.values, 0).unwrap().to_owned(), storeys)
}

#[test]
fn a_new_document_starts_with_its_project_no_undo_step_and_saves_and_opens_again() {
    let dir = std::env::temp_dir().join(format!("archbuilder-app-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut first = app(&dir, 0);
    let tab = first.new_document_in("tr").unwrap();
    let before = structure(&first, tab);
    assert_eq!(before.0, "Adsız Proje", "named in the interface language");
    assert_eq!(before.1.len(), 3);
    // The project is the document's starting content: clean, and nothing for undo to remove.
    assert!(!first.tab(tab).unwrap().is_dirty());
    assert!(!first.undo(tab).unwrap());
    assert_eq!(structure(&first, tab), before);
    let file = dir.join("house.archb");
    first.save_as(tab, &file).unwrap();
    assert!(!first.tab(tab).unwrap().is_dirty());
    first
        .close(tab, |_| arc_app::application::CloseAnswer::Discard)
        .unwrap();

    let mut second = app(&dir, 100);
    let reopened = second.open(&file).unwrap();
    assert_eq!(structure(&second, reopened), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_level_draws_an_outline_and_labels_carry_name_and_elevation_in_the_unit_and_language() {
    let dir = std::env::temp_dir().join(format!("archbuilder-draw-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut app = app(&dir, 200);
    let tab = app.new_document().unwrap();
    let product = app.product();
    let extractor = *product.extractors()[0]
        .factory
        .create()
        .downcast::<Extractor>()
        .unwrap();
    let snapshot = app.tab(tab).unwrap().document().snapshot();
    let levels = spatial::objects(&snapshot, LEVEL);
    assert_eq!(levels.len(), 3);
    for level in &levels {
        let extraction = extractor(&snapshot, level, &Cancel::new()).unwrap();
        assert_eq!(
            extraction.items.len(),
            1,
            "the outline; the label faces the screen"
        );
    }
    use archbuilder_model::units::LengthUnit;
    let floor1 = levels
        .iter()
        .find(|l| text(&l.values, 0) == Some("Floor 1"))
        .unwrap();
    assert_eq!(
        archbuilder::labels::label(floor1, LengthUnit::Meter, "en"),
        "Floor 1  3 m"
    );
    assert_eq!(
        archbuilder::labels::label(floor1, LengthUnit::Millimeter, "tr"),
        "Floor 1  3000 mm"
    );
    // Seen from above every level projects to one place: the labels come highest first.
    let overlays = archbuilder::labels::of_snapshot(&snapshot, "tr");
    assert_eq!(overlays.len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_label_of_the_toolbar_and_the_commands_is_in_both_catalogs() {
    use arc_app::shell::ToolItem;
    let mut ids: Vec<String> = Vec::new();
    for item in archbuilder::chrome::toolbar() {
        match item {
            ToolItem::Button { label, hint, .. } => {
                ids.push(label);
                ids.extend(hint);
            }
            ToolItem::ZoomExtents { label } => ids.push(label),
            _ => {}
        }
    }
    for spec in archbuilder_commands::registry().unwrap().iter() {
        ids.push(spec.label().to_owned());
        ids.push(spec.description().to_owned());
    }
    for language in ["en-US", "tr-TR"] {
        let localizer = archbuilder::localizer(&[language.to_owned()]).unwrap();
        for id in &ids {
            let text = localizer.format(id, &[]);
            assert_eq!(
                text.locale.as_ref().map(ToString::to_string).as_deref(),
                Some(language),
                "{id} in {language}"
            );
        }
    }
}
