use crate::frontend::ast::ParsedModule;
use crate::frontend::diagnostics::Diagnostic;
use crate::ir::KirDocument;

pub use crate::frontend::sysml::{
    ParseReport, SemanticCompileReport, SemanticCompileStatus, SysmlError, compile_sysml_module,
    compile_sysml_module_with_context, compile_sysml_module_with_context_report,
    compile_sysml_text, compile_sysml_text_with_context, compile_sysml_text_with_context_report,
    load_sysml_document, load_sysml_document_with_stdlib, parse_sysml, parse_sysml_recovering,
};

pub fn parse(input: &str) -> Result<ParsedModule, Diagnostic> {
    parse_sysml(input)
}

pub fn compile_text(
    input: &str,
    source_name: &str,
    library_context: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    compile_sysml_text(input, source_name, library_context)
}

pub fn compile_text_with_context(
    input: &str,
    source_name: &str,
    context_modules: &[ParsedModule],
    library_context: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    compile_sysml_text_with_context(input, source_name, context_modules, library_context)
}
