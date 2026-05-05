use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::Value;

use crate::derived::derived_properties;
use crate::graph::{Graph, NodeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementSummary {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub layer: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeValueSource {
    pub element: ElementSummary,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeRow {
    pub name: String,
    pub declared_by: Option<ElementSummary>,
    pub origin_kind: String,
    pub has_direct_value: bool,
    pub direct_value: Option<Value>,
    pub has_effective_value: bool,
    pub effective_value: Option<Value>,
    pub inherited_values: Vec<AttributeValueSource>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementAttributeQuery {
    pub metatype: Option<ElementSummary>,
    pub metatype_specialization_chain: Vec<ElementSummary>,
    pub rows: Vec<AttributeRow>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetatypeQueryOverride {
    pub metatype_key: Option<String>,
    pub specialization_chain: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetamodelAttributeDeclaration {
    pub(crate) name: String,
    pub(crate) declared_by: ElementSummary,
    pub(crate) type_label: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MetamodelAttributeRegistry {
    metatype_lookup: HashMap<String, String>,
    metatype_summaries: HashMap<String, ElementSummary>,
    declared_attributes: HashMap<String, Vec<MetamodelAttributeDeclaration>>,
}

impl MetamodelAttributeRegistry {
    pub fn build(graph: &Graph) -> Self {
        let mut registry = Self::default();

        for element in graph.elements() {
            let summary = element_summary(element);
            registry
                .metatype_summaries
                .insert(element.element_id.clone(), summary.clone());
            for key in metatype_lookup_keys(&element.element_id, &summary.label) {
                registry
                    .metatype_lookup
                    .entry(key)
                    .or_insert_with(|| element.element_id.clone());
            }
        }

        for element in graph.elements() {
            let mut declarations = BTreeMap::new();
            for feature_id in owned_feature_ids(element) {
                let Some(feature_node_id) = graph.node_id(&feature_id) else {
                    continue;
                };
                let Some(feature) = graph.element(feature_node_id) else {
                    continue;
                };
                let Some(declared_name) = declared_name(feature) else {
                    continue;
                };
                let query_key = attribute_query_key(&declared_name);
                declarations.entry(query_key.clone()).or_insert_with(|| {
                    MetamodelAttributeDeclaration {
                        name: query_key,
                        declared_by: element_summary(element),
                        type_label: feature_type_label(feature),
                    }
                });
            }

            if !declarations.is_empty() {
                registry.declared_attributes.insert(
                    element.element_id.clone(),
                    declarations.into_values().collect(),
                );
            }
        }

        registry
    }

    fn resolve_metatype_summary(&self, key: &str) -> Option<ElementSummary> {
        let resolved = self
            .metatype_lookup
            .get(key)
            .cloned()
            .or_else(|| {
                self.metatype_lookup
                    .get(&canonical_identifier(key))
                    .cloned()
            })
            .or_else(|| self.metatype_lookup.get(&label_for_id(key)).cloned())
            .unwrap_or_else(|| key.to_string());
        self.metatype_summaries.get(&resolved).cloned()
    }

    pub(crate) fn declared_attributes_for(
        &self,
        metatype_id: &str,
    ) -> &[MetamodelAttributeDeclaration] {
        self.declared_attributes
            .get(metatype_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn declared_attribute_names_for(&self, metatype_id: &str) -> Vec<String> {
        self.declared_attributes_for(metatype_id)
            .iter()
            .map(|declaration| declaration.name.clone())
            .collect()
    }
}

pub fn query_element_attributes(
    graph: &Graph,
    registry: &MetamodelAttributeRegistry,
    node_id: NodeId,
    metatype_override: Option<&MetatypeQueryOverride>,
) -> Option<ElementAttributeQuery> {
    let element = graph.element(node_id)?;
    let ancestors = collect_specialization_ancestors(graph, node_id);
    let derived_properties = derived_properties(graph, element);
    let effective_properties =
        effective_properties_with_derived(&ancestors, &element.properties, &derived_properties);

    let (metatype, metatype_specialization_chain) = if let Some(override_query) = metatype_override
    {
        let metatype = override_query
            .metatype_key
            .as_deref()
            .and_then(|key| registry.resolve_metatype_summary(key));
        let metatype_specialization_chain = override_query
            .specialization_chain
            .iter()
            .filter_map(|key| registry.resolve_metatype_summary(key))
            .collect::<Vec<_>>();
        (metatype, metatype_specialization_chain)
    } else {
        let metatype_element = element_metatype(graph, node_id)
            .or_else(|| ancestors.first().copied())
            .or_else(|| is_metamodel_type(element).then_some(element));
        let metatype = metatype_element.map(element_summary);
        let metatype_specialization_chain = metatype_element
            .map(|item| collect_specialization_ancestors(graph, item.id))
            .unwrap_or_default()
            .into_iter()
            .map(element_summary)
            .collect::<Vec<_>>();
        (metatype, metatype_specialization_chain)
    };

    let mut declarations = BTreeMap::new();
    if let Some(summary) = &metatype {
        for declaration in registry.declared_attributes_for(&summary.id) {
            declarations
                .entry(declaration.name.clone())
                .or_insert_with(|| declaration.clone());
        }
    }
    for summary in &metatype_specialization_chain {
        for declaration in registry.declared_attributes_for(&summary.id) {
            declarations
                .entry(declaration.name.clone())
                .or_insert_with(|| declaration.clone());
        }
    }
    for name in derived_properties.keys() {
        declarations
            .entry(name.clone())
            .or_insert_with(|| MetamodelAttributeDeclaration {
                name: name.clone(),
                declared_by: metatype.clone().unwrap_or_else(|| element_summary(element)),
                type_label: None,
            });
    }

    let rows = declarations
        .into_values()
        .map(|declaration| {
            let direct_value = element.properties.get(&declaration.name).cloned();
            let effective_value = effective_properties.get(&declaration.name).cloned();
            let inherited_values = ancestors
                .iter()
                .filter_map(|ancestor| {
                    ancestor
                        .properties
                        .get(&declaration.name)
                        .cloned()
                        .map(|value| AttributeValueSource {
                            element: element_summary(ancestor),
                            value,
                        })
                })
                .collect::<Vec<_>>();
            let origin_kind = if derived_properties.contains_key(&declaration.name) {
                "derived"
            } else if direct_value.is_some() {
                "direct"
            } else if !inherited_values.is_empty() && effective_value.is_some() {
                "inherited"
            } else if effective_value.is_some() {
                "derived"
            } else {
                "declared"
            };

            AttributeRow {
                name: declaration.name,
                declared_by: Some(declaration.declared_by),
                origin_kind: origin_kind.to_string(),
                has_direct_value: direct_value.is_some(),
                direct_value,
                has_effective_value: effective_value.is_some(),
                effective_value,
                inherited_values,
            }
        })
        .collect::<Vec<_>>();

    Some(ElementAttributeQuery {
        metatype,
        metatype_specialization_chain,
        rows,
    })
}

pub fn element_metatype(graph: &Graph, node_id: NodeId) -> Option<&crate::graph::Element> {
    graph
        .outgoing(node_id, "metatype")
        .next()
        .and_then(|edge| graph.element(edge.target))
}

pub fn collect_specialization_ancestors(
    graph: &Graph,
    node_id: NodeId,
) -> Vec<&crate::graph::Element> {
    let mut seen = BTreeSet::new();
    let mut ancestors = Vec::new();
    collect_specialization_ancestors_into(graph, node_id, &mut seen, &mut ancestors);
    ancestors
}

pub fn effective_properties(
    ancestors: &[&crate::graph::Element],
    direct_properties: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    effective_properties_with_derived(ancestors, direct_properties, &BTreeMap::new())
}

pub fn effective_properties_with_derived(
    ancestors: &[&crate::graph::Element],
    direct_properties: &BTreeMap<String, Value>,
    derived_properties: &BTreeMap<String, crate::derived::DerivedPropertyValue>,
) -> BTreeMap<String, Value> {
    let mut effective = BTreeMap::new();
    for ancestor in ancestors.iter().rev() {
        for (key, value) in &ancestor.properties {
            effective.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in direct_properties {
        effective.insert(key.clone(), value.clone());
    }
    for (key, value) in derived_properties {
        effective.insert(key.clone(), value.value.clone());
    }
    effective
}

fn collect_specialization_ancestors_into<'a>(
    graph: &'a Graph,
    node_id: NodeId,
    seen: &mut BTreeSet<NodeId>,
    ancestors: &mut Vec<&'a crate::graph::Element>,
) {
    let parent_ids = graph
        .outgoing(node_id, "specializes")
        .map(|edge| edge.target)
        .collect::<Vec<_>>();

    for parent_id in parent_ids {
        if !seen.insert(parent_id) {
            continue;
        }

        if let Some(parent) = graph.element(parent_id) {
            ancestors.push(parent);
            collect_specialization_ancestors_into(graph, parent_id, seen, ancestors);
        }
    }
}

fn owned_feature_ids(element: &crate::graph::Element) -> Vec<String> {
    element
        .properties
        .get("features")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn declared_name(element: &crate::graph::Element) -> Option<String> {
    element
        .properties
        .get("declared_name")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            element
                .properties
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            element
                .properties
                .get("metadata")
                .and_then(Value::as_object)
                .and_then(|metadata| metadata.get("declared_name"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            element
                .properties
                .get("metadata")
                .and_then(Value::as_object)
                .and_then(|metadata| metadata.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn feature_type_label(element: &crate::graph::Element) -> Option<String> {
    relation_value_label(element.properties.get("type"))
}

fn relation_value_label(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(entry)) => Some(label_for_id(entry)),
        Some(Value::Array(entries)) => entries.iter().find_map(|entry| match entry {
            Value::String(item) => Some(label_for_id(item)),
            _ => None,
        }),
        _ => None,
    }
}

fn metatype_lookup_keys(id: &str, label: &str) -> Vec<String> {
    let mut keys = BTreeSet::new();
    keys.insert(id.to_string());
    keys.insert(canonical_identifier(id));
    keys.insert(label.to_string());
    keys.into_iter().collect()
}

fn attribute_query_key(declared_name: &str) -> String {
    match declared_name {
        "ownedFeature" => "features".to_string(),
        "ownedMember" => "members".to_string(),
        "ownedSpecialization" => "specializes".to_string(),
        "documentation" => "doc".to_string(),
        "declaredName" => "declared_name".to_string(),
        "declaredShortName" => "declared_short_name".to_string(),
        "shortName" => "short_name".to_string(),
        "isLibraryElement" => "is_library_element".to_string(),
        "isAbstract" => "is_abstract".to_string(),
        "isDerived" => "is_derived".to_string(),
        "isEnd" => "is_end".to_string(),
        "isOrdered" => "is_ordered".to_string(),
        "isUnique" => "is_unique".to_string(),
        "isVariable" => "is_variable".to_string(),
        "isImplied" => "is_implied".to_string(),
        "featuringType" => "featuring_type".to_string(),
        "chainingFeature" => "chaining_feature".to_string(),
        other => to_snake_case(other),
    }
}

fn is_metamodel_type(element: &crate::graph::Element) -> bool {
    matches!(element.kind.as_str(), "Metaclass" | "MetadataDefinition")
}

fn to_snake_case(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for (index, ch) in value.chars().enumerate() {
        if ch.is_ascii_uppercase() && index > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

fn element_summary(element: &crate::graph::Element) -> ElementSummary {
    ElementSummary {
        id: element.element_id.clone(),
        label: label_for_id(&element.element_id),
        kind: element.kind.clone(),
        layer: element.layer,
    }
}

fn canonical_identifier(value: &str) -> String {
    let tail = value.rsplit("::").next().unwrap_or(value);
    let tail = tail.rsplit('.').next().unwrap_or(tail);
    tail.trim_matches('\'').to_string()
}

fn label_for_id(value: &str) -> String {
    canonical_identifier(value)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};

    use super::*;
    use crate::ir::{KirDocument, KirElement};

    #[test]
    fn derived_properties_are_effective_rows_without_direct_values() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "pkg.generated.1".to_string(),
                    kind: "SysML::Package".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "declared_name".to_string(),
                        Value::String("Demo".to_string()),
                    )]),
                },
                KirElement {
                    id: "type.generated.2".to_string(),
                    kind: "SysML::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([
                        (
                            "declared_name".to_string(),
                            Value::String("Vehicle".to_string()),
                        ),
                        (
                            "owner".to_string(),
                            Value::String("pkg.generated.1".to_string()),
                        ),
                    ]),
                },
            ],
        })
        .unwrap();
        let registry = MetamodelAttributeRegistry::build(&graph);
        let node_id = graph.node_id("type.generated.2").unwrap();
        let query = query_element_attributes(&graph, &registry, node_id, None).unwrap();

        let qualified_name = query
            .rows
            .iter()
            .find(|row| row.name == "qualified_name")
            .unwrap();
        assert_eq!(qualified_name.origin_kind, "derived");
        assert!(!qualified_name.has_direct_value);
        assert_eq!(
            qualified_name.effective_value,
            Some(Value::String("Demo::Vehicle".to_string()))
        );

        let name = query.rows.iter().find(|row| row.name == "name").unwrap();
        assert_eq!(name.origin_kind, "derived");
        assert_eq!(name.direct_value, None);
        assert_eq!(
            name.effective_value,
            Some(Value::String("Vehicle".to_string()))
        );

        let effective = effective_properties_with_derived(
            &collect_specialization_ancestors(&graph, node_id),
            &graph.element(node_id).unwrap().properties,
            &crate::derived::derived_properties(&graph, graph.element(node_id).unwrap()),
        );
        assert_eq!(
            effective.get("qualified_name"),
            Some(&Value::String("Demo::Vehicle".to_string()))
        );
        assert!(
            !graph
                .element(node_id)
                .unwrap()
                .properties
                .contains_key("qualified_name")
        );
    }

    #[test]
    fn derived_values_override_stale_direct_values_in_effective_view() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.generated.1".to_string(),
                kind: "SysML::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([
                    (
                        "declared_name".to_string(),
                        Value::String("Vehicle".to_string()),
                    ),
                    ("name".to_string(), Value::String("Stale".to_string())),
                ]),
            }],
        })
        .unwrap();
        let element = graph.element_by_element_id("type.generated.1").unwrap();
        let effective = effective_properties_with_derived(
            &[],
            &element.properties,
            &crate::derived::derived_properties(&graph, element),
        );

        assert_eq!(effective.get("name"), Some(&json!("Vehicle")));
    }
}
