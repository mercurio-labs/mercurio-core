pub use mercurio_core::frontend::ast::{
    Declaration, GenericDefinitionDecl, GenericUsageDecl, ParsedModule, QualifiedName, SourceSpan,
};
pub use mercurio_core::frontend::diagnostics::Diagnostic;
pub use mercurio_core::language::kerml::KermlLanguageModule;
pub use mercurio_core::language::kerml::parser::{
    KermlError, compile_kerml_module, compile_kerml_module_with_context, compile_kerml_text,
    compile_kerml_text_with_context, compile_kerml_text_with_empty_context, compile_text,
    compile_text_with_context, load_kerml_document, load_kerml_document_with_stdlib, parse,
    parse_kerml,
};
pub use mercurio_core::{BaselineLibrary, KirDocument, SourceLanguage, language_module};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_parses_minimal_kerml() {
        let module = parse("package Demo { classifier Vehicle; }").unwrap();

        assert!(module.package.is_some());
    }
}
