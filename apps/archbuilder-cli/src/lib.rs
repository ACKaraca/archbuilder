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
//! `rooms` list: each with its storey, number, name, state, area, perimeter and volume.
//! `export-ifc <file.archb> <out.ifc> [--timestamp ISO8601]` writes the document as an IFC4 file
//! ([`export`]) and prints the loss report; the shared `run` also has the command
//! `archbuilder.file.export_ifc`.

use std::ffi::OsString;
use std::io::{BufRead, Write};

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
    let mut out = Vec::new();
    let exit = headless::run_with_setup(args, product, None, Some(setup), stdin, &mut out, stderr);
    let text = String::from_utf8_lossy(&out);
    let enriched = text.lines().next().and_then(|line| {
        let mut dump: serde_json::Value = serde_json::from_str(line).ok()?;
        let walls = walls::summary(dump.get("objects")?.as_array()?);
        let doors = walls::inserts(dump.get("objects")?.as_array()?, Kind::Door);
        let windows = walls::inserts(dump.get("objects")?.as_array()?, Kind::Window);
        let floors = floors::summary(dump.get("objects")?.as_array()?);
        let rooms = rooms::summary(dump.get("objects")?.as_array()?);
        let stairs = stairs::summary(dump.get("objects")?.as_array()?);
        let mut found = check::findings(dump.get("objects")?.as_array()?);
        found.extend(rooms::findings(dump.get("objects")?.as_array()?));
        found.extend(stairs::findings(dump.get("objects")?.as_array()?));
        let map = dump.as_object_mut()?;
        // The product's findings join the shared ones only when the check was asked for.
        if checked && let Some(list) = map.get_mut("findings").and_then(Value::as_array_mut) {
            list.extend(found);
        }
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
