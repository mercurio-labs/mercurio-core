use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::graph::{Element, Graph};

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedPropertyValue {
    pub value: Value,
    pub source: DerivedPropertySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedPropertySource {
    ExplicitProperty,
    InverseRelation,
    ForwardRelation,
    DeclaredName,
    DeclaredShortName,
    Layer,
    OwnerNameChain,
}

pub fn derived_properties(
    graph: &Graph,
    element: &Element,
) -> BTreeMap<String, DerivedPropertyValue> {
    let mut properties = BTreeMap::new();

    if let Some(value) = derived_owner(graph, element) {
        properties.insert("owner".to_string(), value);
    }
    if let Some(value) = derived_owned_element(graph, element) {
        properties.insert("owned_element".to_string(), value);
    }
    if let Some(value) = derived_name(element) {
        properties.insert("name".to_string(), value);
    }
    if let Some(value) = derived_short_name(element) {
        properties.insert("short_name".to_string(), value);
    }
    if let Some(value) = derived_qualified_name(graph, element) {
        properties.insert("qualified_name".to_string(), value);
    }
    properties.insert(
        "is_library_element".to_string(),
        DerivedPropertyValue {
            value: Value::Bool(element.layer < 2),
            source: DerivedPropertySource::Layer,
        },
    );

    properties
}

fn derived_owner(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    if let Some(value) = element.properties.get("owner") {
        return Some(DerivedPropertyValue {
            value: value.clone(),
            source: DerivedPropertySource::ExplicitProperty,
        });
    }

    for relation in ["members", "features", "owned_element"] {
        if let Some(edge) = graph.incoming(element.id, relation).next()
            && let Some(owner_id) = graph.element_id(edge.source)
        {
            return Some(DerivedPropertyValue {
                value: Value::String(owner_id.to_string()),
                source: DerivedPropertySource::InverseRelation,
            });
        }
    }

    None
}

fn derived_owned_element(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let mut ids = Vec::new();
    for relation in ["members", "features"] {
        for edge in graph.outgoing(element.id, relation) {
            if let Some(element_id) = graph.element_id(edge.target)
                && !ids.iter().any(|existing| existing == element_id)
            {
                ids.push(Value::String(element_id.to_string()));
            }
        }
    }

    (!ids.is_empty()).then_some(DerivedPropertyValue {
        value: Value::Array(ids),
        source: DerivedPropertySource::ForwardRelation,
    })
}

fn derived_name(element: &Element) -> Option<DerivedPropertyValue> {
    element
        .properties
        .get("declared_name")
        .cloned()
        .map(|value| DerivedPropertyValue {
            value,
            source: DerivedPropertySource::DeclaredName,
        })
}

fn derived_short_name(element: &Element) -> Option<DerivedPropertyValue> {
    element
        .properties
        .get("declared_short_name")
        .cloned()
        .map(|value| DerivedPropertyValue {
            value,
            source: DerivedPropertySource::DeclaredShortName,
        })
}

fn derived_qualified_name(graph: &Graph, element: &Element) -> Option<DerivedPropertyValue> {
    let mut segments = Vec::new();
    let mut current = element;
    let mut seen = BTreeSet::new();

    loop {
        if !seen.insert(current.element_id.clone()) {
            return None;
        }
        segments.push(local_name(current)?);
        let Some(owner_id) = derived_owner_id(graph, current) else {
            break;
        };
        current = graph.element_by_element_id(&owner_id)?;
    }

    segments.reverse();
    Some(DerivedPropertyValue {
        value: Value::String(segments.join("::")),
        source: DerivedPropertySource::OwnerNameChain,
    })
}

fn local_name(element: &Element) -> Option<String> {
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
}

fn derived_owner_id(graph: &Graph, element: &Element) -> Option<String> {
    match derived_owner(graph, element)?.value {
        Value::String(owner_id) => Some(owner_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::ir::{KirDocument, KirElement};

    #[test]
    fn derives_owner_and_owned_element_from_graph_relations() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![
                KirElement {
                    id: "type.Demo.Vehicle".to_string(),
                    kind: "SysML::Systems::PartDefinition".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "features".to_string(),
                        json!(["feature.Demo.Vehicle.engine"]),
                    )]),
                },
                KirElement {
                    id: "feature.Demo.Vehicle.engine".to_string(),
                    kind: "SysML::Parts::PartUsage".to_string(),
                    layer: 2,
                    properties: BTreeMap::from([(
                        "declared_name".to_string(),
                        Value::String("engine".to_string()),
                    )]),
                },
            ],
        })
        .unwrap();

        let owner = graph.element_by_element_id("type.Demo.Vehicle").unwrap();
        let child = graph
            .element_by_element_id("feature.Demo.Vehicle.engine")
            .unwrap();
        let owner_derived = derived_properties(&graph, owner);
        let child_derived = derived_properties(&graph, child);

        assert_eq!(
            owner_derived.get("owned_element").map(|value| &value.value),
            Some(&json!(["feature.Demo.Vehicle.engine"]))
        );
        assert_eq!(
            child_derived.get("owner").map(|value| &value.value),
            Some(&Value::String("type.Demo.Vehicle".to_string()))
        );
        assert_eq!(
            child_derived.get("name").map(|value| &value.value),
            Some(&Value::String("engine".to_string()))
        );
    }

    #[test]
    fn derives_qualified_name_from_owner_name_chain_only() {
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

        let element = graph.element_by_element_id("type.generated.2").unwrap();
        let derived = derived_properties(&graph, element);

        assert_eq!(
            derived.get("qualified_name").map(|value| &value.value),
            Some(&Value::String("Demo::Vehicle".to_string()))
        );
    }

    #[test]
    fn omits_qualified_name_when_owner_name_chain_is_incomplete() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "SysML::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::new(),
            }],
        })
        .unwrap();

        let element = graph.element_by_element_id("type.Demo.Vehicle").unwrap();
        let derived = derived_properties(&graph, element);

        assert!(!derived.contains_key("qualified_name"));
    }
}
