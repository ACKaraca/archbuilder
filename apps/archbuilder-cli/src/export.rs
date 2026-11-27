//! `export-ifc <file.archb> <out.ifc> [--timestamp ISO8601]`: the document as an IFC4 file, and
//! the loss report on standard output as one JSON line. Without `--timestamp` the header takes
//! the current time; the export itself reads no clock.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use arc_base::{Error, Limits, Result};
use arc_command::error::codes;
use arc_command::headless::{Exit, Product, report_error};
use arc_doc::package::{Mode, Package};
use arc_doc::{DocumentConfig, LoadOptions};
use archbuilder_io::time::iso8601;
use archbuilder_io::{ExportOptions, export_ifc};
use serde_json::json;

/// The arguments of the subcommand.
struct Args<'a> {
    file: &'a Path,
    out: &'a Path,
    timestamp: Option<&'a str>,
}

fn usage(reason: &str) -> Error {
    Error::new(codes::CLI_USAGE).with_detail("reason", reason)
}

fn parse(args: &[OsString]) -> Result<Args<'_>> {
    let (mut paths, mut timestamp) = (Vec::new(), None);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let text = arg.to_str();
        match text {
            Some("--timestamp") => {
                let value = iter.next().and_then(|v| v.to_str());
                timestamp = Some(value.ok_or_else(|| usage("missing_value:--timestamp"))?);
            }
            Some(t) if t.starts_with("--timestamp=") => timestamp = t.strip_prefix("--timestamp="),
            Some(t) if t.starts_with("--") => return Err(usage(&format!("unknown_option:{t}"))),
            _ => paths.push(Path::new(arg)),
        }
    }
    match paths[..] {
        [file, out] => Ok(Args {
            file,
            out,
            timestamp,
        }),
        _ => Err(usage("expected:<file.archb> <out.ifc>")),
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn export(args: &Args<'_>, product: &Product) -> Result<serde_json::Value> {
    let limits = Limits::DEFAULT;
    let package = Package::open(args.file, Mode::ReadOnly, &limits)?;
    let document = package.open_document(
        Arc::clone(&product.types),
        Arc::clone(&product.evaluators),
        &LoadOptions {
            limits,
            ..LoadOptions::default()
        },
        DocumentConfig::default(),
    )?;
    let options = ExportOptions {
        file_name: args
            .out
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned(),
        timestamp: args
            .timestamp
            .map_or_else(|| iso8601(now_millis()), str::to_owned),
        application: format!("{} {}", product.name, product.version),
        limits,
        ..ExportOptions::default()
    };
    let export = export_ifc(&document.snapshot(), &options)?;
    std::fs::write(args.out, &export.bytes)
        .map_err(|e| Error::new(codes::PATH_IO).with_source(e))?;
    Ok(json!({
        "file": args.out.display().to_string(),
        "bytes": export.bytes.len(),
        "elements": export.elements,
        "valid": export.valid,
        "summary": export.report.summary,
        "items": export.report.items,
    }))
}

/// Runs the subcommand with the arguments after its name.
pub fn run<W: Write, E: Write>(
    args: &[OsString],
    product: &Product,
    mut stdout: W,
    mut stderr: E,
) -> Exit {
    match parse(args).and_then(|a| export(&a, product)) {
        Ok(line) => {
            let _ = writeln!(stdout, "{line}");
            Exit::Success
        }
        Err(error) => report_error(error, &mut stderr),
    }
}
