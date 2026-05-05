use std::path::{Path, PathBuf};
use std::time::Instant;

use mercurio_core::{StagedEditorFileDto, WorkspaceService};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    All,
    Edited,
    Folders,
}

#[derive(Debug)]
struct BenchmarkTarget {
    name: String,
    path: PathBuf,
}

#[derive(Debug)]
struct SourceStats {
    file_count: usize,
    byte_count: u64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("benchmark_examples: {err}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (mode, root) = parse_args()?;
    let targets = collect_targets(mode, &root)?;
    if targets.is_empty() {
        return Err(format!("no benchmark targets found under {}", root.display()).into());
    }

    if mode == Mode::Edited {
        println!(
            "target,files,bytes,edited_path,cold_workspace_ms,unchanged_warm_ms,unchanged_hits,unchanged_misses,edited_warm_ms,edited_hits,edited_misses,cache_entries,cache_capacity"
        );
    } else {
        println!(
            "target,files,bytes,cold_diagnostics_ms,cold_workspace_ms,warm_scope_ms,warm_success,warm_failure"
        );
    }
    for target in targets {
        let stats = collect_source_stats(&target.path)?;

        if mode == Mode::Edited {
            run_edited_target(&target, &stats)?;
            continue;
        }

        let diagnostics_start = Instant::now();
        let diagnostics_service =
            WorkspaceService::from_workspace_root_diagnostics_only(&target.path)?;
        let diagnostics_response = diagnostics_service
            .compile_project_scope_diagnostics_only(".", &Vec::<StagedEditorFileDto>::new())?;
        let cold_diagnostics_ms = diagnostics_start.elapsed().as_millis();

        let cold_start = Instant::now();
        let service = WorkspaceService::from_workspace_root_compiled(&target.path)?;
        let cold_workspace_ms = cold_start.elapsed().as_millis();

        let warm_start = Instant::now();
        let _response = service.compile_project_scope(".", &Vec::<StagedEditorFileDto>::new())?;
        let warm_scope_ms = warm_start.elapsed().as_millis();

        println!(
            "{},{},{},{},{},{},{},{}",
            csv_escape(&target.name),
            stats.file_count,
            stats.byte_count,
            cold_diagnostics_ms,
            cold_workspace_ms,
            warm_scope_ms,
            diagnostics_response.success_count,
            diagnostics_response.failure_count
        );
    }

    Ok(())
}

fn parse_args() -> Result<(Mode, PathBuf), String> {
    let mut mode = Mode::Folders;
    let mut root = mercurio_core::repo_path("examples/src/examples");
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--all" => mode = Mode::All,
            "--edited" => mode = Mode::Edited,
            "--folders" => mode = Mode::Folders,
            "--root" => {
                root = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or_else(|| "--root requires a path".to_string())?;
            }
            "-h" | "--help" => {
                return Err(usage());
            }
            other => {
                return Err(format!("unknown argument `{other}`\n\n{}", usage()));
            }
        }
    }

    Ok((mode, root))
}

fn usage() -> String {
    "usage: cargo run -q -p mercurio-tools --bin benchmark_examples -- [--folders|--all|--edited] [--root PATH]"
        .to_string()
}

fn collect_targets(mode: Mode, root: &Path) -> Result<Vec<BenchmarkTarget>, std::io::Error> {
    if matches!(mode, Mode::All | Mode::Edited) {
        return Ok(vec![BenchmarkTarget {
            name: root.display().to_string(),
            path: root.to_path_buf(),
        }]);
    }

    let mut targets = std::fs::read_dir(root)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() || !contains_model_source(&path).unwrap_or(false) {
                return None;
            }

            Some(BenchmarkTarget {
                name: entry.file_name().to_string_lossy().into_owned(),
                path,
            })
        })
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(targets)
}

fn run_edited_target(
    target: &BenchmarkTarget,
    stats: &SourceStats,
) -> Result<(), Box<dyn std::error::Error>> {
    let edited_source = first_model_source(&target.path)?
        .ok_or_else(|| format!("no model source found under {}", target.path.display()))?;
    let edited_path = relative_model_path(&target.path, &edited_source)?;
    let edited_content = format!(
        "{}\n// benchmark staged edit\n",
        std::fs::read_to_string(&edited_source)?
    );

    let cold_start = Instant::now();
    let service = WorkspaceService::from_workspace_root_compiled(&target.path)?;
    let cold_workspace_ms = cold_start.elapsed().as_millis();

    let unchanged_start = Instant::now();
    let _unchanged_response =
        service.compile_project_scope(".", &Vec::<StagedEditorFileDto>::new())?;
    let unchanged_warm_ms = unchanged_start.elapsed().as_millis();
    let unchanged_stats = service.semantic_compile_cache_stats();

    let staged_files = vec![StagedEditorFileDto {
        path: edited_path.clone(),
        content: edited_content,
    }];
    let edited_start = Instant::now();
    let _edited_response = service.compile_project_scope(".", &staged_files)?;
    let edited_warm_ms = edited_start.elapsed().as_millis();
    let edited_stats = service.semantic_compile_cache_stats();

    println!(
        "{},{},{},{},{},{},{},{},{},{},{},{},{}",
        csv_escape(&target.name),
        stats.file_count,
        stats.byte_count,
        csv_escape(&edited_path),
        cold_workspace_ms,
        unchanged_warm_ms,
        unchanged_stats.last_hits,
        unchanged_stats.last_misses,
        edited_warm_ms,
        edited_stats.last_hits,
        edited_stats.last_misses,
        edited_stats.entries,
        edited_stats.capacity
    );

    Ok(())
}

fn first_model_source(path: &Path) -> Result<Option<PathBuf>, std::io::Error> {
    let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if let Some(source) = first_model_source(&path)? {
                return Ok(Some(source));
            }
        } else if is_model_source(&path) {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn relative_model_path(root: &Path, path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/"))
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
        } else if is_model_source(&path) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn collect_source_stats(path: &Path) -> Result<SourceStats, std::io::Error> {
    let mut stats = SourceStats {
        file_count: 0,
        byte_count: 0,
    };
    collect_source_stats_into(path, &mut stats)?;
    Ok(stats)
}

fn collect_source_stats_into(path: &Path, stats: &mut SourceStats) -> Result<(), std::io::Error> {
    let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_source_stats_into(&path, stats)?;
        } else if is_model_source(&path) {
            stats.file_count += 1;
            stats.byte_count += entry.metadata()?.len();
        }
    }

    Ok(())
}

fn is_model_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("sysml" | "kerml")
    )
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
