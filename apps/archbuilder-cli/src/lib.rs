//! The headless CLI of ArchBuilder.
//!
//! [`run`] is the program. The shared subcommands of the Arc suite ([`arc_command::headless`]:
//! `commands`, `describe`, `validate-params`, `run`, `play`, `batch`, `inspect`, `validate`,
//! `serve`, `mcp`) run over the ArchBuilder type set and command set, with the model's validator
//! installed on every document. The working file is a document package with the extension
//! `archb`: `run <file> --create --save --allow-edit --write-root <dir> --command
//! archbuilder.project.new` makes a project, and each further `run <file> --save --command ...`
//! changes it. `inspect` adds a `walls` list to the shared dump: each wall with its storey, length,
//! thickness, height and the volume of its derived solid minus its openings, and the `doors` and
//! `windows` lists: each with its host wall, distance along it, width, height and sill, and the
//! `floors` list: each with its storey, thickness, area, volume and the walls its edges follow, and the
//! `rooms` list: each with its storey, number, name, state, area, perimeter and volume, the
//! `statistics` report (counts by kind, type and storey; derived), the `selection_sets` list and
//! the `curtain_walls` list: each curtain wall with its panels and mullions, the rows of
//! `archbuilder.curtain.schedule` (panel: key, kind, door or window type, material, thickness,
//! width, height, area, whether overridden; mullion: key, role, upright or not, profile, size,
//! length).
//! `export-ifc <file.archb> <out.ifc> [--timestamp ISO8601]` writes the document as an IFC4 file
//! ([`export`]) and prints the loss report; the shared `run` also has the command
//! `archbuilder.file.export_ifc`.

use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::sync::Arc;

use arc_command::headless::{self, Exit, Product};
use archbuilder_model::insert::Kind;
use serde_json::Value;

pub mod check;
pub mod export;
pub mod floors;
pub mod rooms;
pub mod stairs;
pub mod walls;

/// The product of the CLI: the commands of `archbuilder-commands` and the export command.
///
/// # Errors
/// The errors of the product's registry.
pub fn product(
    clock: std::sync::Arc<dyn arc_command::Clock>,
    owner: arc_doc::package::LockOwner,
) -> arc_base::Result<Product> {
    let mut product = archbuilder_commands::product(clock, owner)?;
    archbuilder_io::register(&mut product.registry)?;
    Ok(product)
}

/// A seed from the operating system's random source, for a run that names no seed. `std`'s
/// `RandomState` takes its keys from the operating system, so two runs differ.
#[must_use]
pub fn os_seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

/// Runs the command-line program through the shared runner. Never panics: a fault is contained
/// and reported as [`Exit::Internal`].
pub fn run<R, W, E>(
    args: impl IntoIterator<Item = OsString>,
    product: &Product,
    stdin: R,
    stdout: W,
    mut stderr: E,
) -> Exit
where
    R: BufRead,
    W: Write + Send,
    E: Write + Send,
{
    let mut args: Vec<OsString> = args.into_iter().collect();
    // The shared runner takes seed 0 when `--seed` is absent, which gives two new projects the
    // same identifiers; without a seed the CLI draws one from the operating system.
    if args.first().and_then(|a| a.to_str()) == Some("run")
        && !args.iter().any(|a| {
            a.to_str()
                .is_some_and(|s| s == "--seed" || s.starts_with("--seed="))
        })
    {
        args.insert(1, OsString::from("--seed"));
        args.insert(2, OsString::from(os_seed().to_string()));
    }
    let setup = |document: &arc_doc::Document| {
        archbuilder_commands::install(document);
        Ok(())
    };
    if args.first().and_then(|a| a.to_str()) == Some("export-ifc") {
        return export::run(&args[1..], product, stdout, stderr);
    }
    if args.first().and_then(|a| a.to_str()) == Some("help") {
        let exit = headless::run_with_setup(
            args,
            product,
            None,
            Some(&setup),
            stdin,
            stdout,
            &mut stderr,
        );
        let _ = writeln!(
            stderr,
            "  export-ifc <file> <out.ifc> [--timestamp T]   write the document as an IFC4 file; prints the loss report"
        );
        return exit;
    }
    if args.first().and_then(|a| a.to_str()) == Some("inspect") {
        return inspect(args, product, &setup, stdin, stdout, stderr);
    }
    headless::run_with_setup(args, product, None, Some(&setup), stdin, stdout, stderr)
}

/// The snapshot of the file named in `args` (the first argument after `inspect` that is not a
/// flag); `None` when it cannot be opened.
fn open_snapshot(args: &[OsString], product: &Product) -> Option<arc_doc::Snapshot> {
    use arc_doc::package::{Mode, Package};
    let limits = arc_base::Limits::DEFAULT;
    let file = args
        .iter()
        .skip(1)
        .find(|a| !a.to_string_lossy().starts_with("--"))?;
    let document = Package::open(std::path::Path::new(file), Mode::ReadOnly, &limits)
        .and_then(|p| {
            p.open_document(
                Arc::clone(&product.types),
                Arc::clone(&product.evaluators),
                &arc_doc::LoadOptions {
                    limits,
                    ..arc_doc::LoadOptions::default()
                },
                arc_doc::DocumentConfig::default(),
            )
        })
        .ok()?;
    Some(document.snapshot())
}

/// The net volume of every wall of the file named in `args`, by identifier; empty when the file
/// cannot be opened.
fn net_volumes(args: &[OsString], product: &Product) -> std::collections::HashMap<String, f64> {
    let Some(snapshot) = open_snapshot(args, product) else {
        return std::collections::HashMap::new();
    };
    archbuilder_model::spatial::objects(&snapshot, archbuilder_model::types::WALL)
        .iter()
        .filter_map(|w| {
            archbuilder_model::cleanup::wall_net_volume(&snapshot, w).map(|v| (w.id.to_string(), v))
        })
        .collect()
}

/// The area and volume of every room of the file named in `args`, by identifier, as the
/// application shows them (shafts out, under the surface that bounds the room).
fn room_nets(
    args: &[OsString],
    product: &Product,
) -> std::collections::HashMap<String, (Option<f64>, Option<f64>)> {
    let Some(snapshot) = open_snapshot(args, product) else {
        return std::collections::HashMap::new();
    };
    archbuilder_model::spatial::objects(&snapshot, archbuilder_model::types::STOREY)
        .iter()
        .flat_map(|s| archbuilder_model::room::of_storey(&snapshot, s.id))
        .map(|e| {
            (
                e.id.to_string(),
                (
                    archbuilder_model::room::net_area(&snapshot, &e),
                    archbuilder_model::room::net_volume(&snapshot, &e),
                ),
            )
        })
        .collect()
}

/// The model statistics and the selection sets (name, members) of the file named in `args`, as
/// JSON; `null` and an empty list when the file cannot be opened.
fn stats_and_sets(args: &[OsString], product: &Product) -> (Value, Value) {
    let Some(snapshot) = open_snapshot(args, product) else {
        return (Value::Null, Value::Array(Vec::new()));
    };
    let stats = serde_json::to_value(archbuilder_commands::stats::statistics(&snapshot))
        .unwrap_or(Value::Null);
    let sets = archbuilder_model::selection::all(&snapshot)
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id.to_string(),
                "name": archbuilder_model::types::text(&s.values, 0).unwrap_or_default(),
                "members": archbuilder_model::selection::members(&s.values)
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    (stats, Value::Array(sets))
}

/// The curtain walls of the file named in `args` with their derived panels and mullions (the rows
/// of `archbuilder.curtain.schedule`), as JSON; an empty list when the file cannot be opened.
fn curtain_walls(args: &[OsString], product: &Product) -> Value {
    let Some(snapshot) = open_snapshot(args, product) else {
        return Value::Array(Vec::new());
    };
    serde_json::to_value(archbuilder_commands::curtain::schedule(&snapshot, None))
        .unwrap_or_else(|_| Value::Array(Vec::new()))
}

/// The shared `inspect`, with the `walls` list added to its JSON line.
fn inspect<R, W, E>(
    args: Vec<OsString>,
    product: &Product,
    setup: &headless::DocumentSetup,
    stdin: R,
    mut stdout: W,
    stderr: E,
) -> Exit
where
    R: BufRead,
    W: Write + Send,
    E: Write + Send,
{
    let checked = args.iter().any(|a| a.to_str() == Some("--check"));
    // The net volume of each wall, from the document itself: the cleaned layers.
    let net = net_volumes(&args, product);
    let nets = room_nets(&args, product);
    let (stats, sets) = stats_and_sets(&args, product);
    let curtains = curtain_walls(&args, product);
    let mut out = Vec::new();
    let exit = headless::run_with_setup(args, product, None, Some(setup), stdin, &mut out, stderr);
    let text = String::from_utf8_lossy(&out);
    let enriched = text.lines().next().and_then(|line| {
        let mut dump: serde_json::Value = serde_json::from_str(line).ok()?;
        let walls = walls::summary(dump.get("objects")?.as_array()?, &net);
        let doors = walls::inserts(dump.get("objects")?.as_array()?, Kind::Door);
        let windows = walls::inserts(dump.get("objects")?.as_array()?, Kind::Window);
        let floors = floors::summary(dump.get("objects")?.as_array()?);
        let rooms = rooms::summary(dump.get("objects")?.as_array()?, &nets);
        let stairs = stairs::summary(dump.get("objects")?.as_array()?);
        let mut found = check::findings(dump.get("objects")?.as_array()?);
        found.extend(rooms::findings(dump.get("objects")?.as_array()?));
        found.extend(stairs::findings(dump.get("objects")?.as_array()?));
        let map = dump.as_object_mut()?;
        // The product's findings join the shared ones only when the check was asked for.
        if checked && let Some(list) = map.get_mut("findings").and_then(Value::as_array_mut) {
            list.extend(found);
        }
        map.insert("statistics".into(), stats);
        map.insert("selection_sets".into(), sets);
        map.insert("curtain_walls".into(), curtains);
        map.insert("floors".into(), floors.into());
        map.insert("rooms".into(), rooms.into());
        map.insert("stairs".into(), stairs.into());
        map.insert("walls".into(), walls.into());
        map.insert("doors".into(), doors.into());
        map.insert("windows".into(), windows.into());
        serde_json::to_string(&dump).ok()
    });
    // A dump that is not the expected JSON (an error report) goes out as it came.
    let _ = match enriched {
        Some(line) => writeln!(stdout, "{line}"),
        None => stdout.write_all(&out),
    };
    exit
}
