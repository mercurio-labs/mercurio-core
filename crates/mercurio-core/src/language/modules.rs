use crate::frontend::ast::ParsedModule;
use crate::frontend::diagnostics::Diagnostic;
use crate::frontend::transpile::MappingBundle;
use crate::ir::KirDocument;
use crate::language::concepts::SourceLanguage;
use crate::language::kerml::parser as kerml_parser;
use crate::language::library_context::BaselineLibrary;
use crate::language::sysml::parser as sysml_parser;

pub trait LanguageModule: Sync {
    fn language(&self) -> SourceLanguage;

    fn id(&self) -> &'static str {
        self.language().as_str()
    }

    fn extensions(&self) -> &'static [&'static str] {
        self.language().extensions()
    }

    fn parse(&self, input: &str) -> Result<ParsedModule, Diagnostic>;

    fn compile_text_with_context(
        &self,
        input: &str,
        source_name: &str,
        context_modules: &[ParsedModule],
        library_context: &KirDocument,
    ) -> Result<KirDocument, Diagnostic>;

    fn mappings(&self) -> Result<&'static MappingBundle, Diagnostic> {
        MappingBundle::load_for_language(self.language())
    }

    fn default_baseline(&self) -> BaselineLibrary;
}

#[derive(Debug)]
pub struct KermlLanguageModule;

impl LanguageModule for KermlLanguageModule {
    fn language(&self) -> SourceLanguage {
        SourceLanguage::Kerml
    }

    fn parse(&self, input: &str) -> Result<ParsedModule, Diagnostic> {
        kerml_parser::parse(input)
    }

    fn compile_text_with_context(
        &self,
        input: &str,
        source_name: &str,
        context_modules: &[ParsedModule],
        library_context: &KirDocument,
    ) -> Result<KirDocument, Diagnostic> {
        kerml_parser::compile_text_with_context(
            input,
            source_name,
            context_modules,
            library_context,
        )
    }

    fn default_baseline(&self) -> BaselineLibrary {
        BaselineLibrary::Kernel
    }
}

#[derive(Debug)]
pub struct SysmlLanguageModule;

impl LanguageModule for SysmlLanguageModule {
    fn language(&self) -> SourceLanguage {
        SourceLanguage::Sysml
    }

    fn parse(&self, input: &str) -> Result<ParsedModule, Diagnostic> {
        sysml_parser::parse(input)
    }

    fn compile_text_with_context(
        &self,
        input: &str,
        source_name: &str,
        context_modules: &[ParsedModule],
        library_context: &KirDocument,
    ) -> Result<KirDocument, Diagnostic> {
        sysml_parser::compile_text_with_context(
            input,
            source_name,
            context_modules,
            library_context,
        )
    }

    fn default_baseline(&self) -> BaselineLibrary {
        BaselineLibrary::Sysml
    }
}

static KERML_LANGUAGE_MODULE: KermlLanguageModule = KermlLanguageModule;
static SYSML_LANGUAGE_MODULE: SysmlLanguageModule = SysmlLanguageModule;

pub fn language_module(language: SourceLanguage) -> &'static dyn LanguageModule {
    match language {
        SourceLanguage::Kerml => &KERML_LANGUAGE_MODULE,
        SourceLanguage::Sysml => &SYSML_LANGUAGE_MODULE,
    }
}

pub fn language_module_for_path(path: &std::path::Path) -> Option<&'static dyn LanguageModule> {
    SourceLanguage::from_path(path).map(language_module)
}

pub fn language_modules() -> [&'static dyn LanguageModule; 2] {
    [&KERML_LANGUAGE_MODULE, &SYSML_LANGUAGE_MODULE]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_language_modules() {
        assert_eq!(language_module(SourceLanguage::Kerml).id(), "kerml");
        assert_eq!(language_module(SourceLanguage::Sysml).id(), "sysml");
        assert_eq!(
            language_module_for_path(std::path::Path::new("model.kerml"))
                .unwrap()
                .id(),
            "kerml"
        );
        assert_eq!(language_modules().len(), 2);
    }

    #[test]
    fn kerml_module_defaults_to_kernel_baseline() {
        assert!(matches!(
            language_module(SourceLanguage::Kerml).default_baseline(),
            BaselineLibrary::Kernel
        ));
        assert!(matches!(
            language_module(SourceLanguage::Sysml).default_baseline(),
            BaselineLibrary::Sysml
        ));
    }
}
