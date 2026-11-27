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
use archbuilder_io::mesh::{MeshFormat, MeshOptions, Unit, export_mesh};
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

/// The arguments of `export-mesh`.
struct MeshArgs<'a> {
    file: &'a Path,
    out: &'a Path,
    options: MeshOptions,
}

fn parse_mesh<'a>(args: &'a [OsString]) -> Result<MeshArgs<'a>> {
    let (mut paths, mut timestamp) = (Vec::new(), None);
    let mut options = MeshOptions::default();
    let mut format = None;
    let mut only = std::collections::BTreeSet::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let text = arg.to_str();
        let mut value = |name: &str| -> Result<&'a str> {
            iter.next()
                .and_then(|v| v.to_str())
                .ok_or_else(|| usage(&format!("missing_value:{name}")))
        };
        match text {
            Some("--timestamp") => timestamp = Some(value("--timestamp")?),
            Some("--format") => {
                format = Some(
                    MeshFormat::of_extension(value("--format")?)
                        .ok_or_else(|| usage("format:obj|stl|gltf|glb"))?,
                );
            }
            Some("--unit") => {
                options.unit = match value("--unit")? {
                    "m" => Unit::Meter,
                    "cm" => Unit::Centimeter,
                    "mm" => Unit::Millimeter,
                    "ft" => Unit::Foot,
                    "in" => Unit::Inch,
                    _ => return Err(usage("unit:m|cm|mm|ft|in")),
                };
            }
            Some("--element") => {
                let id = value("--element")?
                    .parse()
                    .map_err(|_| usage("element:<identifier>"))?;
                only.insert(id);
            }
            Some("--merge") => options.merge = true,
            Some("--no-materials") => options.materials = false,
            Some(t) if t.starts_with("--") => return Err(usage(&format!("unknown_option:{t}"))),
            _ => paths.push(Path::new(arg)),
        }
    }
    let [file, out] = paths[..] else {
        return Err(usage("expected:<file.archb> <out.obj|stl|gltf|glb>"));
    };
    options.format = format
        .or_else(|| {
            out.extension()
                .and_then(|e| e.to_str())
                .and_then(MeshFormat::of_extension)
        })
        .ok_or_else(|| usage("format:obj|stl|gltf|glb"))?;
    if !only.is_empty() {
        options.only = Some(only);
    }
    options.timestamp = timestamp.map_or_else(|| iso8601(now_millis()), str::to_owned);
    Ok(MeshArgs { file, out, options })
}

fn export_mesh_file(args: MeshArgs<'_>, product: &Product) -> Result<serde_json::Value> {
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
    let mut options = args.options;
    let name = |p: &Path| {
        p.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned()
    };
    let mtl_out = args.out.with_extension("mtl");
    options.file_name = name(args.out);
    options.mtl_name = name(&mtl_out);
    options.application = format!("{} {}", product.name, product.version);
    options.limits = limits;
    let export = export_mesh(&document.snapshot(), &options)?;
    let io = |e| Error::new(codes::PATH_IO).with_source(e);
    std::fs::write(args.out, &export.bytes).map_err(io)?;
    if let Some(mtl) = &export.mtl {
        std::fs::write(&mtl_out, mtl).map_err(io)?;
    }
    Ok(json!({
        "file": args.out.display().to_string(),
        "bytes": export.bytes.len(),
        "elements": export.elements,
        "triangles": export.triangles,
        "bounds": export.bounds,
        "summary": export.report.summary,
        "items": export.report.items,
    }))
}

/// Runs `export-mesh` with the arguments after its name.
pub fn run_mesh<W: Write, E: Write>(
    args: &[OsString],
    product: &Product,
    mut stdout: W,
    mut stderr: E,
) -> Exit {
    match parse_mesh(args).and_then(|a| export_mesh_file(a, product)) {
        Ok(line) => {
            let _ = writeln!(stdout, "{line}");
            Exit::Success
        }
        Err(error) => report_error(error, &mut stderr),
    }
}
