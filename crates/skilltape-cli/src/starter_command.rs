//! Fixed local recipes; scaffolding never executes a task.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use clap::{Subcommand, ValueEnum};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use skilltape_runner::{publish_directory_noreplace, snapshot_inputs, validate_output_root};

const LIMIT: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Scenario {
    Inventory,
    Reconcile,
}
impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Reconcile => "reconcile",
        }
    }
}

#[derive(Subcommand)]
pub(crate) enum StarterCommand {
    /// Create editable inputs and instructions in a new directory. Executes nothing.
    New {
        #[arg(value_enum)]
        scenario: Scenario,
        #[arg(long)]
        output: PathBuf,
    },
    /// Capture, compile, lint, verify and export a fixed local task. Requires --yes.
    Run {
        path: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    /// Execute the fixed recipe used by Capture and Replay. No shell or network.
    #[command(hide = true)]
    Produce {
        #[arg(value_enum)]
        scenario: Scenario,
        #[arg(long)]
        input: PathBuf,
    },
}

pub(crate) fn run(command: StarterCommand) -> ExitCode {
    let result = match command {
        StarterCommand::New { scenario, output } => scaffold(scenario, &output),
        StarterCommand::Run { path, output, yes } => pipeline(&path, &output, yes),
        StarterCommand::Produce { scenario, input } => {
            if input != Path::new("inputs/request.json") {
                Err(fail("use the fixed inputs/request.json path"))
            } else {
                produce(scenario)
            }
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Starter: {error}");
            ExitCode::from(2)
        }
    }
}
fn fail(message: &str) -> io::Error {
    io::Error::other(message)
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(serde_json::to_string_pretty(value)?.as_bytes())?;
    file.write_all(b"\n")
}
fn safe_ancestors(path: &Path) -> bool {
    !path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
        && path.ancestors().all(|part| {
            part.as_os_str().is_empty()
                || fs::symlink_metadata(part)
                    .map(|m| !m.file_type().is_symlink() || platform_alias(part))
                    .unwrap_or(true)
        })
}
fn platform_alias(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        [("/var", "/private/var"), ("/tmp", "/private/tmp")]
            .iter()
            .any(|(alias, target)| {
                path == Path::new(alias)
                    && fs::canonicalize(path).ok().as_deref() == Some(Path::new(target))
            })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}
fn new_target(output: &Path) -> io::Result<(PathBuf, tempfile::TempDir)> {
    if !safe_ancestors(output) || fs::symlink_metadata(output).is_ok() {
        return Err(fail(
            "choose a new output directory without links or parent traversal",
        ));
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent =
        fs::canonicalize(parent).map_err(|_| fail("create the output parent directory first"))?;
    let target = parent.join(
        output
            .file_name()
            .ok_or_else(|| fail("output needs a directory name"))?,
    );
    let stage = tempfile::Builder::new()
        .prefix(".skilltape-starter-")
        .tempdir_in(parent)?;
    Ok((target, stage))
}
fn read_bounded(path: &Path) -> io::Result<Vec<u8>> {
    if !safe_ancestors(path) {
        return Err(fail("input links and parent traversal are not allowed"));
    }
    let before = fs::symlink_metadata(path).map_err(|_| {
        fail("required input file is missing; check the numbered file entry in inputs/request.json")
    })?;
    if !before.is_file() || before.len() > LIMIT {
        return Err(fail("input must be a regular file of at most 1 MiB"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    let opened = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != opened.dev() || before.ino() != opened.ino() {
            return Err(fail("input changed during open"));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if opened.file_attributes() & 0x400 != 0 {
            return Err(fail("input links are not allowed"));
        }
    }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    let after = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(fail("input changed during reading"));
        }
    }
    if bytes.len() as u64 != before.len()
        || before.len() != opened.len()
        || after.len() != before.len()
        || before.modified()? != after.modified()?
    {
        return Err(fail(
            "input changed during reading; keep inputs unchanged and try a new output directory",
        ));
    }
    Ok(bytes)
}
fn scaffold(scenario: Scenario, output: &Path) -> io::Result<()> {
    let (target, stage) = new_target(output)?;
    fs::create_dir(stage.path().join("inputs"))?;
    let request = match scenario {
        Scenario::Inventory => {
            fs::create_dir(stage.path().join("inputs/files"))?;
            fs::write(
                stage.path().join("inputs/files/sample.txt"),
                b"Reviewed local sample.\n",
            )?;
            json!({"files":[{"path":"sample.txt","sha256":hash(b"Reviewed local sample.\n")}]})
        }
        Scenario::Reconcile => {
            json!({"expected":[{"item":"sample-1","quantity":5}],"received":[{"item":"sample-1","quantity":4}]})
        }
    };
    write_json(&stage.path().join("inputs/request.json"), &request)?;
    write_json(
        &stage.path().join("starter.json"),
        &json!({"schema":"skilltape.dev/starter/v1","scenario":scenario.name()}),
    )?;
    fs::write(
        stage.path().join("README.md"),
        include_str!("../../../docs/starter-inputs.md"),
    )?;
    publish_directory_noreplace(stage.path(), &target)?;
    println!("Starter materials created. Review inputs/request.json, then run `skilltape starter run PATH --output NEW_RUN_DIRECTORY --yes`. No task has run yet.");
    Ok(())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recipe {
    schema: String,
    scenario: Scenario,
}
fn pipeline(source: &Path, output: &Path, yes: bool) -> io::Result<()> {
    if !yes {
        return Err(fail(
            "review the fixed task and inputs, then pass --yes to execute locally",
        ));
    }
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return Err(fail("starter execution requires macOS or Linux with the supported sandbox; Windows remains read-only"));
    }
    let recipe: Recipe = serde_json::from_slice(&read_bounded(&source.join("starter.json"))?)
        .map_err(|_| fail("starter.json is invalid; recreate materials with starter new"))?;
    if recipe.schema != "skilltape.dev/starter/v1" {
        return Err(fail("unsupported starter schema"));
    }
    let (target, stage) = new_target(output)?;
    validate_output_root(source, &source.join("inputs"), &target)
        .map_err(|_| fail("run output must be outside starter materials"))?;
    fs::create_dir(stage.path().join("capture"))?;
    snapshot_inputs(&source.join("inputs"), &stage.path().join("capture/inputs")).map_err(|_| fail("inputs could not be snapshotted: reject links, changed files and capacity overflow (10,000 entries, 16 MiB/file, 64 MiB total)"))?;
    // Validate before Capture. This computes but never writes task results.
    report(recipe.scenario, &stage.path().join("capture/inputs"))?;
    fs::create_dir(stage.path().join("tapes"))?;
    fs::create_dir(stage.path().join("skills"))?;
    fs::create_dir(stage.path().join("deliveries"))?;
    publish_directory_noreplace(stage.path(), &target)?;
    let executable = std::env::current_exe()?.to_string_lossy().into_owned();
    let workspace = target.join("capture");
    let tape = target.join("tapes/task");
    step(
        "Capture",
        super::capture_command::run(super::capture_command::CaptureConfig {
            name: recipe.scenario.name().into(),
            workspace: Some(workspace.clone()),
            command: Some(executable),
            args: vec![
                "starter".into(),
                "produce".into(),
                recipe.scenario.name().into(),
                "--input".into(),
                "inputs/request.json".into(),
            ],
            interactive: false,
            output: Some(tape.clone()),
            allow_env: vec![],
            max_output_bytes: 4096,
            json: false,
            yes: true,
        }),
    )?;
    let expected = hash(&read_bounded(&workspace.join("outputs/report.json"))?);
    let package = target.join("skills/task");
    step(
        "Compile",
        super::compile_command::run(super::compile_command::CompileConfig {
            tape,
            output: package.clone(),
            provider: None,
            accept_proposal: false,
        }),
    )?;
    step("Lint", super::lint(package.clone(), true, false))?;
    let delivery = target.join("deliveries/task");
    step(
        "Verify",
        super::run_command::verify(super::run_command::VerifyConfig {
            skill_path: package.clone(),
            input: Some(workspace.join("inputs")),
            receipt: None,
            context: None,
            delivery_dir: Some(delivery.clone()),
            expected_files: vec![],
            expected_hashes: vec![format!("report.json={expected}")],
            json: false,
        }),
    )?;
    step(
        "Export",
        super::export_command::run(super::export_command::ExportConfig {
            skill_path: package,
            target: "generic".into(),
            output: target.join("export"),
            receipt: Some(delivery.join("receipt.json")),
            json: false,
        }),
    )?;
    println!("Completed a real run. Results: deliveries/task/artifacts/report.json. Open Console with `skilltape console --workspace RUN_DIRECTORY --open`. Console is read-only.");
    Ok(())
}
fn step(name: &str, status: ExitCode) -> io::Result<()> {
    if status == ExitCode::SUCCESS {
        Ok(())
    } else {
        Err(fail(&format!("{name} failed. Completed stages remain in the run directory; inspect that stage, correct inputs, and choose a new run directory. No successful delivery is claimed.")))
    }
}
fn produce(scenario: Scenario) -> io::Result<()> {
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return Err(fail(
            "starter execution requires a supported macOS or Linux sandbox",
        ));
    }
    let value = report(scenario, Path::new("inputs"))?;
    fs::create_dir("outputs")?;
    write_json(Path::new("outputs/report.json"), &value)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    files: Vec<FileSpec>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSpec {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Quantities {
    expected: Vec<Quantity>,
    received: Vec<Quantity>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Quantity {
    item: String,
    quantity: u64,
}
fn report(scenario: Scenario, input: &Path) -> io::Result<Value> {
    let bytes = read_bounded(&input.join("request.json"))?;
    match scenario {
        Scenario::Inventory => {
            let request: Inventory = serde_json::from_slice(&bytes).map_err(|_| {
                fail("inputs/request.json: expected files with path and sha256 fields")
            })?;
            if request.files.is_empty() || request.files.len() > 128 {
                return Err(fail("inputs/request.json: use 1 to 128 files"));
            }
            let mut seen = BTreeSet::new();
            let mut rows = Vec::new();
            for (index, file) in request.files.iter().enumerate() {
                if file.path.len() > 128
                    || file.path.is_empty()
                    || file.path.contains(['/', '\\'])
                    || file.path == "."
                    || file.path == ".."
                    || file.path.chars().any(char::is_control)
                    || !seen.insert(&file.path)
                    || file.sha256.len() != 64
                    || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(fail(&format!("inputs/request.json: file entry {} needs a unique plain file name and 64 hexadecimal hash characters", index+1)));
                }
                let content = read_bounded(&input.join("files").join(&file.path)).map_err(|_| fail(&format!("inputs/files: file entry {} is missing, unsafe, changed or larger than 1 MiB", index+1)))?;
                let digest = hash(&content);
                if !digest.eq_ignore_ascii_case(&file.sha256) {
                    return Err(fail(&format!("inputs/request.json: file entry {} hash differs; review the source file before changing its expected hash", index+1)));
                }
                rows.push(json!({"path":file.path,"bytes":content.len(),"sha256":digest}));
            }
            Ok(json!({"scenario":"inventory","matched":true,"files":rows}))
        }
        Scenario::Reconcile => {
            let request: Quantities = serde_json::from_slice(&bytes).map_err(|_| fail("inputs/request.json: expected expected/received arrays with item and nonnegative integer quantity fields"))?;
            if request.expected.is_empty()
                || request.expected.len() > 1000
                || request.received.len() > 1000
            {
                return Err(fail("inputs/request.json: expected needs 1 to 1000 rows; received allows up to 1000"));
            }
            let mut totals: BTreeMap<String, (i64, i64)> = BTreeMap::new();
            for (side, rows) in [request.expected, request.received].into_iter().enumerate() {
                for (index, row) in rows.into_iter().enumerate() {
                    if row.item.is_empty()
                        || row.item.len() > 128
                        || row.item.chars().any(char::is_control)
                        || row.quantity > 1_000_000_000
                    {
                        return Err(fail(&format!("inputs/request.json: {} row {} needs an item of 1 to 128 bytes and quantity from 0 to 1000000000", if side==0 {"expected"} else {"received"},index+1)));
                    }
                    let total = totals.entry(row.item).or_default();
                    if side == 0 {
                        total.0 += row.quantity as i64;
                    } else {
                        total.1 += row.quantity as i64;
                    }
                }
            }
            let balanced = totals.values().all(|(a, b)| a == b);
            let rows: Vec<_> = totals.into_iter().map(|(item,(expected,received))|json!({"item":item,"expected":expected,"received":received,"difference":received-expected})).collect();
            Ok(json!({"scenario":"reconcile","balanced":balanced,"rows":rows}))
        }
    }
}
