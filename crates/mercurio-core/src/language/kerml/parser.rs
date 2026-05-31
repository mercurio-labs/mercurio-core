use crate::frontend::ast::ParsedModule;
use crate::frontend::diagnostics::Diagnostic;
use crate::ir::KirDocument;

pub use crate::frontend::kerml::{
    KermlError, compile_kerml_module, compile_kerml_module_with_context, compile_kerml_text,
    compile_kerml_text_with_context, compile_kerml_text_with_empty_context, load_kerml_document,
    load_kerml_document_with_stdlib, parse_kerml,
};

pub fn parse(input: &str) -> Result<ParsedModule, Diagnostic> {
    parse_kerml(input)
}

pub fn compile_text(
    input: &str,
    source_name: &str,
    library_context: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    compile_kerml_text(input, source_name, library_context)
}

pub fn compile_text_with_context(
    input: &str,
    source_name: &str,
    context_modules: &[ParsedModule],
    library_context: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    compile_kerml_text_with_context(input, source_name, context_modules, library_context)
}
