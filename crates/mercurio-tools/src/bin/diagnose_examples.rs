use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mercurio_core::{StagedEditorFileDto, WorkspaceService};
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Serialize)]
struct Report {
    generated_at_utc: String,
    root: String,
    mode: String,
    aggregate: TargetSummary,
    targets: Vec<TargetReport>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct TargetSummary {
    file_count: usize,
    success_count: usize,
    failure_count: usize,
    primary_kind_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct TargetReport {
    name: String,
    path: String,
    summary: TargetSummary,
    files: Vec<FileReport>,
}

#[derive(Debug, Serialize)]
struct FileReport {
    path: String,
    ok: bool,
    semantic_status: String,
    diagnostic_count: usize,
    primary_kind: Option<String>,
    primary_message: Option<String>,
    primary_line: Option<usize>,
    primary_column: Option<usize>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    let targets = collect_targets(args.mode, &args.root)?
        .into_iter()
        .map(run_target)
        .collect::<Result<Vec<_>, _>>()?;
    let aggregate = summarize_targets(&targets);

    let report = Report {
        generated_at_utc: OffsetDateTime::now_utc().format(&Rfc3339)?,
        root: args.root.display().to_string(),
        mode: args.mode.as_str().to_string(),
        aggregate,
        targets,
    };

    if let Some(output) = args.output {
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&output, serde_json::to_string_pretty(&report)?)?;
        println!("wrote {}", output.display());
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }

    Ok(())
}

#[derive(Debug)]
struct Args {
    root: PathBuf,
    output: Option<PathBuf>,
    mode: Mode,
}

#[derive(Debug, Clone, Copy)]
enum Mode {
    Single,
    Folders,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Folders => "folders",
        }
    }
}

#[derive(Debug)]
struct Target {
    name: String,
    path: PathBuf,
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut root = mercurio_core::repo_path("examples/src/examples");
    let mut output = None;
    let mut mode = Mode::Single;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--single" => mode = Mode::Single,
            "--folders" => mode = Mode::Folders,
            "--root" => {
                index += 1;
                root = PathBuf::from(args.get(index).ok_or("missing value for --root")?);
            }
            "--out" => {
                index += 1;
                output = Some(PathBuf::from(
                    args.get(index).ok_or("missing value for --out")?,
                ));
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            unknown => return Err(format!("unknown argument: {unknown}").into()),
        }
        index += 1;
    }

    Ok(Args { root, output, mode })
}

fn print_usage() {
    println!(
        "Usage: cargo run -q -p mercurio-tools --bin diagnose_examples -- [--single|--folders] [--root PATH] [--out PATH]"
    );
}

fn collect_targets(mode: Mode, root: &Path) -> Result<Vec<Target>, std::io::Error> {
    match mode {
        Mode::Single => Ok(vec![Target {
            name: root.display().to_string(),
            path: root.to_path_buf(),
        }]),
        Mode::Folders => {
            let mut targets = std::fs::read_dir(root)?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter_map(|entry| {
                    let path = entry.path();
                    if !path.is_dir() || !contains_model_source(&path).unwrap_or(false) {
                        return None;
                    }

                    Some(Target {
                        name: entry.file_name().to_string_lossy().into_owned(),
                        path,
                    })
                })
                .collect::<Vec<_>>();
            targets.sort_by(|left, right| left.name.cmp(&right.name));
            Ok(targets)
        }
    }
}

fn run_target(target: Target) -> Result<TargetReport, Box<dyn std::error::Error>> {
    let service = WorkspaceService::from_workspace_root_diagnostics_only(&target.path)?;
    let response =
        service.compile_project_scope_diagnostics_only(".", &Vec::<StagedEditorFileDto>::new())?;

    let mut primary_kind_counts = BTreeMap::new();
    let files = response
        .results
        .into_iter()
        .map(|result| {
            let primary = result.diagnostics.first();
            let primary_message = primary.map(|diagnostic| diagnostic.message.clone());
            let primary_kind = primary_message.as_deref().map(classify_problem_kind);
            if let Some(kind) = &primary_kind {
                *primary_kind_counts.entry(kind.clone()).or_insert(0) += 1;
            }

            FileReport {
                path: result.path,
                ok: result.ok,
                semantic_status: result.semantic_status,
                diagnostic_count: result.diagnostics.len(),
                primary_kind,
                primary_message,
                primary_line: primary.map(|diagnostic| diagnostic.start_line_number),
                primary_column: primary.map(|diagnostic| diagnostic.start_column),
            }
        })
        .collect::<Vec<_>>();

    Ok(TargetReport {
        name: target.name,
        path: target.path.display().to_string(),
        summary: TargetSummary {
            file_count: response.file_count,
            success_count: response.success_count,
            failure_count: response.failure_count,
            primary_kind_counts,
        },
        files,
    })
}

fn summarize_targets(targets: &[TargetReport]) -> TargetSummary {
    let mut summary = TargetSummary::default();
    for target in targets {
        summary.file_count += target.summary.file_count;
        summary.success_count += target.summary.success_count;
        summary.failure_count += target.summary.failure_count;
        for (kind, count) in &target.summary.primary_kind_counts {
            *summary.primary_kind_counts.entry(kind.clone()).or_insert(0) += count;
        }
    }
    summary
}

fn contains_model_source(path: &Path) -> Result<bool, std::io::Error> {
    let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if contains_model_source(&path)? {
                return Ok(true);
            }
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("sysml" | "kerml")
        ) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn classify_problem_kind(message: &str) -> String {
    let normalized = message
        .to_ascii_lowercase()
        .replace('\\', "/")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if normalized.contains("expected `") || normalized.contains("expected a declaration") {
        "parse_expected_token".to_string()
    } else if normalized.contains("unresolved") || normalized.contains("could not resolve") {
        "unresolved_reference".to_string()
    } else if normalized.contains("duplicate") {
        "duplicate_definition".to_string()
    } else if normalized.contains("import") {
        "import_error".to_string()
    } else if normalized.contains("missing construct mapping") {
        "missing_construct_mapping".to_string()
    } else if normalized.contains("missing emission mapping") {
        "missing_emission_mapping".to_string()
    } else if normalized.contains("recovery stopped") {
        "partial_recovery_limit".to_string()
    } else {
        "other".to_string()
    }
}
