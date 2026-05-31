//! SysML language facade.
//!
//! This crate is the public SysML-facing boundary while the parser/compiler
//! implementation is still hosted in `mercurio-core`. Keep this surface focused
//! on SysML parsing, recovery/reporting, compilation to KIR, and the SysML
//! baseline library.

pub use mercurio_core::frontend::ast::{ParsedModule, QualifiedName, SourceSpan, SysmlModule};
pub use mercurio_core::frontend::diagnostics::Diagnostic;
pub use mercurio_core::ir::KirError;
pub use mercurio_core::language::sysml::SysmlLanguageModule;
pub use mercurio_core::language::sysml::parser::{
    ParseReport, SemanticCompileReport, SemanticCompileStatus, SysmlError, compile_sysml_module,
    compile_sysml_module_with_context, compile_sysml_module_with_context_report,
    compile_sysml_text, compile_sysml_text_with_context, compile_sysml_text_with_context_report,
    compile_text, compile_text_with_context, load_sysml_document, load_sysml_document_with_stdlib,
    parse, parse_sysml, parse_sysml_recovering,
};
pub use mercurio_core::{BaselineLibrary, KirDocument};

pub fn default_sysml_library_path() -> std::path::PathBuf {
    mercurio_core::default_sysml_library_path()
}

pub fn load_sysml_baseline() -> Result<KirDocument, KirError> {
    BaselineLibrary::Sysml.load()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_parses_minimal_sysml() {
        let module = parse("package Demo { part def Vehicle; }").unwrap();

        assert!(module.package.is_some());
    }
}
