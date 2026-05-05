use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use serde_json::Value;

use crate::ir::{KirDocument, KirElement};

pub type NodeId = u32;

#[derive(Debug, Clone)]
pub struct Graph {
    elements: Vec<Element>,
    by_element_id: HashMap<String, NodeId>,
    edges: Vec<Edge>,
    outgoing: HashMap<NodeId, Vec<Edge>>,
    incoming: HashMap<NodeId, Vec<Edge>>,
}

#[derive(Debug, Clone)]
pub struct Element {
    pub id: NodeId,
    pub element_id: String,
    pub kind: String,
    pub layer: u8,
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub relation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    DuplicateId(String),
    UnknownElement(String),
    NodeOverflow,
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateId(id) => write!(f, "duplicate element id: {id}"),
            Self::UnknownElement(id) => write!(f, "unknown element id: {id}"),
            Self::NodeOverflow => write!(f, "too many elements for u32 node ids"),
        }
    }
}

impl std::error::Error for GraphError {}

impl Graph {
    pub fn from_document(document: KirDocument) -> Result<Self, GraphError> {
        let mut by_element_id = HashMap::new();
        let mut elements = Vec::with_capacity(document.elements.len());

        for raw in document.elements {
            if by_element_id.contains_key(&raw.id) {
                return Err(GraphError::DuplicateId(raw.id));
            }

            let id = NodeId::try_from(elements.len()).map_err(|_| GraphError::NodeOverflow)?;
            by_element_id.insert(raw.id.clone(), id);
            elements.push(Element::from_raw(id, raw));
        }

        let mut graph = Self {
            elements,
            by_element_id,
            edges: Vec::new(),
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
        };
        graph.build_edges()?;
        Ok(graph)
    }

    fn build_edges(&mut self) -> Result<(), GraphError> {
        let known_ids: HashSet<&str> = self.by_element_id.keys().map(String::as_str).collect();

        for element in &self.elements {
            for (property, value) in &element.properties {
                if property == "element_id" {
                    continue;
                }
                for external_target in referenced_ids(value, &known_ids) {
                    let Some(&target) = self.by_element_id.get(external_target) else {
                        return Err(GraphError::UnknownElement(external_target.to_string()));
                    };
                    let edge = Edge {
                        source: element.id,
                        target,
                        relation: property.clone(),
                    };
                    self.outgoing
                        .entry(element.id)
                        .or_default()
                        .push(edge.clone());
                    self.incoming.entry(target).or_default().push(edge);
                    self.edges.push(Edge {
                        source: element.id,
                        target,
                        relation: property.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn element(&self, id: NodeId) -> Option<&Element> {
        self.elements.get(id as usize)
    }

    pub fn element_by_element_id(&self, element_id: &str) -> Option<&Element> {
        self.node_id(element_id).and_then(|id| self.element(id))
    }

    pub fn node_id(&self, element_id: &str) -> Option<NodeId> {
        self.by_element_id.get(element_id).copied()
    }

    pub fn element_id(&self, id: NodeId) -> Option<&str> {
        self.element(id).map(|element| element.element_id.as_str())
    }

    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn outgoing_edges(&self, id: NodeId) -> impl Iterator<Item = &Edge> {
        self.outgoing
            .get(&id)
            .into_iter()
            .flat_map(|edges| edges.iter())
    }

    pub fn incoming_edges(&self, id: NodeId) -> impl Iterator<Item = &Edge> {
        self.incoming
            .get(&id)
            .into_iter()
            .flat_map(|edges| edges.iter())
    }

    pub fn outgoing(&self, id: NodeId, relation: &str) -> impl Iterator<Item = &Edge> {
        self.outgoing_edges(id)
            .filter(move |edge| edge.relation == relation)
    }

    pub fn incoming(&self, id: NodeId, relation: &str) -> impl Iterator<Item = &Edge> {
        self.incoming_edges(id)
            .filter(move |edge| edge.relation == relation)
    }

    pub fn relation_targets(
        &self,
        element_id: &str,
        relation: &str,
    ) -> Result<Vec<&Element>, GraphError> {
        let node_id = self
            .node_id(element_id)
            .ok_or_else(|| GraphError::UnknownElement(element_id.to_string()))?;

        Ok(self
            .outgoing(node_id, relation)
            .filter_map(|edge| self.element(edge.target))
            .collect())
    }
}

impl Element {
    fn from_raw(id: NodeId, raw: KirElement) -> Self {
        let mut properties = raw.properties;
        properties.insert("element_id".to_string(), Value::String(raw.id.clone()));

        Self {
            id,
            element_id: raw.id,
            kind: raw.kind,
            layer: raw.layer,
            properties,
        }
    }
}

fn referenced_ids<'a>(value: &'a Value, known_ids: &HashSet<&str>) -> Vec<&'a str> {
    match value {
        Value::String(s) if known_ids.contains(s.as_str()) => vec![s],
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(s) if known_ids.contains(s.as_str()) => Some(s.as_str()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_element_id_as_property_without_creating_self_edge() {
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
        assert_eq!(element.element_id, "type.Demo.Vehicle");
        assert_eq!(
            element.properties.get("element_id"),
            Some(&Value::String("type.Demo.Vehicle".to_string()))
        );
        assert!(graph.edges().is_empty());
    }

    #[test]
    fn canonical_element_id_overwrites_mismatched_property() {
        let graph = Graph::from_document(KirDocument {
            metadata: BTreeMap::new(),
            elements: vec![KirElement {
                id: "type.Demo.Vehicle".to_string(),
                kind: "SysML::Systems::PartDefinition".to_string(),
                layer: 2,
                properties: BTreeMap::from([(
                    "element_id".to_string(),
                    Value::String("stale".to_string()),
                )]),
            }],
        })
        .unwrap();

        let element = graph.element_by_element_id("type.Demo.Vehicle").unwrap();
        assert_eq!(
            element.properties.get("element_id"),
            Some(&Value::String("type.Demo.Vehicle".to_string()))
        );
    }
}
