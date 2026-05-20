use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::paths::default_stdlib_path;

pub const KIR_SCHEMA_VERSION: &str = "0.2";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KirDocument {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
    pub elements: Vec<KirElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KirElement {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub layer: u8,
    #[serde(default)]
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug)]
pub enum KirError {
    Io(std::io::Error),
    Json(serde_json::Error),
    DuplicateId(String),
    Validation(Vec<KirValidationDiagnostic>),
    Frontend(String),
    Sysml(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KirValidationDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub element_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KirFieldKind {
    Scalar,
    Reference,
    ReferenceList,
    Expression,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KirFieldSpec {
    pub kind: KirFieldKind,
}

#[derive(Debug, Clone, Copy)]
pub struct KirFieldRegistry;

impl KirFieldRegistry {
    pub fn standard() -> Self {
        Self
    }

    pub fn field(&self, name: &str) -> Option<KirFieldSpec> {
        let kind = match name {
            "declared_name"
            | "declared_short_name"
            | "name"
            | "documentation"
            | "direction"
            | "multiplicity"
            | "declared_multiplicity"
            | "operator"
            | "operator_expression"
            | "text"
            | "requirement_id"
            | "body"
            | "language"
            | "source_file"
            | "source_language" => KirFieldKind::Scalar,

            "is_abstract" | "is_conjugated" | "is_derived" | "is_end" | "is_variable"
            | "is_readonly" | "is_ordered" | "is_unique" => KirFieldKind::Scalar,

            "owner"
            | "owning_type"
            | "type"
            | "metatype"
            | "source"
            | "target"
            | "payload"
            | "result"
            | "original_definition"
            | "conjugated"
            | "opposite"
            | "target_ref" => KirFieldKind::Reference,

            "members"
            | "features"
            | "owned_features"
            | "feature_typings"
            | "specializes"
            | "subsets"
            | "subsetted_features"
            | "redefines"
            | "redefined_features"
            | "specialized_features"
            | "imports"
            | "relationships"
            | "sources"
            | "targets"
            | "parts"
            | "items"
            | "owned_feature"
            | "verify"
            | "satisfy"
            | "related"
            | "parameters"
            | "arguments"
            | "successions"
            | "dependencies" => KirFieldKind::ReferenceList,

            "expression" => KirFieldKind::Scalar,
            "expression_ir" => KirFieldKind::Expression,
            "metadata" | "source_span" | "doc" => KirFieldKind::Metadata,
            "element_id" => KirFieldKind::Scalar,
            _ => return None,
        };

        Some(KirFieldSpec { kind })
    }

    pub fn reference_ids<'a>(&self, field: &str, value: &'a Value) -> Vec<&'a str> {
        match self.field(field).map(|spec| spec.kind) {
            Some(KirFieldKind::Reference) => reference_list_items(value),
            Some(KirFieldKind::ReferenceList) => reference_list_items(value),
            _ => Vec::new(),
        }
    }

    fn validate_value(
        &self,
        field: &str,
        value: &Value,
        element_id: &str,
    ) -> Option<KirValidationDiagnostic> {
        let spec = self.field(field)?;
        let valid = match spec.kind {
            KirFieldKind::Scalar => {
                value.is_string() || value.is_number() || value.is_boolean() || value.is_null()
            }
            KirFieldKind::Reference => {
                value.is_string()
                    || value
                        .as_array()
                        .is_some_and(|items| items.iter().all(Value::is_string))
                    || value.is_null()
            }
            KirFieldKind::ReferenceList => {
                value.is_string()
                    || value
                        .as_array()
                        .is_some_and(|items| items.iter().all(Value::is_string))
                    || value.is_null()
            }
            KirFieldKind::Expression | KirFieldKind::Metadata => {
                value.is_object() || value.is_array() || value.is_null()
            }
        };

        (!valid).then(|| KirValidationDiagnostic {
            code: "kir.element.property.shape",
            message: format!(
                "KIR element {element_id} property `{field}` has invalid shape for {:?}",
                spec.kind
            ),
            element_id: Some(element_id.to_string()),
        })
    }

    fn unknown_field_diagnostic(
        &self,
        field: &str,
        element_id: &str,
    ) -> Option<KirValidationDiagnostic> {
        (self.field(field).is_none() && !field.starts_with("x_")).then(|| {
            KirValidationDiagnostic {
                code: "kir.element.property.unknown",
                message: format!(
                    "KIR element {element_id} property `{field}` is not registered in the field contract"
                ),
                element_id: Some(element_id.to_string()),
            }
        })
    }
}

fn reference_list_items(value: &Value) -> Vec<&str> {
    match value {
        Value::String(value) => vec![value.as_str()],
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

impl fmt::Display for KirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read KIR document: {err}"),
            Self::Json(err) => write!(f, "failed to parse KIR document: {err}"),
            Self::DuplicateId(id) => write!(f, "duplicate KIR element id: {id}"),
            Self::Validation(diagnostics) => {
                write!(f, "invalid KIR document")?;
                if let Some(first) = diagnostics.first() {
                    write!(f, ": {}", first.message)?;
                }
                Ok(())
            }
            Self::Frontend(err) => write!(f, "{err}"),
            Self::Sysml(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for KirError {}

impl From<std::io::Error> for KirError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for KirError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl KirDocument {
    pub fn from_str(input: &str) -> Result<Self, KirError> {
        let document: Self = serde_json::from_str(input)?;
        document.validate()?;
        Ok(document)
    }

    pub fn from_path(path: &Path) -> Result<Self, KirError> {
        let input = std::fs::read_to_string(path)?;
        Self::from_str(&input)
    }

    pub fn validate(&self) -> Result<(), KirError> {
        self.validate_with_field_contract(false)
    }

    pub fn validate_strict_field_contract(&self) -> Result<(), KirError> {
        self.validate_with_field_contract(true)
    }

    fn validate_with_field_contract(&self, reject_unknown_fields: bool) -> Result<(), KirError> {
        let mut diagnostics = Vec::new();
        let mut seen = BTreeSet::new();
        let field_registry = KirFieldRegistry::standard();

        for element in &self.elements {
            let trimmed_id = element.id.trim();
            if trimmed_id.is_empty() {
                diagnostics.push(KirValidationDiagnostic {
                    code: "kir.element.id.empty",
                    message: "KIR element id must not be empty".to_string(),
                    element_id: None,
                });
            } else if trimmed_id != element.id {
                diagnostics.push(KirValidationDiagnostic {
                    code: "kir.element.id.invalid",
                    message: format!(
                        "KIR element id must not contain leading or trailing whitespace: {}",
                        element.id
                    ),
                    element_id: Some(element.id.clone()),
                });
            }

            if element.kind.trim().is_empty() {
                diagnostics.push(KirValidationDiagnostic {
                    code: "kir.element.kind.empty",
                    message: format!("KIR element {} must declare a semantic kind", element.id),
                    element_id: Some(element.id.clone()),
                });
            }

            if element.layer > 2 {
                diagnostics.push(KirValidationDiagnostic {
                    code: "kir.element.layer.unsupported",
                    message: format!(
                        "KIR element {} uses unsupported layer {}",
                        element.id, element.layer
                    ),
                    element_id: Some(element.id.clone()),
                });
            }

            if !element.id.is_empty() && !seen.insert(element.id.clone()) {
                diagnostics.push(KirValidationDiagnostic {
                    code: "kir.element.id.duplicate",
                    message: format!("duplicate KIR element id: {}", element.id),
                    element_id: Some(element.id.clone()),
                });
            }

            for (property, value) in &element.properties {
                if reject_unknown_fields {
                    if let Some(diagnostic) =
                        field_registry.unknown_field_diagnostic(property, &element.id)
                    {
                        diagnostics.push(diagnostic);
                        continue;
                    }
                }
                if let Some(diagnostic) =
                    field_registry.validate_value(property, value, &element.id)
                {
                    diagnostics.push(diagnostic);
                }
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(KirError::Validation(diagnostics))
        }
    }

    pub fn merge<I>(documents: I) -> Result<Self, KirError>
    where
        I: IntoIterator<Item = KirDocument>,
    {
        let mut seen = BTreeSet::new();
        let mut elements = Vec::new();
        let mut source_metadata = Vec::new();

        for document in documents {
            if !document.metadata.is_empty() {
                source_metadata.push(Value::Object(document.metadata.into_iter().collect()));
            }
            for element in document.elements {
                if !seen.insert(element.id.clone()) {
                    return Err(KirError::DuplicateId(element.id));
                }
                elements.push(element);
            }
        }

        elements.sort_by(|left, right| left.id.cmp(&right.id));
        let mut metadata = BTreeMap::from([(
            "kir_schema_version".to_string(),
            Value::String(KIR_SCHEMA_VERSION.to_string()),
        )]);
        if !source_metadata.is_empty() {
            metadata.insert("merged_sources".to_string(), Value::Array(source_metadata));
        }

        let merged = Self { metadata, elements };
        merged.validate()?;
        Ok(merged)
    }

    pub fn write_pretty_to_path(&self, path: &Path) -> Result<(), KirError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

pub fn load_model_stack(model_path: &Path) -> Result<KirDocument, KirError> {
    let stdlib_path = default_stdlib_path();
    if paths_equivalent(model_path, &stdlib_path) {
        return KirDocument::from_path(model_path);
    }

    let stdlib_document = KirDocument::from_path(&stdlib_path)?;

    let user_document = match model_path.extension().and_then(|value| value.to_str()) {
        Some("sysml") => {
            crate::frontend::sysml::load_sysml_document_with_stdlib(model_path, &stdlib_document)
                .map_err(|err| KirError::Frontend(err.to_string()))?
        }
        Some("kerml") => {
            crate::frontend::kerml::load_kerml_document_with_stdlib(model_path, &stdlib_document)
                .map_err(|err| KirError::Frontend(err.to_string()))?
        }
        _ => KirDocument::from_path(model_path)?,
    };

    KirDocument::merge([stdlib_document, user_document])
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

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{KirDocument, KirElement, KirError};

    #[test]
    fn merge_rejects_duplicate_ids() {
        let left = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "dup".to_string(),
                kind: "kerml.Type".to_string(),
                layer: 0,
                properties: Default::default(),
            }],
        };
        let right = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "dup".to_string(),
                kind: "sysml.PartDefinition".to_string(),
                layer: 1,
                properties: Default::default(),
            }],
        };

        let error = KirDocument::merge([left, right]).unwrap_err();
        assert!(matches!(error, KirError::DuplicateId(id) if id == "dup"));
    }

    #[test]
    fn validate_rejects_empty_element_id() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: String::new(),
                kind: "KerML::Core::Type".to_string(),
                layer: 0,
                properties: Default::default(),
            }],
        };

        let error = document.validate().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.id.empty")
        );
    }

    #[test]
    fn validate_rejects_unsupported_layer() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Bad.Layer".to_string(),
                kind: "KerML::Core::Type".to_string(),
                layer: 3,
                properties: Default::default(),
            }],
        };

        let error = document.validate().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.layer.unsupported")
        );
    }

    #[test]
    fn validate_rejects_registered_reference_field_with_wrong_shape() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "feature.Demo.Vehicle.engine".to_string(),
                kind: "SysML::PartUsage".to_string(),
                layer: 2,
                properties: [("owner".to_string(), Value::Bool(true))]
                    .into_iter()
                    .collect(),
            }],
        };

        let error = document.validate().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.property.shape")
        );
    }

    #[test]
    fn validate_accepts_registered_reference_list_as_scalar_or_array() {
        let scalar = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "SysML::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: [(
                    "specializes".to_string(),
                    Value::String("Parts::Part".to_string()),
                )]
                .into_iter()
                .collect(),
            }],
        };
        let array = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "SysML::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: [(
                    "specializes".to_string(),
                    Value::Array(vec![Value::String("Parts::Part".to_string())]),
                )]
                .into_iter()
                .collect(),
            }],
        };

        scalar.validate().unwrap();
        array.validate().unwrap();
    }

    #[test]
    fn strict_field_contract_rejects_unknown_non_extension_property() {
        let document = KirDocument {
            metadata: Default::default(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "SysML::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: [
                    ("custom".to_string(), Value::String("value".to_string())),
                    (
                        "x_vendor".to_string(),
                        Value::String("preserved".to_string()),
                    ),
                ]
                .into_iter()
                .collect(),
            }],
        };

        document.validate().unwrap();
        let error = document.validate_strict_field_contract().unwrap_err();
        assert!(
            matches!(error, KirError::Validation(diagnostics) if diagnostics[0].code == "kir.element.property.unknown")
        );
    }

    #[test]
    fn merge_preserves_document_metadata_as_sources() {
        let left = KirDocument {
            metadata: [("source".to_string(), Value::String("stdlib".to_string()))]
                .into_iter()
                .collect(),
            elements: vec![],
        };
        let right = KirDocument {
            metadata: [("source".to_string(), Value::String("user".to_string()))]
                .into_iter()
                .collect(),
            elements: vec![],
        };

        let merged = KirDocument::merge([left, right]).unwrap();
        assert_eq!(
            merged.metadata["kir_schema_version"],
            Value::String(super::KIR_SCHEMA_VERSION.to_string())
        );
        assert_eq!(
            merged.metadata["merged_sources"].as_array().unwrap().len(),
            2
        );
    }

    #[test]
    fn load_model_stack_accepts_kerml_sources() {
        let document = super::load_model_stack(&crate::paths::repo_path(
            "fixtures/kerml/minimal_classifier.kerml",
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
