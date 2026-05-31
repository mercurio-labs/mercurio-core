pub use mercurio_kir::{
    KIR_SCHEMA_VERSION, KirDocument, KirElement, KirError, KirFieldKind, KirFieldRegistry,
    KirFieldSpec, KirValidationDiagnostic,
};

use std::path::Path;

use crate::language::{SourceLanguage, language_module};
use crate::paths::default_sysml_library_path;

pub fn load_model_stack(model_path: &Path) -> Result<KirDocument, KirError> {
    let sysml_library_path = default_sysml_library_path();
    if paths_equivalent(model_path, &sysml_library_path) || is_kir_json(model_path) {
        return KirDocument::from_path(model_path);
    }

    let language = SourceLanguage::from_path(model_path).unwrap_or(SourceLanguage::Sysml);
    let library_context = language_module(language).default_baseline().load()?;
    let user_document = load_document_for_language(model_path, language, &library_context)?;

    KirDocument::merge([library_context, user_document])
}

pub fn load_model_stack_with_language(
    model_path: &Path,
    language: SourceLanguage,
) -> Result<KirDocument, KirError> {
    if is_kir_json(model_path) {
        return KirDocument::from_path(model_path);
    }

    let library_context = language_module(language).default_baseline().load()?;
    let user_document = load_document_for_language(model_path, language, &library_context)?;

    KirDocument::merge([library_context, user_document])
}

fn load_document_for_language(
    model_path: &Path,
    language: SourceLanguage,
    library_context: &KirDocument,
) -> Result<KirDocument, KirError> {
    Ok(
        match SourceLanguage::from_path(model_path).map(|_| language) {
            Some(SourceLanguage::Sysml) => {
                crate::language::sysml::parser::load_sysml_document_with_stdlib(
                    model_path,
                    library_context,
                )
                .map_err(|err| KirError::Frontend(err.to_string()))?
            }
            Some(SourceLanguage::Kerml) => {
                crate::language::kerml::parser::load_kerml_document_with_stdlib(
                    model_path,
                    library_context,
                )
                .map_err(|err| KirError::Frontend(err.to_string()))?
            }
            None => KirDocument::from_path(model_path)?,
        },
    )
}

fn paths_equivalent(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }

    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn is_kir_json(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|file_name| file_name.ends_with(".kir.json"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn load_model_stack_accepts_kerml_sources() {
        let document = super::load_model_stack(&crate::paths::repo_path(
            "test_files/kerml/minimal_classifier.kerml",
        ))
        .unwrap();

        assert!(
            document
                .elements
                .iter()
                .any(|element| element.id == "type.Demo.Vehicle")
        );
        assert!(
            document
                .elements
                .iter()
                .any(|element| element.id == "feature.Demo.Vehicle.engine")
        );
    }
}
