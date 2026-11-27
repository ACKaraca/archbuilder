//! `family check <file.archbfam> [--available name@version]...` reads a family package, verifies
//! its checksums, definitions, test cases and dependencies and prints the report as one JSON line
//! (exit 3 when the package is refused); `family publish --out <file.archbfam> --name N
//! --version V --family <definition.txt>=<version>... [--license L] [--source S] [--author A]
//! [--description D] [--min-product V] [--depends name>=version]...` publishes definitions (given
//! as canonical text) into a new package, after every test case passes.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use arc_base::{Error, Limits, Result};
use arc_command::error::codes;
use arc_command::headless::{Exit, Product, report_error};
use arc_command::{Invocation, Invoker, Permission, Session, Surface};
use arc_doc::package::{Metadata, Mode, Package};
use arc_doc::{Document, DocumentConfig, LoadOptions, Snapshot};
use archbuilder_commands::family::{FAMILY_MANIFEST, FAMILY_PUBLISH, available_of, report};
use serde_json::{Value, json};

fn usage(reason: &str) -> Error {
    Error::new(codes::CLI_USAGE).with_detail("reason", reason)
}

fn open(file: &Path, product: &Product) -> Result<Document> {
    let limits = Limits::DEFAULT;
    Package::open(file, Mode::ReadOnly, &limits)?.open_document(
        Arc::clone(&product.types),
        Arc::clone(&product.evaluators),
        &LoadOptions {
            limits,
            ..LoadOptions::default()
        },
        DocumentConfig::default(),
    )
}

/// The value of each `--name value` in `args`, and the positional arguments.
type Options = Vec<(String, String)>;

fn split(args: &[OsString]) -> Result<(Options, Vec<String>)> {
    let (mut opts, mut rest) = (Vec::new(), Vec::new());
    let mut it = args.iter().map(|a| a.to_string_lossy().into_owned());
    while let Some(a) = it.next() {
        if let Some(name) = a.strip_prefix("--") {
            let v = it
                .next()
                .ok_or_else(|| usage(&format!("missing_value:{name}")))?;
            opts.push((name.to_owned(), v));
        } else {
            rest.push(a);
        }
    }
    Ok((opts, rest))
}

fn all<'a>(opts: &'a [(String, String)], name: &'a str) -> impl Iterator<Item = &'a str> {
    opts.iter()
        .filter(move |(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

fn check(args: &[OsString], product: &Product) -> Result<(Value, bool)> {
    let (opts, rest) = split(args)?;
    let [file] = rest.as_slice() else {
        return Err(usage("expected:family check <file.archbfam>"));
    };
    if let Some((n, _)) = opts.iter().find(|(n, _)| n != "available") {
        return Err(usage(&format!("unknown_option:--{n}")));
    }
    let available = available_of(
        &all(&opts, "available")
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    )?;
    let document = open(Path::new(file), product)?;
    let report = report(&document.snapshot(), &available);
    let ok = report.ok;
    Ok((serde_json::to_value(report).unwrap_or(Value::Null), ok))
}

fn invoke(product: &Product, document: &Document, command: &str, params: Value) -> Result<Value> {
    let invocation = Invocation {
        seed: crate::os_seed(),
        ..Invocation::new(command, params)
    };
    let session = Session::new(Surface::Automation, [Permission::Read, Permission::Edit]);
    Invoker::new(&product.registry)
        .invoke(document, &session, invocation)
        .map(|o| o.value)
}

fn publish(args: &[OsString], product: &Product) -> Result<Value> {
    let (opts, rest) = split(args)?;
    if !rest.is_empty() {
        return Err(usage("unexpected_argument"));
    }
    let one = |name: &str| all(&opts, name).next().map(str::to_owned);
    let out = one("out").ok_or_else(|| usage("missing:--out"))?;
    let name = one("name").ok_or_else(|| usage("missing:--name"))?;
    let version = one("version").ok_or_else(|| usage("missing:--version"))?;
    let families: Vec<&str> = all(&opts, "family").collect();
    if families.is_empty() {
        return Err(usage("missing:--family"));
    }
    if Path::new(&out).exists() {
        return Err(Error::new(codes::PATH_IO).with_detail("reason", "exists"));
    }
    let document = Document::open(
        Snapshot::empty(Arc::clone(&product.types)),
        Arc::clone(&product.evaluators),
        DocumentConfig::default(),
    )?;
    let text = |n: &str| one(n).unwrap_or_default();
    invoke(
        product,
        &document,
        FAMILY_MANIFEST,
        json!({
            "name": name, "version": version, "description": text("description"),
            "license": text("license"), "source": text("source"), "author": text("author"),
            "min_product": text("min-product"),
            "dependencies": all(&opts, "depends").collect::<Vec<_>>(),
        }),
    )?;
    let mut published = Vec::new();
    for entry in families {
        let (file, v) = entry
            .rsplit_once('=')
            .ok_or_else(|| usage("family:<definition.txt>=<version>"))?;
        let body =
            std::fs::read_to_string(file).map_err(|e| Error::new(codes::PATH_IO).with_source(e))?;
        published.push(invoke(
            product,
            &document,
            FAMILY_PUBLISH,
            json!({"text": body, "version": v}),
        )?);
    }
    let owner = product.owner.clone();
    let mut package = Package::create(Path::new(&out), &Metadata::default(), &owner)?;
    let saved = package.save_document(&document, &[], None, &mut |_| Ok(()));
    if let Err(e) = saved {
        drop(package);
        let _ = std::fs::remove_file(&out);
        return Err(e);
    }
    Ok(json!({"file": out, "package": name, "version": version, "families": published}))
}

/// Runs the subcommand with the arguments after its name.
pub fn run<W: Write, E: Write>(
    args: &[OsString],
    product: &Product,
    mut stdout: W,
    mut stderr: E,
) -> Exit {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().and_then(|a| a.to_str()) {
        Some("check") => match check(rest, product) {
            Ok((line, ok)) => {
                let _ = writeln!(stdout, "{line}");
                if ok { Exit::Success } else { Exit::Invalid }
            }
            Err(error) => report_error(error, &mut stderr),
        },
        Some("publish") => match publish(rest, product) {
            Ok(line) => {
                let _ = writeln!(stdout, "{line}");
                Exit::Success
            }
            Err(error) => report_error(error, &mut stderr),
        },
        _ => report_error(usage("expected:family check|publish"), &mut stderr),
    }
}
