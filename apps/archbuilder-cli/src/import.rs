//! `import-ifc <file.archb> <in.ifc>`: reads an IFC file into a document with the command
//! `archbuilder.file.import_ifc` (one undo step), creating the document when the path does not
//! exist, saves it and prints the result with the loss report as one JSON line. The command runs
//! through the shared runner, so the read root is the directory of the IFC file and the write
//! root the directory of the document.

use std::ffi::OsString;
use std::io::{BufRead, Write};
use std::path::Path;

use arc_command::headless::{self, Exit, Product};

/// Runs the subcommand with the arguments after its name.
pub fn run<R, W, E>(
    args: &[OsString],
    product: &Product,
    setup: &headless::DocumentSetup,
    stdin: R,
    stdout: W,
    mut stderr: E,
) -> Exit
where
    R: BufRead,
    W: Write + Send,
    E: Write + Send,
{
    let [document, ifc] = args else {
        let _ = writeln!(stderr, "usage: import-ifc <file.archb> <in.ifc>");
        return Exit::Usage;
    };
    let absolute = |p: &OsString| std::fs::canonicalize(p).or_else(|_| std::path::absolute(p));
    let (Ok(ifc_path), Ok(document_path)) = (absolute(ifc), absolute(document)) else {
        let _ = writeln!(stderr, "import-ifc: a path cannot be resolved");
        return Exit::Usage;
    };
    let dir = |p: &Path| {
        p.parent()
            .map_or_else(|| p.to_path_buf(), Path::to_path_buf)
    };
    let Some(ifc_text) = ifc_path.to_str() else {
        let _ = writeln!(stderr, "import-ifc: the IFC path is not text");
        return Exit::Usage;
    };
    let params = serde_json::json!({ "path": ifc_text }).to_string();
    let mut run: Vec<OsString> = vec![
        "run".into(),
        document_path.clone().into_os_string(),
        "--seed".into(),
        crate::os_seed().to_string().into(),
        "--command".into(),
        archbuilder_io::import::IMPORT_IFC.into(),
        "--params".into(),
        params.into(),
        "--read-root".into(),
        dir(&ifc_path).into_os_string(),
        "--write-root".into(),
        dir(&document_path).into_os_string(),
        "--allow-edit".into(),
        "--save".into(),
    ];
    if !document_path.exists() {
        run.push("--create".into());
    }
    headless::run_with_setup(run, product, None, Some(setup), stdin, stdout, stderr)
}
