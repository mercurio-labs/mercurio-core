use std::path::Path;

use crate::frontend::ast::SysmlModule;
use crate::frontend::diagnostics::Diagnostic;
use crate::frontend::kerml::{
    compile_kerml_module_with_resolver_context, compile_kerml_text_with_context, parse_kerml,
};
use crate::frontend::resolver::ResolverContext;
use crate::frontend::sysml::{
    compile_sysml_module_with_resolver_context, compile_sysml_text_with_context, parse_sysml,
};
use crate::frontend::transpile::MappingBundle;
use crate::logging::compile_timer_start;
use crate::ir::{KirDocument, KirError};
use crate::logging::log_compile_timed_event;

#[derive(Debug, Clone)]
pub struct SourceDocument {
    pub path: String,
    pub content: String,
    pub module: Option<SysmlModule>,
}

pub struct SourceCompileContext {
    pub context_modules: Vec<SysmlModule>,
    pub mappings: &'static MappingBundle,
    pub resolver_context: ResolverContext,
}

impl SourceCompileContext {
    pub fn from_source_documents(
        source_documents: &[SourceDocument],
        library_context: &KirDocument,
    ) -> Result<Self, Diagnostic> {
        let context_modules = collect_context_modules(source_documents);
        let mappings = MappingBundle::load()?;
        let resolver_context =
            ResolverContext::from_modules(&context_modules, library_context, mappings)?;

        Ok(Self {
            context_modules,
            mappings,
            resolver_context,
        })
    }
}

impl SourceDocument {
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        let path = path.into();
        let content = content.into();
        let parse_start = compile_timer_start();
        let module_result = parse_source_module(&path, &content);
        log_compile_timed_event(
            "source_set.parse_source_module",
            parse_start,
            if module_result.is_ok() { "ok" } else { "error" },
            format!("path={} bytes={}", path, content.len()),
        );
        let module = module_result.ok();

        Self {
            path,
            content,
            module,
        }
    }
}

pub fn parse_source_module(path: &str, content: &str) -> Result<SysmlModule, Diagnostic> {
    if is_kerml_path(path) {
        parse_kerml(content)
    } else {
        parse_sysml(content)
    }
}

pub fn compile_source_text_with_context(
    path: &str,
    content: &str,
    context_modules: &[SysmlModule],
    library_context: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    if is_kerml_path(path) {
        compile_kerml_text_with_context(content, path, context_modules, library_context)
    } else {
        compile_sysml_text_with_context(content, path, context_modules, library_context)
    }
}

pub fn compile_source_document_with_context(
    file: &SourceDocument,
    compile_context: &SourceCompileContext,
    library_context: &KirDocument,
) -> Result<KirDocument, Diagnostic> {
    match file.module.as_ref() {
        Some(module) if is_kerml_path(&file.path) => compile_kerml_module_with_resolver_context(
            module,
            &file.path,
            &compile_context.resolver_context,
            compile_context.mappings,
        ),
        Some(module) => compile_sysml_module_with_resolver_context(
            module,
            &file.path,
            &compile_context.resolver_context,
            compile_context.mappings,
        ),
        None => compile_source_text_with_context(
            &file.path,
            &file.content,
            &compile_context.context_modules,
            library_context,
        ),
    }
}

pub fn collect_context_modules(source_documents: &[SourceDocument]) -> Vec<SysmlModule> {
    source_documents
        .iter()
        .filter_map(|file| file.module.clone())
        .collect()
}

pub fn compile_source_documents(
    source_documents: Vec<SourceDocument>,
    library_context: &KirDocument,
) -> Result<KirDocument, KirError> {
    let compile_context =
        SourceCompileContext::from_source_documents(&source_documents, library_context)
            .map_err(|err| KirError::Sysml(err.to_string()))?;
    let documents = source_documents
        .iter()
        .map(|file| {
            compile_source_document_with_context(file, &compile_context, library_context)
                .map_err(|err| KirError::Sysml(err.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    KirDocument::merge(documents)
}

fn is_kerml_path(path: &str) -> bool {
    Path::new(path).extension().and_then(|value| value.to_str()) == Some("kerml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_mixed_kerml_and_sysml_source_documents() {
        let document = compile_source_documents(
            vec![
                SourceDocument::new(
                    "types.kerml",
                    "package Domain {
                      classifier Engine;
                    }",
                ),
                SourceDocument::new(
                    "vehicle.sysml",
                    "package VehicleModel {
                      import Domain::*;
                      part def Vehicle {
                        part engine : Engine;
                      }
                    }",
                ),
            ],
            &KirDocument {
                metadata: Default::default(),
                elements: vec![
                    crate::ir::KirElement {
                        id: "Parts::Part".to_string(),
                        kind: "SysML::PartDefinition".to_string(),
                        layer: 1,
                        properties: Default::default(),
                    },
                    crate::ir::KirElement {
                        id: "Items::Item::subparts".to_string(),
                        kind: "SysML::PartUsage".to_string(),
                        layer: 1,
                        properties: Default::default(),
                    },
                ],
            },
        )
        .unwrap();

        assert!(
            document
                .elements
                .iter()
                .any(|element| element.id == "type.Domain.Engine")
        );
        assert!(document.elements.iter().any(|element| {
            element.id == "feature.VehicleModel.Vehicle.engine"
                && element.properties.get("type") == Some(&serde_json::json!("type.Domain.Engine"))
        }));
    }
}
