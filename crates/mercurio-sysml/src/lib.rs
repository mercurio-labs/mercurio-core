pub use mercurio_core::frontend::ast::{
    Declaration, GenericDefinitionDecl, GenericUsageDecl, ParsedModule, QualifiedName, SourceSpan,
    SysmlModule,
};
pub use mercurio_core::frontend::diagnostics::Diagnostic;
pub use mercurio_core::language::sysml::SysmlLanguageModule;
pub use mercurio_core::language::sysml::parser::{
    ParseReport, SemanticCompileReport, SemanticCompileStatus, SysmlError, compile_sysml_module,
    compile_sysml_module_with_context, compile_sysml_module_with_context_report,
    compile_sysml_text, compile_sysml_text_with_context, compile_sysml_text_with_context_report,
    compile_text, compile_text_with_context, load_sysml_document, load_sysml_document_with_stdlib,
    parse, parse_sysml, parse_sysml_recovering,
};
pub use mercurio_core::{BaselineLibrary, KirDocument, SourceLanguage, language_module};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_parses_minimal_sysml() {
        let module = parse("package Demo { part def Vehicle; }").unwrap();

        assert!(module.package.is_some());
    }
}
