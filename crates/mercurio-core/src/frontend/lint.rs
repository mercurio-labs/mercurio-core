use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::frontend::ast::{
    Declaration, GenericDefinitionDecl, GenericUsageDecl, ImportDecl, PackageDecl,
    PartDefinitionDecl, PartUsageDecl, SourceSpan, SysmlModule,
};
use crate::frontend::diagnostics::Diagnostic;
use crate::frontend::kerml::{compile_kerml_module_with_context, parse_kerml};
use crate::frontend::sysml::{compile_sysml_module_with_context_report, parse_sysml_recovering};
use crate::ir::KirDocument;
use crate::language::SourceLanguage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintSeverity {
    Error,
    Warning,
    Info,
}

impl LintSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

impl fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintDiagnostic {
    pub language: SourceLanguage,
    pub severity: LintSeverity,
    pub code: String,
    pub message: String,
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LintReport {
    pub source_name: String,
    pub language: SourceLanguage,
    pub diagnostics: Vec<LintDiagnostic>,
}

impl LintReport {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == LintSeverity::Error)
    }

    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == LintSeverity::Warning)
    }
}

pub fn lint_text(
    input: &str,
    source_name: &str,
    language: SourceLanguage,
    context_modules: &[SysmlModule],
    stdlib: &KirDocument,
) -> LintReport {
    match language {
        SourceLanguage::Sysml => lint_sysml_text(input, source_name, context_modules, stdlib),
        SourceLanguage::Kerml => lint_kerml_text(input, source_name, context_modules, stdlib),
    }
}

pub fn lint_sysml_text(
    input: &str,
    source_name: &str,
    context_modules: &[SysmlModule],
    stdlib: &KirDocument,
) -> LintReport {
    let mut report = LintReport {
        source_name: source_name.to_string(),
        language: SourceLanguage::Sysml,
        diagnostics: Vec::new(),
    };

    let parse_report = match parse_sysml_recovering(input) {
        Ok(parse_report) => parse_report,
        Err(diagnostic) => {
            report.diagnostics.push(convert_diagnostic(
                SourceLanguage::Sysml,
                "parse",
                diagnostic,
            ));
            return report;
        }
    };

    for diagnostic in parse_report.diagnostics {
        report.diagnostics.push(convert_diagnostic(
            SourceLanguage::Sysml,
            "parse",
            diagnostic,
        ));
    }

    let effective_context = context_with_module(context_modules, &parse_report.module);
    let semantic_report = compile_sysml_module_with_context_report(
        &parse_report.module,
        source_name,
        &effective_context,
        stdlib,
    );
    for diagnostic in semantic_report.diagnostics {
        report.diagnostics.push(convert_diagnostic(
            SourceLanguage::Sysml,
            "semantic",
            diagnostic,
        ));
    }
    lint_module_ast(
        &parse_report.module,
        SourceLanguage::Sysml,
        &mut report.diagnostics,
    );

    report
}

pub fn lint_kerml_text(
    input: &str,
    source_name: &str,
    context_modules: &[SysmlModule],
    stdlib: &KirDocument,
) -> LintReport {
    let mut report = LintReport {
        source_name: source_name.to_string(),
        language: SourceLanguage::Kerml,
        diagnostics: Vec::new(),
    };

    let module = match parse_kerml(input) {
        Ok(module) => module,
        Err(diagnostic) => {
            report.diagnostics.push(convert_diagnostic(
                SourceLanguage::Kerml,
                "parse",
                diagnostic,
            ));
            return report;
        }
    };

    let effective_context = context_with_module(context_modules, &module);
    if let Err(diagnostic) =
        compile_kerml_module_with_context(&module, source_name, &effective_context, stdlib)
    {
        report.diagnostics.push(convert_diagnostic(
            SourceLanguage::Kerml,
            "semantic",
            diagnostic,
        ));
    }
    lint_module_ast(&module, SourceLanguage::Kerml, &mut report.diagnostics);

    report
}

fn convert_diagnostic(
    language: SourceLanguage,
    code: impl Into<String>,
    diagnostic: Diagnostic,
) -> LintDiagnostic {
    LintDiagnostic {
        language,
        severity: LintSeverity::Error,
        code: code.into(),
        message: diagnostic.message,
        span: diagnostic.span,
    }
}

fn lint_module_ast(
    module: &SysmlModule,
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    lint_duplicate_imports(&module.imports, language, diagnostics);
    lint_declarations(&module.members, language, diagnostics);
}

fn context_with_module(context_modules: &[SysmlModule], module: &SysmlModule) -> Vec<SysmlModule> {
    let mut context = context_modules.to_vec();
    if !context.iter().any(|candidate| candidate == module) {
        context.push(module.clone());
    }
    context
}

fn lint_package(
    package: &PackageDecl,
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    if package.members.is_empty() {
        diagnostics.push(warning(
            language,
            "empty-package",
            format!(
                "package `{}` has no members",
                package.name.as_colon_string()
            ),
            Some(package.span.clone()),
        ));
    }
}

fn lint_declarations(
    declarations: &[Declaration],
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    lint_duplicate_names(declarations, language, diagnostics);

    for declaration in declarations {
        lint_declaration(declaration, language, diagnostics);
    }
}

fn lint_declaration(
    declaration: &Declaration,
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    match declaration {
        Declaration::Package(package) => {
            lint_duplicate_imports(&package.imports, language, diagnostics);
            lint_package(package, language, diagnostics);
            lint_declarations(&package.members, language, diagnostics);
        }
        Declaration::Import(import) => lint_duplicate_modifiers(
            &import.modifiers,
            language,
            "import",
            &import.span,
            diagnostics,
        ),
        Declaration::PartDefinition(definition) => {
            lint_part_definition(definition, language, diagnostics)
        }
        Declaration::PartUsage(usage) => lint_part_usage(usage, language, diagnostics),
        Declaration::GenericDefinition(definition) => {
            lint_generic_definition(definition, language, diagnostics)
        }
        Declaration::GenericUsage(usage) => lint_generic_usage(usage, language, diagnostics),
        Declaration::Alias(alias) => lint_duplicate_modifiers(
            &alias.modifiers,
            language,
            "alias",
            &alias.span,
            diagnostics,
        ),
    }
}

fn lint_part_definition(
    definition: &PartDefinitionDecl,
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    lint_duplicate_modifiers(
        &definition.modifiers,
        language,
        "part definition",
        &definition.span,
        diagnostics,
    );
    lint_self_specialization(
        &definition.name,
        &definition.specializes,
        language,
        &definition.span,
        diagnostics,
    );
    lint_declarations(&definition.members, language, diagnostics);
}

fn lint_part_usage(
    usage: &PartUsageDecl,
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    lint_duplicate_modifiers(
        &usage.modifiers,
        language,
        "part usage",
        &usage.span,
        diagnostics,
    );
    lint_self_specialization(
        &usage.name,
        &usage.specializes,
        language,
        &usage.span,
        diagnostics,
    );
    lint_declarations(&usage.body_members, language, diagnostics);
}

fn lint_generic_definition(
    definition: &GenericDefinitionDecl,
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    lint_duplicate_modifiers(
        &definition.modifiers,
        language,
        "definition",
        &definition.span,
        diagnostics,
    );
    lint_self_specialization(
        &definition.name,
        &definition.specializes,
        language,
        &definition.span,
        diagnostics,
    );
    lint_declarations(&definition.members, language, diagnostics);
}

fn lint_generic_usage(
    usage: &GenericUsageDecl,
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    lint_duplicate_modifiers(
        &usage.modifiers,
        language,
        "usage",
        &usage.span,
        diagnostics,
    );
    lint_self_specialization(
        &usage.name,
        &usage.specializes,
        language,
        &usage.span,
        diagnostics,
    );
    lint_declarations(&usage.body_members, language, diagnostics);
}

fn lint_duplicate_names(
    declarations: &[Declaration],
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    let mut seen: BTreeMap<String, SourceSpan> = BTreeMap::new();

    for declaration in declarations {
        let Some((name, span)) = declaration_name_and_span(declaration) else {
            continue;
        };
        if name.is_empty() || name.starts_with('_') {
            continue;
        }

        if seen.insert(name.clone(), span.clone()).is_some() {
            diagnostics.push(warning(
                language,
                "duplicate-name",
                format!("duplicate declaration name `{name}` in the same scope"),
                Some(span),
            ));
        }
    }
}

fn lint_duplicate_imports(
    imports: &[ImportDecl],
    language: SourceLanguage,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for import in imports {
        let path = import.path.as_colon_string();
        if !seen.insert(path.clone()) {
            diagnostics.push(warning(
                language,
                "duplicate-import",
                format!("duplicate import `{path}`"),
                Some(import.span.clone()),
            ));
        }
    }
}

fn lint_duplicate_modifiers(
    modifiers: &[String],
    language: SourceLanguage,
    subject: &str,
    span: &SourceSpan,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    let mut seen = BTreeSet::new();

    for modifier in modifiers {
        if !seen.insert(modifier) {
            diagnostics.push(warning(
                language,
                "duplicate-modifier",
                format!("{subject} repeats modifier `{modifier}`"),
                Some(span.clone()),
            ));
        }
    }
}

fn lint_self_specialization(
    name: &str,
    specializations: &[crate::frontend::ast::QualifiedName],
    language: SourceLanguage,
    fallback_span: &SourceSpan,
    diagnostics: &mut Vec<LintDiagnostic>,
) {
    for specialization in specializations {
        if specialization
            .segments
            .last()
            .is_some_and(|segment| segment == name)
        {
            diagnostics.push(warning(
                language,
                "self-specialization",
                format!("`{name}` specializes itself"),
                Some(if specialization.span.start_line > 0 {
                    specialization.span.clone()
                } else {
                    fallback_span.clone()
                }),
            ));
        }
    }
}

fn declaration_name_and_span(declaration: &Declaration) -> Option<(String, SourceSpan)> {
    match declaration {
        Declaration::Package(package) => {
            Some((package.name.as_colon_string(), package.name.span.clone()))
        }
        Declaration::PartDefinition(definition) => {
            Some((definition.name.clone(), definition.span.clone()))
        }
        Declaration::PartUsage(usage) if !usage.is_implicit_name => {
            Some((usage.name.clone(), usage.span.clone()))
        }
        Declaration::GenericDefinition(definition) => {
            Some((definition.name.clone(), definition.span.clone()))
        }
        Declaration::GenericUsage(usage) if !usage.is_implicit_name => {
            Some((usage.name.clone(), usage.span.clone()))
        }
        Declaration::Alias(alias) => Some((alias.name.clone(), alias.span.clone())),
        Declaration::Import(_) | Declaration::PartUsage(_) | Declaration::GenericUsage(_) => None,
    }
}

fn warning(
    language: SourceLanguage,
    code: impl Into<String>,
    message: impl Into<String>,
    span: Option<SourceSpan>,
) -> LintDiagnostic {
    LintDiagnostic {
        language,
        severity: LintSeverity::Warning,
        code: code.into(),
        message: message.into(),
        span,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::KirDocument;
    use crate::paths::default_stdlib_path;

    fn stdlib() -> KirDocument {
        KirDocument::from_path(&default_stdlib_path()).expect("stdlib should load")
    }

    #[test]
    fn sysml_lint_reports_duplicate_names() {
        let report = lint_sysml_text(
            "package Demo { part def Vehicle; part def Vehicle; }",
            "test.sysml",
            &[],
            &stdlib(),
        );

        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "duplicate-name")
        );
    }

    #[test]
    fn kerml_lint_reports_empty_package() {
        let report = lint_kerml_text("package Empty { }", "test.kerml", &[], &stdlib());

        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "empty-package")
        );
    }

    #[test]
    fn detects_language_from_path_case_insensitively() {
        assert_eq!(
            SourceLanguage::from_path(Path::new("model.SYSML")),
            Some(SourceLanguage::Sysml)
        );
        assert_eq!(
            SourceLanguage::from_path(Path::new("model.KERML")),
            Some(SourceLanguage::Kerml)
        );
        assert_eq!(SourceLanguage::from_path(Path::new("model.txt")), None);
    }
}
