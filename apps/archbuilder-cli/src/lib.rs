//! The headless CLI of ArchBuilder.
//!
//! [`run`] is the program. The shared subcommands of the Arc suite ([`arc_command::headless`]:
//! `commands`, `describe`, `validate-params`, `run`, `play`, `batch`, `inspect`, `validate`,
//! `serve`, `mcp`) run over the ArchBuilder type set and command set, with the model's validator
//! installed on every document. The working file is a document package with the extension
//! `archb`: `run <file> --create --save --allow-edit --write-root <dir> --command
//! archbuilder.project.new` makes a project, and each further `run <file> --save --command ...`
//! changes it. `inspect` adds a `walls` list to the shared dump: each wall with its storey, length,
//! thickness, height and the volume of its derived solid.

use std::ffi::OsString;
use std::io::{BufRead, Write};

use arc_command::headless::{self, Exit, Product};

pub mod walls;

pub use archbuilder_commands::product;

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
    stderr: E,
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
    let mut out = Vec::new();
    let exit = headless::run_with_setup(args, product, None, Some(setup), stdin, &mut out, stderr);
    let text = String::from_utf8_lossy(&out);
    let enriched = text.lines().next().and_then(|line| {
        let mut dump: serde_json::Value = serde_json::from_str(line).ok()?;
        let walls = walls::summary(dump.get("objects")?.as_array()?);
        dump.as_object_mut()?.insert("walls".into(), walls.into());
        serde_json::to_string(&dump).ok()
    });
    // A dump that is not the expected JSON (an error report) goes out as it came.
    let _ = match enriched {
        Some(line) => writeln!(stdout, "{line}"),
        None => stdout.write_all(&out),
    };
    exit
}
