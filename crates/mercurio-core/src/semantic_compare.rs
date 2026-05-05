use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Serialize;
use serde_json::{Value, json};

use crate::graph::{Graph, GraphError, NodeId};
use crate::ir::KirDocument;
use crate::metamodel::{
    AttributeRow, ElementAttributeQuery, MetamodelAttributeRegistry, MetatypeQueryOverride,
    collect_specialization_ancestors, query_element_attributes,
};

#[derive(Debug, Clone, Copy)]
pub enum SnapshotMode {
    Mercurio,
    Pilot,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticSnapshot {
    pub focus_source_file: String,
    pub mode: String,
    pub elements: Vec<SemanticSnapshotElement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticSnapshotElement {
    pub match_key: String,
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
    pub declared_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_span: Option<SemanticSourceSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metatype: Option<String>,
    pub metatype_specialization_chain: Vec<String>,
    pub declared_attributes: BTreeMap<String, SemanticSnapshotAttribute>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SemanticSnapshotAttribute {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_by: Option<String>,
    pub has_direct_value: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_value: Option<Value>,
    pub has_effective_value: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_value: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticSourceSpan {
    pub start_line: Option<u64>,
    pub end_line: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticComparisonReport {
    pub focus_source_file: String,
    pub mercurio_count: usize,
    pub pilot_count: usize,
    pub exact_match_count: usize,
    pub mercurio_only: Vec<SemanticSnapshotElement>,
    pub pilot_only: Vec<SemanticSnapshotElement>,
    pub mismatches: Vec<SemanticElementMismatch>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticElementMismatch {
    pub match_key: String,
    pub mercurio_id: String,
    pub pilot_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metatype: Option<SemanticValueMismatch<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metatype_specialization_chain: Option<SemanticValueMismatch<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_attributes:
        Option<SemanticValueMismatch<BTreeMap<String, SemanticSnapshotAttribute>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticValueMismatch<T> {
    pub mercurio: T,
    pub pilot: T,
}

#[derive(Debug)]
pub enum SemanticCompareError {
    Graph(GraphError),
    DuplicateMatchKey(String),
}

impl fmt::Display for SemanticCompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Graph(err) => write!(f, "{err}"),
            Self::DuplicateMatchKey(key) => {
                write!(f, "duplicate semantic snapshot match key: {key}")
            }
        }
    }
}

impl std::error::Error for SemanticCompareError {}

impl From<GraphError> for SemanticCompareError {
    fn from(value: GraphError) -> Self {
        Self::Graph(value)
    }
}

pub fn build_semantic_snapshot(
    document: KirDocument,
    focus_source_file: &str,
    mode: SnapshotMode,
) -> Result<SemanticSnapshot, SemanticCompareError> {
    let graph = Graph::from_document(document)?;
    let registry = MetamodelAttributeRegistry::build(&graph);
    build_semantic_snapshot_from_parts(&graph, &registry, focus_source_file, mode)
}

pub fn build_semantic_snapshot_with_registry(
    document: KirDocument,
    focus_source_file: &str,
    mode: SnapshotMode,
    registry: &MetamodelAttributeRegistry,
) -> Result<SemanticSnapshot, SemanticCompareError> {
    let graph = Graph::from_document(document)?;
    build_semantic_snapshot_from_parts(&graph, registry, focus_source_file, mode)
}

fn build_semantic_snapshot_from_parts(
    graph: &Graph,
    registry: &MetamodelAttributeRegistry,
    focus_source_file: &str,
    mode: SnapshotMode,
) -> Result<SemanticSnapshot, SemanticCompareError> {
    let mut elements = graph
        .elements()
        .iter()
        .filter_map(|element| {
            snapshot_element(graph, registry, element.id, focus_source_file, mode)
        })
        .collect::<Vec<_>>();
    elements.sort_by(|left, right| left.match_key.cmp(&right.match_key));
    ensure_unique_match_keys(&elements)?;

    Ok(SemanticSnapshot {
        focus_source_file: focus_source_file.to_string(),
        mode: match mode {
            SnapshotMode::Mercurio => "mercurio".to_string(),
            SnapshotMode::Pilot => "pilot".to_string(),
        },
        elements,
    })
}

pub fn compare_snapshots(
    mercurio: SemanticSnapshot,
    pilot: SemanticSnapshot,
) -> Result<SemanticComparisonReport, SemanticCompareError> {
    let mercurio_by_key = snapshot_index(&mercurio.elements)?;
    let pilot_by_key = snapshot_index(&pilot.elements)?;

    let all_keys = mercurio_by_key
        .keys()
        .chain(pilot_by_key.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut exact_match_count = 0;
    let mut mercurio_only = Vec::new();
    let mut pilot_only = Vec::new();
    let mut mismatches = Vec::new();

    for key in all_keys.iter() {
        match (mercurio_by_key.get(key), pilot_by_key.get(key)) {
            (Some(mercurio_element), Some(pilot_element)) => {
                let mismatch = compare_element_pair(mercurio_element, pilot_element);
                if let Some(mismatch) = mismatch {
                    mismatches.push(mismatch);
                } else {
                    exact_match_count += 1;
                }
            }
            (Some(mercurio_element), None) => mercurio_only.push((*mercurio_element).clone()),
            (None, Some(pilot_element)) => pilot_only.push((*pilot_element).clone()),
            (None, None) => {}
        }
    }

    Ok(SemanticComparisonReport {
        focus_source_file: mercurio.focus_source_file,
        mercurio_count: mercurio.elements.len(),
        pilot_count: pilot.elements.len(),
        exact_match_count,
        mercurio_only,
        pilot_only,
        mismatches,
    })
}

fn ensure_unique_match_keys(
    elements: &[SemanticSnapshotElement],
) -> Result<(), SemanticCompareError> {
    let mut seen = BTreeSet::new();
    for element in elements {
        if !seen.insert(element.match_key.clone()) {
            return Err(SemanticCompareError::DuplicateMatchKey(
                element.match_key.clone(),
            ));
        }
    }
    Ok(())
}

fn snapshot_index<'a>(
    elements: &'a [SemanticSnapshotElement],
) -> Result<BTreeMap<String, &'a SemanticSnapshotElement>, SemanticCompareError> {
    let mut index = BTreeMap::new();
    for element in elements {
        if index.insert(element.match_key.clone(), element).is_some() {
            return Err(SemanticCompareError::DuplicateMatchKey(
                element.match_key.clone(),
            ));
        }
    }
    Ok(index)
}

fn compare_element_pair(
    mercurio: &SemanticSnapshotElement,
    pilot: &SemanticSnapshotElement,
) -> Option<SemanticElementMismatch> {
    let metatype = (mercurio.metatype != pilot.metatype).then(|| SemanticValueMismatch {
        mercurio: mercurio.metatype.clone().unwrap_or_default(),
        pilot: pilot.metatype.clone().unwrap_or_default(),
    });
    let metatype_specialization_chain = (mercurio.metatype_specialization_chain
        != pilot.metatype_specialization_chain)
        .then(|| SemanticValueMismatch {
            mercurio: mercurio.metatype_specialization_chain.clone(),
            pilot: pilot.metatype_specialization_chain.clone(),
        });
    let filtered_mercurio_attributes = normalize_element_compare_attributes(
        mercurio,
        filtered_compare_attributes(&mercurio.declared_attributes, &pilot.declared_attributes),
    );
    let filtered_pilot_attributes = normalize_element_compare_attributes(
        pilot,
        filtered_compare_attributes(&pilot.declared_attributes, &mercurio.declared_attributes),
    );
    let declared_attributes =
        (filtered_mercurio_attributes != filtered_pilot_attributes).then(|| {
            SemanticValueMismatch {
                mercurio: filtered_mercurio_attributes,
                pilot: filtered_pilot_attributes,
            }
        });

    if metatype.is_none()
        && metatype_specialization_chain.is_none()
        && declared_attributes.is_none()
    {
        return None;
    }

    Some(SemanticElementMismatch {
        match_key: mercurio.match_key.clone(),
        mercurio_id: mercurio.id.clone(),
        pilot_id: pilot.id.clone(),
        metatype,
        metatype_specialization_chain,
        declared_attributes,
    })
}

fn filtered_compare_attributes(
    primary: &BTreeMap<String, SemanticSnapshotAttribute>,
    secondary: &BTreeMap<String, SemanticSnapshotAttribute>,
) -> BTreeMap<String, SemanticSnapshotAttribute> {
    primary
        .iter()
        .filter(|(name, attribute)| {
            compare_attribute_is_canonical(name)
                && !attribute_is_compare_optional_when_missing(
                    name,
                    attribute,
                    secondary.get(*name),
                )
        })
        .map(|(name, attribute)| {
            let mut normalized = attribute.clone();
            normalized.declared_by = None;
            (name.clone(), normalized)
        })
        .collect()
}

fn normalize_element_compare_attributes(
    element: &SemanticSnapshotElement,
    mut attributes: BTreeMap<String, SemanticSnapshotAttribute>,
) -> BTreeMap<String, SemanticSnapshotAttribute> {
    promote_effective_name_values(&mut attributes, "declared_name");
    promote_effective_name_values(&mut attributes, "name");

    if attributes
        .get("declared_name")
        .is_some_and(attribute_has_no_value)
        && let Some(name_attribute) = attributes.get("name").cloned()
        && !attribute_has_no_value(&name_attribute)
    {
        attributes.insert("declared_name".to_string(), name_attribute);
    }

    if element.metatype.as_deref() == Some("ReferenceUsage") {
        if let Some(name_attribute) = attributes.get("name").cloned()
            && !attribute_has_no_value(&name_attribute)
        {
            attributes.insert("declared_name".to_string(), name_attribute);
        }
        if attribute_contains_identifier(attributes.get("redefines"), "sourceOutput") {
            set_attribute_value(&mut attributes, "owner", json!(["source"]));
            set_attribute_value(&mut attributes, "featuring_type", json!(["?"]));
        } else if attribute_contains_identifier(attributes.get("redefines"), "targetInput") {
            set_attribute_value(&mut attributes, "owner", json!(["target"]));
            set_attribute_value(&mut attributes, "featuring_type", json!(["?"]));
        }
    }

    if element.metatype.as_deref() == Some("PartUsage") {
        strip_part_family_default(&mut attributes, "specializes");
        strip_part_family_default(&mut attributes, "subsets");
    }

    attributes
}

fn promote_effective_name_values(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    name: &str,
) {
    if let Some(attribute) = attributes.get_mut(name)
        && !attribute.has_direct_value
        && attribute.direct_value.is_none()
        && attribute.has_effective_value
        && let Some(effective_value) = attribute.effective_value.clone()
    {
        attribute.has_direct_value = true;
        attribute.direct_value = Some(effective_value);
    }
}

fn attribute_contains_identifier(
    attribute: Option<&SemanticSnapshotAttribute>,
    identifier: &str,
) -> bool {
    attribute
        .and_then(|attribute| {
            attribute
                .effective_value
                .as_ref()
                .or(attribute.direct_value.as_ref())
        })
        .is_some_and(|value| value_contains_identifier(value, identifier))
}

fn value_contains_identifier(value: &Value, identifier: &str) -> bool {
    match value {
        Value::String(raw) => canonical_identifier(raw) == identifier,
        Value::Array(items) => items
            .iter()
            .any(|item| value_contains_identifier(item, identifier)),
        _ => false,
    }
}

fn set_attribute_value(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    name: &str,
    value: Value,
) {
    if let Some(attribute) = attributes.get_mut(name) {
        attribute.has_direct_value = true;
        attribute.direct_value = Some(value.clone());
        attribute.has_effective_value = true;
        attribute.effective_value = Some(value);
    }
}

fn strip_part_family_default(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    name: &str,
) {
    if let Some(attribute) = attributes.get_mut(name) {
        if let Some(normalized) = strip_identifier_from_attribute(attribute, "parts") {
            attribute.has_direct_value = true;
            attribute.direct_value = Some(normalized.clone());
            attribute.has_effective_value = true;
            attribute.effective_value = Some(normalized);
        }
    }
}

fn strip_identifier_from_attribute(
    attribute: &SemanticSnapshotAttribute,
    identifier: &str,
) -> Option<Value> {
    let value = attribute
        .effective_value
        .as_ref()
        .or(attribute.direct_value.as_ref())?;
    let items = value.as_array()?;
    if items.len() < 2
        || !items
            .iter()
            .any(|item| value_contains_identifier(item, identifier))
    {
        return None;
    }
    let filtered = items
        .iter()
        .filter(|item| !value_contains_identifier(item, identifier))
        .cloned()
        .collect::<Vec<_>>();
    Some(Value::Array(filtered))
}

fn compare_attribute_is_canonical(name: &str) -> bool {
    name.starts_with("is_")
        || matches!(
            name,
            "declared_name"
                | "name"
                | "owner"
                | "type"
                | "definition"
                | "specializes"
                | "features"
                | "members"
                | "featuring_type"
                | "direction"
                | "subsets"
                | "redefines"
        )
}

fn attribute_is_compare_optional_when_missing(
    name: &str,
    primary: &SemanticSnapshotAttribute,
    secondary: Option<&SemanticSnapshotAttribute>,
) -> bool {
    (matches!(name, "features" | "members")
        && (secondary.is_some() || attribute_has_no_value(primary)))
        || (attribute_has_no_value(primary)
            && secondary.is_some_and(attribute_is_effectively_default))
        || (name == "owner"
            && ((attribute_has_no_value(primary)
                && secondary
                    .and_then(single_effective_identifier_from_attribute)
                    .as_deref()
                    == Some("Namespace"))
                || (secondary.is_none()
                    && single_effective_identifier_from_attribute(primary).as_deref()
                        == Some("Namespace"))
                || (secondary.is_some_and(attribute_has_no_value)
                    && single_effective_identifier_from_attribute(primary).as_deref()
                        == Some("Namespace"))))
}

fn attribute_has_no_value(attribute: &SemanticSnapshotAttribute) -> bool {
    !attribute.has_direct_value
        && attribute.direct_value.is_none()
        && !attribute.has_effective_value
        && attribute.effective_value.is_none()
}

fn attribute_is_effectively_default(attribute: &SemanticSnapshotAttribute) -> bool {
    if attribute_has_no_value(attribute) {
        return true;
    }

    matches!(
        attribute
            .direct_value
            .as_ref()
            .or(attribute.effective_value.as_ref()),
        Some(Value::Bool(false))
    )
}

fn snapshot_element(
    graph: &Graph,
    registry: &MetamodelAttributeRegistry,
    node_id: NodeId,
    focus_source_file: &str,
    mode: SnapshotMode,
) -> Option<SemanticSnapshotElement> {
    let element = graph.element(node_id)?;
    let raw_metatype_key = element
        .properties
        .get("metatype")
        .and_then(Value::as_str)
        .map(str::to_string);
    let raw_metatype = raw_metatype_key.as_deref().map(canonical_identifier);
    if !is_compare_visible_kind(&element.kind)
        && !raw_metatype.as_deref().is_some_and(is_compare_visible_kind)
    {
        return None;
    }
    let metadata = element.properties.get("metadata")?.as_object()?;
    let source_file = metadata.get("source_file")?.as_str()?;
    if !source_file_matches_relative_path(source_file, focus_source_file) {
        return None;
    }

    let source_span = metadata
        .get("source_span")
        .and_then(Value::as_object)
        .map(|span| SemanticSourceSpan {
            start_line: span.get("start_line").and_then(Value::as_u64),
            end_line: span.get("end_line").and_then(Value::as_u64),
        });
    let declared_name = element
        .properties
        .get("declared_name")
        .and_then(Value::as_str)
        .map(str::to_string);
    let fallback_name = element
        .properties
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string);

    let attribute_query = query_element_attributes(
        graph,
        registry,
        node_id,
        metatype_override_for(mode, graph, element, raw_metatype_key.as_deref()).as_ref(),
    )
    .unwrap_or_else(|| ElementAttributeQuery {
        metatype: None,
        metatype_specialization_chain: Vec::new(),
        rows: Vec::new(),
    });
    let queried_metatype = attribute_query
        .metatype
        .as_ref()
        .map(|summary| canonical_identifier(&summary.id));
    let metatype = match mode {
        SnapshotMode::Mercurio => raw_metatype.clone().or(queried_metatype.clone()),
        SnapshotMode::Pilot => queried_metatype.clone().or(raw_metatype.clone()),
    };
    let metatype_specialization_chain = sorted_strings(
        attribute_query
            .metatype_specialization_chain
            .iter()
            .map(|summary| canonical_identifier(&summary.id))
            .collect(),
    );
    let declared_attributes =
        snapshot_attributes_from_rows(attribute_query.rows, &element.properties);
    if is_pilot_connection_end_typing_artifact(
        graph,
        element,
        mode,
        declared_name.as_ref(),
        source_file,
        source_span.as_ref(),
        &declared_attributes,
    ) {
        return None;
    }
    let label = declared_name.clone().unwrap_or_else(|| {
        if canonical_identifier(&element.kind) == "ConnectionUsage" {
            "ConnectionUsage".to_string()
        } else if let Some(metatype) = metatype.clone() {
            metatype
        } else {
            fallback_name
                .clone()
                .unwrap_or_else(|| canonical_identifier(&element.element_id))
        }
    });

    let match_name = if declared_name.is_none()
        && fallback_name
            .as_deref()
            .is_some_and(|name| matches!(name, "acceptSubactions" | "successionFlows"))
    {
        &label
    } else {
        declared_name
            .as_deref()
            .or(fallback_name.as_deref())
            .unwrap_or(&label)
    };

    Some(SemanticSnapshotElement {
        match_key: build_match_key(source_file, source_span.as_ref(), match_name),
        id: element.element_id.clone(),
        label,
        kind: element.kind.clone(),
        layer: element.layer,
        declared_name,
        source_span,
        metatype,
        metatype_specialization_chain,
        declared_attributes,
    })
}

fn is_compare_visible_kind(kind: &str) -> bool {
    let kind = canonical_identifier(kind);
    kind == "Package" || kind.ends_with("Definition") || kind.ends_with("Usage")
}

fn is_pilot_connection_end_typing_artifact(
    graph: &Graph,
    element: &crate::graph::Element,
    mode: SnapshotMode,
    declared_name: Option<&String>,
    source_file: &str,
    source_span: Option<&SemanticSourceSpan>,
    declared_attributes: &BTreeMap<String, SemanticSnapshotAttribute>,
) -> bool {
    if !matches!(mode, SnapshotMode::Pilot) {
        return false;
    }
    if canonical_identifier(&element.kind) != "ReferenceUsage" || declared_name.is_some() {
        return false;
    }
    let Some(span) = source_span else {
        return false;
    };
    let Some(start_line) = span.start_line else {
        return false;
    };
    if span.end_line != Some(start_line) {
        return false;
    }
    let Some(owner_name) = single_effective_identifier(declared_attributes.get("owner")) else {
        return false;
    };
    let Some(type_name) = single_effective_identifier(declared_attributes.get("type")) else {
        return false;
    };

    graph.elements().iter().any(|candidate| {
        canonical_identifier(&candidate.kind) == "PartUsage"
            && element_source_file_matches(candidate, source_file)
            && element_start_line(candidate) == Some(start_line)
            && element_name(candidate).as_deref() == Some(owner_name.as_str())
            && element_type_matches(candidate, &type_name)
            && candidate
                .properties
                .get("is_end")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    })
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

fn metatype_override_for(
    mode: SnapshotMode,
    graph: &Graph,
    element: &crate::graph::Element,
    raw_metatype_key: Option<&str>,
) -> Option<MetatypeQueryOverride> {
    match mode {
        SnapshotMode::Mercurio => raw_metatype_key.map(|metatype_key| MetatypeQueryOverride {
            metatype_key: Some(metatype_key.to_string()),
            specialization_chain: graph
                .node_id(metatype_key)
                .map(|node_id| collect_specialization_ancestors(graph, node_id))
                .unwrap_or_default()
                .into_iter()
                .map(|ancestor| ancestor.element_id.clone())
                .collect(),
        }),
        SnapshotMode::Pilot => Some(MetatypeQueryOverride {
            metatype_key: Some(element.kind.clone()),
            specialization_chain: element
                .properties
                .get("metatype_specialization_chain")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
        }),
    }
}

fn snapshot_attributes_from_rows(
    rows: Vec<AttributeRow>,
    element_properties: &BTreeMap<String, Value>,
) -> BTreeMap<String, SemanticSnapshotAttribute> {
    let mut attributes = rows
        .into_iter()
        .map(|row| {
            let name = row.name.clone();
            let mut attribute = SemanticSnapshotAttribute {
                declared_by: row
                    .declared_by
                    .map(|summary| canonical_identifier(&summary.id)),
                has_direct_value: row.has_direct_value,
                direct_value: row
                    .direct_value
                    .as_ref()
                    .map(|value| normalize_compare_value_for_key(&name, value)),
                has_effective_value: row.has_effective_value,
                effective_value: row
                    .effective_value
                    .as_ref()
                    .map(|value| normalize_compare_value_for_key(&name, value)),
            };
            if name.starts_with("is_")
                && !attribute.has_direct_value
                && attribute.direct_value.is_none()
                && !attribute.has_effective_value
                && attribute.effective_value.is_none()
            {
                attribute.has_direct_value = true;
                attribute.direct_value = Some(Value::Bool(false));
                attribute.has_effective_value = true;
                attribute.effective_value = Some(Value::Bool(false));
            }
            (name.clone(), attribute)
        })
        .collect::<BTreeMap<_, _>>();

    for (name, value) in element_properties {
        if !compare_attribute_supports_alias(name) || attributes.contains_key(name) {
            continue;
        }
        let normalized = normalize_compare_value_for_key(name, value);
        match &normalized {
            Value::Null => continue,
            Value::Array(values) if values.is_empty() => continue,
            Value::String(text) if text.is_empty() => continue,
            _ => {}
        }
        attributes.insert(
            name.clone(),
            SemanticSnapshotAttribute {
                declared_by: None,
                has_direct_value: true,
                direct_value: Some(normalized.clone()),
                has_effective_value: true,
                effective_value: Some(normalized),
            },
        );
    }

    copy_attribute_alias(&mut attributes, "subsetted_features", "subsets");
    copy_attribute_alias(&mut attributes, "redefined_features", "redefines");
    copy_attribute_alias(&mut attributes, "specialized_features", "specializes");

    attributes
}

fn copy_attribute_alias(
    attributes: &mut BTreeMap<String, SemanticSnapshotAttribute>,
    source: &str,
    alias: &str,
) {
    if attributes.contains_key(alias) {
        return;
    }
    if let Some(value) = attributes.get(source).cloned() {
        attributes.insert(alias.to_string(), value);
    }
}

fn compare_attribute_supports_alias(name: &str) -> bool {
    compare_attribute_is_canonical(name)
        || matches!(
            name,
            "specialized_features" | "subsetted_features" | "redefined_features"
        )
}

fn build_match_key(
    source_file: &str,
    source_span: Option<&SemanticSourceSpan>,
    declared_name: &str,
) -> String {
    let start_line = source_span
        .and_then(|span| span.start_line)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "?".to_string());
    format!(
        "{}:{}:{}",
        normalize_source_file(source_file),
        start_line,
        declared_name
    )
}

fn normalize_compare_value(value: &Value) -> Value {
    match value {
        Value::String(raw) => Value::String(canonical_identifier(raw)),
        Value::Array(items) => {
            let mut normalized = items
                .iter()
                .map(normalize_compare_value)
                .collect::<Vec<_>>();
            normalized.sort_by_key(|item| item.to_string());
            normalized.dedup();
            Value::Array(normalized)
        }
        Value::Object(map) => {
            let mut normalized = serde_json::Map::new();
            for (key, item) in map {
                normalized.insert(key.clone(), normalize_compare_value(item));
            }
            Value::Object(normalized)
        }
        _ => value.clone(),
    }
}

fn normalize_compare_value_for_key(key: &str, value: &Value) -> Value {
    match (key, value) {
        ("featuring_type", Value::String(raw)) => Value::Array(vec![Value::String(
            canonical_compare_identifier_for_key(key, raw),
        )]),
        (
            "owner" | "type" | "definition" | "specializes" | "subsets" | "redefines",
            Value::String(raw),
        ) => Value::Array(vec![Value::String(canonical_compare_identifier_for_key(
            key, raw,
        ))]),
        (
            "owner" | "featuring_type" | "type" | "definition" | "specializes" | "subsets"
            | "redefines",
            Value::Array(items),
        ) => {
            let mut normalized = items
                .iter()
                .filter_map(|item| item.as_str())
                .map(|raw| Value::String(canonical_compare_identifier_for_key(key, raw)))
                .collect::<Vec<_>>();
            normalized.sort_by_key(|item| item.to_string());
            normalized.dedup();
            Value::Array(normalized)
        }
        _ => normalize_compare_value(value),
    }
}

fn source_file_matches_relative_path(source_file: &str, relative_path: &str) -> bool {
    let normalized_source = normalize_source_file(source_file);
    let normalized_relative = normalize_source_file(relative_path);
    normalized_source == normalized_relative || normalized_source.ends_with(&normalized_relative)
}

fn normalize_source_file(path: &str) -> String {
    path.replace('\\', "/")
}

fn canonical_identifier(value: &str) -> String {
    let tail = value.rsplit("::").next().unwrap_or(value);
    let tail = tail.rsplit('.').next().unwrap_or(tail);
    let tail = tail.trim_matches('\'');
    let tail = tail
        .split_once("_snapshots#")
        .map(|(prefix, _)| prefix)
        .or_else(|| tail.strip_suffix("_snapshots"))
        .unwrap_or(tail);
    tail.to_string()
}

fn canonical_compare_identifier_for_key(key: &str, value: &str) -> String {
    let canonical = canonical_identifier(value);
    if matches!(key, "owner" | "featuring_type") && canonical.chars().all(|ch| ch.is_ascii_digit())
    {
        if let Some(previous) = value
            .replace("::", ".")
            .split('.')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>()
            .iter()
            .rev()
            .nth(1)
        {
            return canonical_identifier(previous);
        }
    }
    if key == "featuring_type"
        && (canonical == "AcceptActionUsage"
            || canonical == "SuccessionFlowUsage"
            || canonical.contains("AcceptActionUsage")
            || canonical.contains("SuccessionFlowUsage"))
    {
        return "?".to_string();
    }
    if key == "featuring_type" && canonical.contains('_') {
        if let Some((_, tail)) = canonical.rsplit_once('_') {
            return tail.to_string();
        }
    }
    canonical
}

fn single_effective_identifier(attribute: Option<&SemanticSnapshotAttribute>) -> Option<String> {
    let value = attribute?.effective_value.as_ref()?;
    single_identifier_from_value(value)
}

fn single_effective_identifier_from_attribute(
    attribute: &SemanticSnapshotAttribute,
) -> Option<String> {
    attribute
        .effective_value
        .as_ref()
        .and_then(single_identifier_from_value)
}

fn single_identifier_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(raw) => Some(canonical_identifier(raw)),
        Value::Array(items) if items.len() == 1 => items.first().and_then(|item| match item {
            Value::String(raw) => Some(canonical_identifier(raw)),
            _ => None,
        }),
        _ => None,
    }
}

fn element_source_file_matches(element: &crate::graph::Element, source_file: &str) -> bool {
    element
        .properties
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("source_file"))
        .and_then(Value::as_str)
        .is_some_and(|candidate| {
            normalize_source_file(candidate) == normalize_source_file(source_file)
        })
}

fn element_start_line(element: &crate::graph::Element) -> Option<u64> {
    element
        .properties
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get("source_span"))
        .and_then(Value::as_object)
        .and_then(|span| span.get("start_line"))
        .and_then(Value::as_u64)
}

fn element_name(element: &crate::graph::Element) -> Option<String> {
    element
        .properties
        .get("declared_name")
        .or_else(|| element.properties.get("name"))
        .and_then(Value::as_str)
        .map(canonical_identifier)
}

fn element_type_matches(element: &crate::graph::Element, expected: &str) -> bool {
    match element.properties.get("type") {
        Some(Value::String(raw)) => canonical_identifier(raw) == expected,
        Some(Value::Array(items)) => items.iter().any(|item| match item {
            Value::String(raw) => canonical_identifier(raw) == expected,
            _ => false,
        }),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{
        SnapshotMode, build_semantic_snapshot, build_semantic_snapshot_with_registry,
        canonical_compare_identifier_for_key, compare_snapshots,
    };
    use crate::ir::{KirDocument, KirElement};
    use crate::{Graph, MetamodelAttributeRegistry};

    #[test]
    fn builds_and_compares_basic_snapshots() {
        let registry = MetamodelAttributeRegistry::build(
            &Graph::from_document(KirDocument {
                metadata: BTreeMap::new(),
                elements: vec![
                    KirElement {
                        id: "SysML::Systems::PartDefinition".to_string(),
                        kind: "MetadataDefinition".to_string(),
                        layer: 1,
                        properties: BTreeMap::new(),
                    },
                    KirElement {
                        id: "SysML::Systems::Definition".to_string(),
                        kind: "MetadataDefinition".to_string(),
                        layer: 1,
                        properties: BTreeMap::new(),
                    },
                    KirElement {
                        id: "KerML::Core::Type".to_string(),
                        kind: "Metaclass".to_string(),
                        layer: 0,
                        properties: BTreeMap::new(),
                    },
                ],
            })
            .unwrap(),
        );
        let mercurio = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "SysML::Systems::PartDefinition".to_string(),
                    kind: "SysML::Systems::PartDefinition".to_string(),
                    layer: 1,
                    properties: BTreeMap::from([(
                        "specializes".to_string(),
                        json!(["SysML::Systems::ItemDefinition"]),
                    )]),
                },
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "SysML::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("Vehicle")),
                        (
                            "specializes".to_string(),
                            json!(["SysML::Systems::PartDefinition"]),
                        ),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.sysml",
                                "source_span": { "start_line": 2, "end_line": 4 }
                            }),
                        ),
                    ]),
                },
            ],
        };
        let pilot = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "SysML::Systems::PartDefinition".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 1,
                    properties: BTreeMap::new(),
                },
                KirElement {
                    id: "Demo::Vehicle".to_string(),
                    kind: "PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("Vehicle")),
                        (
                            "specializes".to_string(),
                            json!(["SysML::Systems::PartDefinition"]),
                        ),
                        (
                            "metatype_specialization_chain".to_string(),
                            json!(["Definition", "Type"]),
                        ),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.sysml",
                                "source_span": { "start_line": 2, "end_line": 4 }
                            }),
                        ),
                    ]),
                },
            ],
        };

        let mercurio_snapshot = build_semantic_snapshot_with_registry(
            mercurio,
            "demo.sysml",
            SnapshotMode::Mercurio,
            &registry,
        )
        .unwrap();
        let pilot_snapshot = build_semantic_snapshot_with_registry(
            pilot,
            "demo.sysml",
            SnapshotMode::Pilot,
            &registry,
        )
        .unwrap();
        let report = compare_snapshots(mercurio_snapshot, pilot_snapshot).unwrap();

        assert_eq!(report.mercurio_count, 1);
        assert_eq!(report.pilot_count, 1);
        assert_eq!(report.exact_match_count, 0);
        assert_eq!(report.mismatches.len(), 1);
        assert_eq!(report.mismatches[0].match_key, "demo.sysml:2:Vehicle");
    }

    #[test]
    fn filters_pilot_connection_end_typing_artifacts_from_snapshot() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "feature.Demo.PressureSeat.bead".to_string(),
                    kind: "PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("declared_name".to_string(), json!("bead")),
                        ("type".to_string(), json!("TireBead")),
                        ("is_end".to_string(), json!(true)),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.sysml",
                                "source_span": { "start_line": 15, "end_line": 15 }
                            }),
                        ),
                    ]),
                },
                KirElement {
                    id: "pilot-anon-ref".to_string(),
                    kind: "ReferenceUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("owner".to_string(), json!("bead")),
                        ("type".to_string(), json!("TireBead")),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.sysml",
                                "source_span": { "start_line": 15, "end_line": 15 }
                            }),
                        ),
                    ]),
                },
            ],
        };

        let snapshot =
            build_semantic_snapshot(document, "demo.sysml", SnapshotMode::Pilot).unwrap();

        assert_eq!(snapshot.elements.len(), 1);
        assert_eq!(snapshot.elements[0].kind, "PartUsage");
        assert_eq!(snapshot.elements[0].label, "bead");
    }

    #[test]
    fn uses_name_property_to_disambiguate_same_line_anonymous_pilot_elements() {
        let document = KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "InputModel::demo::25::p".to_string(),
                    kind: "ReferenceUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("name".to_string(), json!("p")),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.sysml",
                                "source_span": { "start_line": 25, "end_line": 25 }
                            }),
                        ),
                    ]),
                },
                KirElement {
                    id: "InputModel::demo::25::receiver".to_string(),
                    kind: "ReferenceUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        ("name".to_string(), json!("receiver")),
                        (
                            "metadata".to_string(),
                            json!({
                                "source_file": "demo.sysml",
                                "source_span": { "start_line": 25, "end_line": 25 }
                            }),
                        ),
                    ]),
                },
            ],
        };

        let snapshot =
            build_semantic_snapshot(document, "demo.sysml", SnapshotMode::Pilot).unwrap();
        let keys = snapshot
            .elements
            .iter()
            .map(|element| element.match_key.clone())
            .collect::<Vec<_>>();

        assert_eq!(keys, vec!["demo.sysml:25:p", "demo.sysml:25:receiver"]);
    }

    #[test]
    fn normalizes_pilot_synthesized_featuring_type_names() {
        assert_eq!(
            canonical_compare_identifier_for_key(
                "featuring_type",
                "'Parts Example-1'::smallVehicle_eng"
            ),
            "eng"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("featuring_type", "bigVehicle_eng"),
            "eng"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("featuring_type", "'Parts Example-1'::Vehicle"),
            "Vehicle"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("featuring_type", "reference.PartTest.C.y.38"),
            "y"
        );
        assert_eq!(
            canonical_compare_identifier_for_key("owner", "reference.PartTest.C.y.38"),
            "y"
        );
    }
}
