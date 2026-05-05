use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use mercurio_core::frontend::ast::{Declaration, SysmlModule};
use mercurio_core::frontend::diagnostics::Diagnostic;
use mercurio_core::frontend::kerml::{compile_kerml_text, parse_kerml};
use mercurio_core::frontend::sysml::{compile_sysml_text_with_context_report, parse_sysml};
use mercurio_core::{
    KirDocument, KparPackageBuild, KparPackageSource, LibraryProviderConfig, LintReport,
    LintSeverity, SemanticCompileStatus, SourceLanguage, default_stdlib_path, lint_text,
    write_kpar_package,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "mercurio")]
#[command(about = "Parse, compile, and lint SysML v2 and KerML sources.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Parse(ParseCommand),
    Compile(CompileCommand),
    Lint(LintCommand),
    Package(PackageCommand),
}

#[derive(Debug, Args)]
struct ParseCommand {
    #[command(flatten)]
    input: SingleInput,
    #[arg(long, value_enum)]
    language: Option<LanguageArg>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
}

#[derive(Debug, Args)]
struct CompileCommand {
    #[command(flatten)]
    input: SingleInput,
    #[arg(long, value_enum)]
    language: Option<LanguageArg>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
    #[arg(long)]
    stdlib: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct LintCommand {
    #[arg(long = "file")]
    files: Vec<PathBuf>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long, value_enum)]
    language: Option<LanguageArg>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
    #[arg(long)]
    stdlib: Option<PathBuf>,
    #[arg(long, alias = "deny-warnings")]
    warnings_as_errors: bool,
    #[arg(long)]
    quiet: bool,
}

#[derive(Debug, Args)]
struct PackageCommand {
    #[command(subcommand)]
    command: PackageSubcommand,
}

#[derive(Debug, Subcommand)]
enum PackageSubcommand {
    Build(PackageBuildCommand),
}

#[derive(Debug, Args)]
struct PackageBuildCommand {
    #[arg(long = "file")]
    files: Vec<PathBuf>,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    stdlib: Option<PathBuf>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    version: Option<String>,
    #[arg(long)]
    quiet: bool,
}

#[derive(Debug, Args)]
struct SingleInput {
    #[arg(long)]
    file: Option<PathBuf>,
    #[arg(long)]
    text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LanguageArg {
    Auto,
    Sysml,
    Kerml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone)]
struct SourceInput {
    source_name: String,
    language: SourceLanguage,
    content: String,
}

#[derive(Debug)]
struct CliError {
    message: String,
    code: i32,
}

impl CliError {
    fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
        }
    }

    fn execution(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(result) => {
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            std::process::exit(result.exit_code);
        }
        Err(error) => {
            eprintln!("mercurio: {error}");
            std::process::exit(error.code);
        }
    }
}

#[derive(Debug)]
struct RunResult {
    exit_code: i32,
    stdout: String,
}

fn run(cli: Cli) -> Result<RunResult, CliError> {
    match cli.command {
        Command::Parse(command) => run_parse(command),
        Command::Compile(command) => run_compile(command),
        Command::Lint(command) => run_lint(command),
        Command::Package(command) => run_package(command),
    }
}

fn run_parse(command: ParseCommand) -> Result<RunResult, CliError> {
    let source = read_single_input(&command.input, command.language)?;
    let parsed = parse_source(&source);
    let failed = parsed.is_err();
    let response = match parsed {
        Ok(module) => ParseResponse {
            source: source.source_name,
            language: source.language,
            status: "ok",
            diagnostics: Vec::new(),
            ast: Some(module),
        },
        Err(diagnostic) => ParseResponse {
            source: source.source_name,
            language: source.language,
            status: "failed",
            diagnostics: vec![diagnostic],
            ast: None,
        },
    };

    let stdout = match command.format {
        OutputFormat::Text => format_parse_text(&response),
        OutputFormat::Json => to_pretty_json(&response)?,
    };

    Ok(RunResult {
        exit_code: if failed { 1 } else { 0 },
        stdout,
    })
}

fn run_compile(command: CompileCommand) -> Result<RunResult, CliError> {
    let source = read_single_input(&command.input, command.language)?;
    let stdlib = load_stdlib(command.stdlib.as_deref())?;
    let response = compile_source(&source, &stdlib);
    let failed = response.status == "failed" || !response.diagnostics.is_empty();
    let stdout = match command.format {
        OutputFormat::Text => format_compile_text(&response),
        OutputFormat::Json => to_pretty_json(&response)?,
    };

    Ok(RunResult {
        exit_code: if failed { 1 } else { 0 },
        stdout,
    })
}

fn run_lint(command: LintCommand) -> Result<RunResult, CliError> {
    let sources = read_lint_inputs(&command)?;
    let stdlib = load_stdlib(command.stdlib.as_deref())?;
    let context_modules = sources
        .iter()
        .filter_map(|source| parse_source(source).ok())
        .collect::<Vec<_>>();
    let reports = sources
        .iter()
        .map(|source| {
            lint_text(
                &source.content,
                &source.source_name,
                source.language,
                &context_modules,
                &stdlib,
            )
        })
        .collect::<Vec<_>>();

    let failing = lint_should_fail(&reports, command.warnings_as_errors);
    let stdout = if command.quiet {
        String::new()
    } else {
        match command.format {
            OutputFormat::Text => format_lint_text(&reports),
            OutputFormat::Json => to_pretty_json(&reports)?,
        }
    };

    Ok(RunResult {
        exit_code: if failing { 1 } else { 0 },
        stdout,
    })
}

fn run_package(command: PackageCommand) -> Result<RunResult, CliError> {
    match command.command {
        PackageSubcommand::Build(command) => run_package_build(command),
    }
}

fn run_package_build(command: PackageBuildCommand) -> Result<RunResult, CliError> {
    let sources = read_package_sources(&command.files)?;
    let package_name = command
        .name
        .clone()
        .unwrap_or_else(|| derive_package_name(&command.out));
    let package = KparPackageBuild {
        name: package_name,
        version: command.version,
        sources,
    };
    let stdlib = load_stdlib(command.stdlib.as_deref())?;
    let temp_path = temp_kpar_path(&command.out)?;

    write_kpar_package(&temp_path, &package)
        .map_err(|err| CliError::execution(format!("failed to write package: {err}")))?;

    let validation = LibraryProviderConfig::KparFile {
        path: temp_path.display().to_string(),
    }
    .resolve_with_context("package", None, Some(&stdlib));

    let artifact = match validation {
        Ok(artifact) => artifact,
        Err(err) => {
            let _ = std::fs::remove_file(&temp_path);
            let stdout = format!("package validation failed: {err}\n");
            return Ok(RunResult {
                exit_code: 1,
                stdout,
            });
        }
    };

    if let Some(parent) = command.out.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            CliError::execution(format!(
                "failed to create output directory {}: {err}",
                parent.display()
            ))
        })?;
    }
    std::fs::copy(&temp_path, &command.out).map_err(|err| {
        CliError::execution(format!(
            "failed to write output package {}: {err}",
            command.out.display()
        ))
    })?;
    std::fs::remove_file(&temp_path).map_err(|err| {
        CliError::execution(format!(
            "failed to remove temporary package {}: {err}",
            temp_path.display()
        ))
    })?;

    let stdout = if command.quiet {
        String::new()
    } else {
        format!(
            "wrote: {}\nsources: {}\nelements: {}\n",
            command.out.display(),
            package.sources.len(),
            artifact.document.elements.len()
        )
    };

    Ok(RunResult {
        exit_code: 0,
        stdout,
    })
}

fn read_single_input(
    input: &SingleInput,
    language: Option<LanguageArg>,
) -> Result<SourceInput, CliError> {
    match (&input.file, &input.text) {
        (Some(_), Some(_)) => Err(CliError::usage("provide exactly one of --file or --text")),
        (None, None) => Err(CliError::usage("provide exactly one of --file or --text")),
        (Some(path), None) => read_file_source(path, language),
        (None, Some(text)) => read_text_source(text, language),
    }
}

fn read_package_sources(paths: &[PathBuf]) -> Result<Vec<KparPackageSource>, CliError> {
    if paths.is_empty() {
        return Err(CliError::usage("provide at least one --file"));
    }

    let mut sources = Vec::new();
    for path in paths {
        collect_package_sources(path, path, &mut sources)?;
    }
    sources.sort_by(|left, right| left.path.cmp(&right.path));

    let mut seen = std::collections::BTreeSet::new();
    for source in &sources {
        if !seen.insert(source.path.clone()) {
            return Err(CliError::usage(format!(
                "duplicate package source path: {}",
                source.path
            )));
        }
    }

    Ok(sources)
}

fn collect_package_sources(
    root: &Path,
    path: &Path,
    sources: &mut Vec<KparPackageSource>,
) -> Result<(), CliError> {
    if path.is_file() {
        if SourceLanguage::from_path(path).is_none() {
            return Err(CliError::usage(format!(
                "unsupported file extension: {}",
                path.display()
            )));
        }
        let content = std::fs::read_to_string(path).map_err(|err| {
            CliError::execution(format!("failed to read {}: {err}", path.display()))
        })?;
        sources.push(KparPackageSource {
            path: package_entry_path(root, path)?,
            content,
        });
        return Ok(());
    }

    if path.is_dir() {
        let mut entries = std::fs::read_dir(path)
            .map_err(|err| {
                CliError::execution(format!(
                    "failed to read directory {}: {err}",
                    path.display()
                ))
            })?
            .collect::<Result<Vec<_>, std::io::Error>>()
            .map_err(|err| CliError::execution(format!("failed to read directory entry: {err}")))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let entry_path = entry.path();
            if entry_path.is_dir() || SourceLanguage::from_path(&entry_path).is_some() {
                collect_package_sources(root, &entry_path, sources)?;
            }
        }
        return Ok(());
    }

    Err(CliError::usage(format!(
        "input does not exist: {}",
        path.display()
    )))
}

fn package_entry_path(root: &Path, path: &Path) -> Result<String, CliError> {
    let relative = if root.is_dir() {
        path.strip_prefix(root).unwrap_or(path)
    } else {
        path.file_name()
            .map(Path::new)
            .ok_or_else(|| CliError::usage(format!("invalid source path: {}", path.display())))?
    };
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn read_lint_inputs(command: &LintCommand) -> Result<Vec<SourceInput>, CliError> {
    if command.text.is_some() && !command.files.is_empty() {
        return Err(CliError::usage("provide --text or --file, not both"));
    }
    if command.text.is_none() && command.files.is_empty() {
        return Err(CliError::usage("provide at least one --file or --text"));
    }
    if let Some(text) = &command.text {
        return Ok(vec![read_text_source(text, command.language)?]);
    }

    let mut files = Vec::new();
    for path in &command.files {
        collect_lint_files(path, &mut files, command.language)?;
    }
    files.sort();
    files.dedup();

    files
        .iter()
        .map(|path| read_file_source(path, command.language))
        .collect()
}

fn read_text_source(text: &str, language: Option<LanguageArg>) -> Result<SourceInput, CliError> {
    let language = match language {
        None => SourceLanguage::Sysml,
        Some(LanguageArg::Auto) => {
            return Err(CliError::usage(
                "--language auto is not valid with --text; use sysml or kerml",
            ));
        }
        Some(LanguageArg::Sysml) => SourceLanguage::Sysml,
        Some(LanguageArg::Kerml) => SourceLanguage::Kerml,
    };

    Ok(SourceInput {
        source_name: inline_source_name(language).to_string(),
        language,
        content: text.to_string(),
    })
}

fn read_file_source(path: &Path, language: Option<LanguageArg>) -> Result<SourceInput, CliError> {
    let resolved_language = resolve_file_language(path, language)?;
    let content = std::fs::read_to_string(path)
        .map_err(|err| CliError::execution(format!("failed to read {}: {err}", path.display())))?;
    Ok(SourceInput {
        source_name: path.display().to_string(),
        language: resolved_language,
        content,
    })
}

fn resolve_file_language(
    path: &Path,
    language: Option<LanguageArg>,
) -> Result<SourceLanguage, CliError> {
    match language {
        None | Some(LanguageArg::Auto) => SourceLanguage::from_path(path).ok_or_else(|| {
            CliError::usage(format!(
                "cannot infer language from {}; use --language sysml|kerml",
                path.display()
            ))
        }),
        Some(LanguageArg::Sysml) => Ok(SourceLanguage::Sysml),
        Some(LanguageArg::Kerml) => Ok(SourceLanguage::Kerml),
    }
}

fn collect_lint_files(
    path: &Path,
    files: &mut Vec<PathBuf>,
    language: Option<LanguageArg>,
) -> Result<(), CliError> {
    if path.is_file() {
        if !matches!(language, None | Some(LanguageArg::Auto))
            || SourceLanguage::from_path(path).is_some()
        {
            files.push(path.to_path_buf());
            return Ok(());
        }
        return Err(CliError::usage(format!(
            "unsupported file extension: {}",
            path.display()
        )));
    }
    if path.is_dir() {
        for entry in std::fs::read_dir(path).map_err(|err| {
            CliError::execution(format!(
                "failed to read directory {}: {err}",
                path.display()
            ))
        })? {
            let entry = entry.map_err(|err| {
                CliError::execution(format!("failed to read directory entry: {err}"))
            })?;
            collect_lint_files(&entry.path(), files, language)?;
        }
        return Ok(());
    }

    Err(CliError::usage(format!(
        "input does not exist: {}",
        path.display()
    )))
}

fn parse_source(source: &SourceInput) -> Result<SysmlModule, Diagnostic> {
    match source.language {
        SourceLanguage::Sysml => parse_sysml(&source.content),
        SourceLanguage::Kerml => parse_kerml(&source.content),
    }
}

fn compile_source(source: &SourceInput, stdlib: &KirDocument) -> CompileResponse {
    match source.language {
        SourceLanguage::Sysml => {
            let report = compile_sysml_text_with_context_report(
                &source.content,
                &source.source_name,
                &[],
                stdlib,
            );
            CompileResponse {
                source: source.source_name.clone(),
                language: source.language,
                status: compile_status_str(report.status),
                diagnostics: report.diagnostics,
                document: report.document,
            }
        }
        SourceLanguage::Kerml => {
            match compile_kerml_text(&source.content, &source.source_name, stdlib) {
                Ok(document) => CompileResponse {
                    source: source.source_name.clone(),
                    language: source.language,
                    status: "ok",
                    diagnostics: Vec::new(),
                    document: Some(document),
                },
                Err(diagnostic) => CompileResponse {
                    source: source.source_name.clone(),
                    language: source.language,
                    status: "failed",
                    diagnostics: vec![diagnostic],
                    document: None,
                },
            }
        }
    }
}

fn load_stdlib(path: Option<&Path>) -> Result<KirDocument, CliError> {
    let path = path
        .map(Path::to_path_buf)
        .unwrap_or_else(default_stdlib_path);
    KirDocument::from_path(&path).map_err(|err| {
        CliError::execution(format!("failed to load stdlib {}: {err}", path.display()))
    })
}

fn compile_status_str(status: SemanticCompileStatus) -> &'static str {
    match status {
        SemanticCompileStatus::Ok => "ok",
        SemanticCompileStatus::Partial => "partial",
        SemanticCompileStatus::Failed => "failed",
    }
}

fn inline_source_name(language: SourceLanguage) -> &'static str {
    match language {
        SourceLanguage::Sysml => "<inline.sysml>",
        SourceLanguage::Kerml => "<inline.kerml>",
    }
}

fn derive_package_name(out: &Path) -> String {
    out.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("package")
        .to_string()
}

fn temp_kpar_path(out: &Path) -> Result<PathBuf, CliError> {
    let file_name = out
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| CliError::usage(format!("invalid output path: {}", out.display())))?;
    let temp_name = format!(".{file_name}.{}.tmp", std::process::id());
    Ok(out
        .parent()
        .map(|parent| parent.join(&temp_name))
        .unwrap_or_else(|| PathBuf::from(temp_name)))
}

#[derive(Serialize)]
struct ParseResponse {
    source: String,
    language: SourceLanguage,
    status: &'static str,
    diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ast: Option<SysmlModule>,
}

#[derive(Serialize)]
struct CompileResponse {
    source: String,
    language: SourceLanguage,
    status: &'static str,
    diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    document: Option<KirDocument>,
}

fn format_parse_text(response: &ParseResponse) -> String {
    let mut output = String::new();
    output.push_str(&format!("source: {}\n", response.source));
    output.push_str(&format!("language: {}\n", response.language));
    output.push_str(&format!("status: {}\n", response.status));

    if let Some(module) = &response.ast {
        output.push_str(&format!(
            "package: {}\n",
            module
                .package
                .as_ref()
                .map(|package| package.name.as_dot_string())
                .unwrap_or_else(|| "<none>".to_string())
        ));
        output.push_str(&format!("top-level members: {}\n", module.members.len()));
        for (kind, count) in declaration_counts(module) {
            output.push_str(&format!("{kind}: {count}\n"));
        }
    }

    for diagnostic in &response.diagnostics {
        output.push_str(&format!("error: {diagnostic}\n"));
    }

    output
}

fn format_compile_text(response: &CompileResponse) -> String {
    let mut output = String::new();
    output.push_str(&format!("source: {}\n", response.source));
    output.push_str(&format!("language: {}\n", response.language));
    output.push_str(&format!("status: {}\n", response.status));
    output.push_str(&format!("diagnostics: {}\n", response.diagnostics.len()));
    output.push_str(&format!(
        "elements: {}\n",
        response
            .document
            .as_ref()
            .map(|document| document.elements.len())
            .unwrap_or(0)
    ));
    for diagnostic in &response.diagnostics {
        output.push_str(&format!("diagnostic: {diagnostic}\n"));
    }
    output
}

fn format_lint_text(reports: &[LintReport]) -> String {
    let mut output = String::new();
    for report in reports {
        if report.diagnostics.is_empty() {
            output.push_str(&format!("{}: ok\n", report.source_name));
            continue;
        }
        for diagnostic in &report.diagnostics {
            output.push_str(&format!(
                "{}: {} [{}] {}\n",
                report.source_name, diagnostic.severity, diagnostic.code, diagnostic.message
            ));
            if let Some(span) = &diagnostic.span {
                output.push_str(&format!(
                    "  at {}:{}-{}:{}\n",
                    span.start_line, span.start_col, span.end_line, span.end_col
                ));
            }
        }
    }
    output
}

fn declaration_counts(module: &SysmlModule) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for declaration in &module.members {
        count_declaration(declaration, &mut counts);
    }
    counts
}

fn count_declaration<'a>(declaration: &'a Declaration, counts: &mut BTreeMap<&'static str, usize>) {
    let key = match declaration {
        Declaration::Package(package) => {
            for member in &package.members {
                count_declaration(member, counts);
            }
            "packages"
        }
        Declaration::Import(_) => "imports",
        Declaration::PartDefinition(definition) => {
            for member in &definition.members {
                count_declaration(member, counts);
            }
            "part definitions"
        }
        Declaration::PartUsage(usage) => {
            for member in &usage.body_members {
                count_declaration(member, counts);
            }
            "part usages"
        }
        Declaration::GenericDefinition(definition) => {
            for member in &definition.members {
                count_declaration(member, counts);
            }
            "generic definitions"
        }
        Declaration::GenericUsage(usage) => {
            for member in &usage.body_members {
                count_declaration(member, counts);
            }
            "generic usages"
        }
        Declaration::Alias(_) => "aliases",
    };
    *counts.entry(key).or_insert(0) += 1;
}

fn lint_should_fail(reports: &[LintReport], warnings_as_errors: bool) -> bool {
    reports.iter().any(|report| {
        report.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == LintSeverity::Error
                || (warnings_as_errors && diagnostic.severity == LintSeverity::Warning)
        })
    })
}

fn to_pretty_json(value: &impl Serialize) -> Result<String, CliError> {
    serde_json::to_string_pretty(value)
        .map(|mut value| {
            value.push('\n');
            value
        })
        .map_err(|err| CliError::execution(format!("failed to serialize JSON: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn run_args(args: &[&str]) -> Result<RunResult, CliError> {
        let cli = Cli::try_parse_from(std::iter::once("mercurio").chain(args.iter().copied()))
            .map_err(|err| CliError::usage(err.to_string()))?;
        run(cli)
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{nanos}"))
    }

    #[test]
    fn parse_text_sysml_succeeds() {
        let result = run_args(&["parse", "--text", "package Demo { part def Vehicle; }"]).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("status: ok"));
        assert!(result.stdout.contains("package: Demo"));
    }

    #[test]
    fn parse_file_kerml_succeeds() {
        let root = temp_dir("mercurio-cli-kerml");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("model.kerml");
        std::fs::write(&path, "package Demo { classifier Vehicle; }").unwrap();

        let result = run_args(&["parse", "--file", path.to_str().unwrap()]).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("language: kerml"));
        assert!(result.stdout.contains("package: Demo"));
    }

    #[test]
    fn compile_text_json_returns_document() {
        let result = run_args(&[
            "compile",
            "--text",
            "package Demo { part def Vehicle; }",
            "--format",
            "json",
        ])
        .unwrap();

        assert_eq!(result.exit_code, 0);
        let json: serde_json::Value = serde_json::from_str(&result.stdout).unwrap();
        assert_eq!(json["status"], "ok");
        assert!(json["document"]["elements"].as_array().unwrap().len() > 0);
    }

    #[test]
    fn lint_file_directory_scans_model_files() {
        let root = temp_dir("mercurio-cli-lint");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.sysml"), "package A { part def Vehicle; }").unwrap();
        std::fs::write(root.join("b.kerml"), "package B { classifier Vehicle; }").unwrap();

        let result = run_args(&["lint", "--file", root.to_str().unwrap()]).unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("a.sysml"));
        assert!(result.stdout.contains("b.kerml"));
    }

    #[test]
    fn rejects_both_file_and_text() {
        let root = temp_dir("mercurio-cli-both");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("model.sysml");
        std::fs::write(&path, "package Demo { }").unwrap();

        let err = run_args(&[
            "parse",
            "--file",
            path.to_str().unwrap(),
            "--text",
            "package Demo { }",
        ])
        .unwrap_err();

        assert_eq!(err.code, 2);
    }

    #[test]
    fn rejects_missing_input() {
        let err = run_args(&["compile"]).unwrap_err();

        assert_eq!(err.code, 2);
    }

    #[test]
    fn rejects_text_language_auto() {
        let err =
            run_args(&["lint", "--text", "package Demo { }", "--language", "auto"]).unwrap_err();

        assert_eq!(err.code, 2);
    }

    #[test]
    fn diagnostic_returns_exit_code_one() {
        let result = run_args(&["parse", "--text", "package Demo { part def ; }"]).unwrap();

        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn package_build_file_writes_kpar() {
        let root = temp_dir("mercurio-cli-package");
        std::fs::create_dir_all(&root).unwrap();
        let source_path = root.join("model.sysml");
        let out_path = root.join("model.kpar");
        std::fs::write(&source_path, "package Demo { part def Vehicle; }").unwrap();

        let result = run_args(&[
            "package",
            "build",
            "--file",
            source_path.to_str().unwrap(),
            "--out",
            out_path.to_str().unwrap(),
        ])
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(out_path.exists());
        let artifact = LibraryProviderConfig::KparFile {
            path: out_path.display().to_string(),
        }
        .resolve("demo")
        .unwrap();
        assert!(
            artifact
                .document
                .elements
                .iter()
                .any(|element| element.id == "type.Demo.Vehicle")
        );
    }
}
