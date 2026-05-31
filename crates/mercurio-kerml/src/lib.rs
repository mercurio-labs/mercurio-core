//! KerML language facade.
//!
//! This crate is the public KerML-facing boundary while the parser/compiler
//! implementation is still hosted in `mercurio-core`. Keep this surface narrow:
//! parsing KerML, compiling KerML to KIR, and loading the KerML Kernel baseline.

pub use mercurio_core::frontend::ast::{ParsedModule, QualifiedName, SourceSpan};
pub use mercurio_core::frontend::diagnostics::Diagnostic;
pub use mercurio_core::ir::KirError;
pub use mercurio_core::language::kerml::KermlLanguageModule;
pub use mercurio_core::language::kerml::parser::{
    KermlError, compile_kerml_module, compile_kerml_module_with_context, compile_kerml_text,
    compile_kerml_text_with_context, compile_kerml_text_with_empty_context, compile_text,
    compile_text_with_context, load_kerml_document, load_kerml_document_with_stdlib, parse,
    parse_kerml,
};
pub use mercurio_core::{BaselineLibrary, KirDocument};

pub fn default_kernel_library_path() -> std::path::PathBuf {
    mercurio_core::default_kernel_library_path()
}

pub fn load_kernel_baseline() -> Result<KirDocument, KirError> {
    BaselineLibrary::Kernel.load()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facade_parses_minimal_kerml() {
        let module = parse("package Demo { classifier Vehicle; }").unwrap();

        assert!(module.package.is_some());
    }
}
