use std::collections::BTreeMap;

use mercurio_core::graph::Element;
use mercurio_core::runtime::Runtime;
use mercurio_core::views::{RequirementSourceDto, RequirementTableRowDto, requirements_table_view};
use mercurio_reasoner_api::{
    CapabilityDescriptor, CapabilityKind, EvidenceGraph, EvidenceNode, EvidenceNodeKind,
    FindingSeverity, REASONING_API_VERSION, ReasoningArtifact, ReasoningFinding, ReasoningReport,
    ReasoningStatus, SemanticContextRef, SemanticElementRef, SourceSpanRef,
};
use serde_json::{Value, json};

pub const REQUIREMENT_COVERAGE_CAPABILITY_ID: &str = "mercurio.requirement.coverage";
pub const SEMANTIC_IMPACT_CAPABILITY_ID: &str = "mercurio.semantic.impact";

pub fn builtin_reasoning_capabilities() -> Vec<CapabilityDescriptor> {
    vec![
        requirement_coverage_capability_descriptor(),
        semantic_impact_capability_descriptor(),
    ]
}

pub fn requirement_coverage_capability_descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: REQUIREMENT_COVERAGE_CAPABILITY_ID.to_string(),
        kind: CapabilityKind::RequirementCoverage,
        name: "Requirement Coverage".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: REASONING_API_VERSION.to_string(),
        deterministic: true,
        input_artifact_kinds: vec![
            "runtime_artifact".to_string(),
            "derived_indexes".to_string(),
        ],
        output_artifact_kinds: vec![
            "finding".to_string(),
            "evidence_graph".to_string(),
            "requirement_coverage_summary".to_string(),
        ],
    }
}

pub fn semantic_impact_capability_descriptor() -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: SEMANTIC_IMPACT_CAPABILITY_ID.to_string(),
        kind: CapabilityKind::StaticAnalysis,
        name: "Semantic Impact".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: REASONING_API_VERSION.to_string(),
        deterministic: true,
        input_artifact_kinds: vec!["runtime_artifact".to_string(), "semantic_graph".to_string()],
        output_artifact_kinds: vec![
            "finding".to_string(),
            "evidence_graph".to_string(),
            "semantic_impact_summary".to_string(),
        ],
    }
}

pub fn analyze_requirement_coverage(
    runtime: &Runtime,
    context: SemanticContextRef,
    request_id: impl Into<String>,
) -> ReasoningReport {
    let view = requirements_table_view(runtime.graph());
    let mut findings = Vec::new();
    let mut evidence_nodes = Vec::new();

    for requirement in &view.rows {
        evidence_nodes.push(requirement_evidence_node(requirement));

        if requirement.satisfied_by.is_empty() {
            findings.push(missing_trace_finding(
                requirement,
                "satisfy",
                "Requirement has no satisfaction evidence",
                "No satisfy relationship reaches this requirement.",
                FindingSeverity::Warning,
            ));
        }

        if requirement.verified_by.is_empty() {
            findings.push(missing_trace_finding(
                requirement,
                "verify",
                "Requirement has no verification evidence",
                "No verify relationship reaches this requirement.",
                FindingSeverity::Error,
            ));
        }
    }

    for warning in &view.warnings {
        findings.push(ReasoningFinding {
            id: "requirement.coverage.no_requirements".to_string(),
            title: "No requirements found".to_string(),
            severity: FindingSeverity::Warning,
            message: warning.clone(),
            elements: Vec::new(),
            source_spans: Vec::new(),
            evidence_ids: Vec::new(),
            properties: BTreeMap::new(),
        });
    }

    let status = if findings.iter().any(|finding| {
        matches!(
            finding.severity,
            FindingSeverity::Error | FindingSeverity::Critical
        )
    }) {
        ReasoningStatus::Failed
    } else if findings.is_empty() {
        ReasoningStatus::Passed
    } else {
        ReasoningStatus::Inconclusive
    };

    let summary_payload = json!({
        "requirementCount": view.rows.len(),
        "satisfiedCount": view.rows.iter().filter(|row| !row.satisfied_by.is_empty()).count(),
        "verifiedCount": view.rows.iter().filter(|row| !row.verified_by.is_empty()).count(),
        "findingCount": findings.len(),
    });

    ReasoningReport {
        request_id: request_id.into(),
        capability: requirement_coverage_capability_descriptor(),
        context,
        status,
        findings,
        artifacts: vec![ReasoningArtifact {
            id: "artifact.requirement_coverage.summary".to_string(),
            kind: "requirement_coverage_summary".to_string(),
            schema: "mercurio.requirement_coverage.summary.v1".to_string(),
            digest: summary_digest(&summary_payload),
            element_refs: view.rows.iter().map(requirement_element_ref).collect(),
            payload: summary_payload,
        }],
        evidence: EvidenceGraph {
            nodes: evidence_nodes,
            edges: Vec::new(),
        },
    }
}

pub fn analyze_semantic_impact(
    runtime: &Runtime,
    context: SemanticContextRef,
    request_id: impl Into<String>,
) -> ReasoningReport {
    let graph = runtime.graph();
    let requirements = requirements_table_view(graph);
    let mut findings = Vec::new();
    let mut evidence_nodes = Vec::new();
    let mut relation_counts = BTreeMap::<String, usize>::new();
    let mut hotspot_count = 0usize;

    for edge in graph.edges() {
        *relation_counts.entry(edge.relation.clone()).or_default() += 1;
    }

    for element in graph.elements() {
        let incoming_count = graph.incoming_edges(element.id).count();
        let outgoing_count = graph.outgoing_edges(element.id).count();
        if incoming_count == 0 && outgoing_count == 0 {
            continue;
        }

        evidence_nodes.push(impact_evidence_node(
            element,
            incoming_count,
            outgoing_count,
        ));

        let degree = incoming_count + outgoing_count;
        if degree >= 5 {
            hotspot_count += 1;
            findings.push(ReasoningFinding {
                id: format!("finding.semantic_impact.hotspot.{}", element.element_id),
                title: "Element has high semantic impact".to_string(),
                severity: FindingSeverity::Info,
                message: format!(
                    "Element participates in {degree} semantic relationships ({incoming_count} incoming, {outgoing_count} outgoing)."
                ),
                elements: vec![element_ref(element)],
                source_spans: Vec::new(),
                evidence_ids: vec![impact_evidence_id(element)],
                properties: BTreeMap::from([
                    ("incomingCount".to_string(), json!(incoming_count)),
                    ("outgoingCount".to_string(), json!(outgoing_count)),
                    ("degree".to_string(), json!(degree)),
                ]),
            });
        }
    }

    for requirement in &requirements.rows {
        if requirement.satisfied_by.is_empty() {
            findings.push(missing_trace_finding(
                requirement,
                "satisfy",
                "Requirement has no downstream satisfaction impact",
                "Impact analysis found no satisfy relationship reaching this requirement.",
                FindingSeverity::Warning,
            ));
        }
        if requirement.verified_by.is_empty() {
            findings.push(missing_trace_finding(
                requirement,
                "verify",
                "Requirement has no downstream verification impact",
                "Impact analysis found no verify relationship reaching this requirement.",
                FindingSeverity::Error,
            ));
        }
    }

    if graph.edges().is_empty() {
        findings.push(ReasoningFinding {
            id: "finding.semantic_impact.no_relationships".to_string(),
            title: "No semantic relationships found".to_string(),
            severity: FindingSeverity::Warning,
            message: "The semantic graph contains no derived relationships to traverse."
                .to_string(),
            elements: Vec::new(),
            source_spans: Vec::new(),
            evidence_ids: Vec::new(),
            properties: BTreeMap::new(),
        });
    }

    let status = if findings.iter().any(|finding| {
        matches!(
            finding.severity,
            FindingSeverity::Error | FindingSeverity::Critical
        )
    }) {
        ReasoningStatus::Failed
    } else if findings.is_empty() {
        ReasoningStatus::Passed
    } else {
        ReasoningStatus::Inconclusive
    };

    let summary_payload = json!({
        "elementCount": graph.elements().len(),
        "relationshipCount": graph.edges().len(),
        "relationCounts": relation_counts,
        "requirementCount": requirements.rows.len(),
        "hotspotCount": hotspot_count,
        "findingCount": findings.len(),
    });

    ReasoningReport {
        request_id: request_id.into(),
        capability: semantic_impact_capability_descriptor(),
        context,
        status,
        findings,
        artifacts: vec![ReasoningArtifact {
            id: "artifact.semantic_impact.summary".to_string(),
            kind: "semantic_impact_summary".to_string(),
            schema: "mercurio.semantic_impact.summary.v1".to_string(),
            digest: summary_digest(&summary_payload),
            element_refs: evidence_nodes
                .iter()
                .flat_map(|node| node.element_refs.clone())
                .collect(),
            payload: summary_payload,
        }],
        evidence: EvidenceGraph {
            nodes: evidence_nodes,
            edges: Vec::new(),
        },
    }
}

fn impact_evidence_node(
    element: &Element,
    incoming_count: usize,
    outgoing_count: usize,
) -> EvidenceNode {
    EvidenceNode {
        id: impact_evidence_id(element),
        kind: EvidenceNodeKind::KirElement,
        label: element_label(element),
        element_refs: vec![element_ref(element)],
        source_spans: Vec::new(),
        properties: BTreeMap::from([
            ("incomingCount".to_string(), json!(incoming_count)),
            ("outgoingCount".to_string(), json!(outgoing_count)),
        ]),
    }
}

fn impact_evidence_id(element: &Element) -> String {
    format!("evidence.semantic_impact.{}", element.element_id)
}

fn element_ref(element: &Element) -> SemanticElementRef {
    SemanticElementRef {
        element_id: element.element_id.clone(),
        qualified_name: None,
        label: Some(element_label(element)),
    }
}

fn element_label(element: &Element) -> String {
    element
        .properties
        .get("declared_name")
        .or_else(|| element.properties.get("name"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| element.element_id.clone())
}

fn missing_trace_finding(
    requirement: &RequirementTableRowDto,
    trace_kind: &str,
    title: &str,
    message: &str,
    severity: FindingSeverity,
) -> ReasoningFinding {
    ReasoningFinding {
        id: format!(
            "finding.requirement.{trace_kind}.missing.{}",
            requirement.id
        ),
        title: title.to_string(),
        severity,
        message: message.to_string(),
        elements: vec![requirement_element_ref(requirement)],
        source_spans: source_spans(requirement),
        evidence_ids: vec![requirement_evidence_id(requirement)],
        properties: BTreeMap::from([
            (
                "requirementId".to_string(),
                Value::String(requirement.id.clone()),
            ),
            (
                "traceKind".to_string(),
                Value::String(trace_kind.to_string()),
            ),
        ]),
    }
}

fn requirement_evidence_node(requirement: &RequirementTableRowDto) -> EvidenceNode {
    EvidenceNode {
        id: requirement_evidence_id(requirement),
        kind: EvidenceNodeKind::KirElement,
        label: requirement
            .name
            .clone()
            .unwrap_or_else(|| requirement.id.clone()),
        element_refs: vec![requirement_element_ref(requirement)],
        source_spans: source_spans(requirement),
        properties: BTreeMap::from([
            (
                "satisfiedBy".to_string(),
                Value::Array(
                    requirement
                        .satisfied_by
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
            (
                "verifiedBy".to_string(),
                Value::Array(
                    requirement
                        .verified_by
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            ),
        ]),
    }
}

fn requirement_element_ref(requirement: &RequirementTableRowDto) -> SemanticElementRef {
    SemanticElementRef {
        element_id: requirement.id.clone(),
        qualified_name: None,
        label: requirement.name.clone(),
    }
}

fn requirement_evidence_id(requirement: &RequirementTableRowDto) -> String {
    format!("evidence.requirement.{}", requirement.id)
}

fn source_spans(requirement: &RequirementTableRowDto) -> Vec<SourceSpanRef> {
    requirement
        .source
        .as_ref()
        .and_then(source_span_ref)
        .into_iter()
        .collect()
}

fn source_span_ref(source: &RequirementSourceDto) -> Option<SourceSpanRef> {
    Some(SourceSpanRef {
        file: source.file.clone()?,
        start_line: u32::try_from(source.start_line?).ok()?,
        start_col: 1,
        end_line: u32::try_from(source.end_line?).ok()?,
        end_col: 1,
    })
}

fn summary_digest(value: &Value) -> String {
    let encoded = serde_json::to_string(value).unwrap_or_default();
    let mut hash = 0xcbf29ce484222325u64;
    for byte in encoded.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64_{hash:016x}")
}

#[cfg(test)]
mod tests {
    use mercurio_core::{KirDocument, Runtime, repo_path};
    use mercurio_reasoner_api::{SemanticArtifactRef, SemanticContextKind, SemanticContextRef};

    use super::*;

    #[test]
    fn requirement_coverage_reports_missing_verification() {
        let document =
            KirDocument::from_path(&repo_path("examples/requirements_table_model.json")).unwrap();
        let runtime = Runtime::from_document(document).unwrap();
        let report = analyze_requirement_coverage(&runtime, test_context(), "req-coverage-test");

        assert_eq!(report.status, ReasoningStatus::Failed);
        assert!(report.findings.iter().any(|finding| {
            finding
                .id
                .contains("verify.missing.req.VehicleSafety.DriverAlert")
        }));
        assert_eq!(report.evidence.nodes.len(), 3);
        assert_eq!(
            report.artifacts[0].payload["verifiedCount"],
            serde_json::Value::from(2)
        );
    }

    #[test]
    fn semantic_impact_reports_graph_relationship_summary() {
        let document =
            KirDocument::from_path(&repo_path("examples/requirements_table_model.json")).unwrap();
        let runtime = Runtime::from_document(document).unwrap();
        let report = analyze_semantic_impact(&runtime, test_context(), "semantic-impact-test");

        assert_eq!(report.capability.id, SEMANTIC_IMPACT_CAPABILITY_ID);
        assert_eq!(report.capability.kind, CapabilityKind::StaticAnalysis);
        assert_eq!(report.status, ReasoningStatus::Failed);
        assert!(
            report
                .artifacts
                .iter()
                .any(|artifact| artifact.schema == "mercurio.semantic_impact.summary.v1")
        );
        assert!(
            report.artifacts[0].payload["relationshipCount"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(report.findings.iter().any(|finding| {
            finding
                .id
                .contains("verify.missing.req.VehicleSafety.DriverAlert")
        }));
    }

    fn test_context() -> SemanticContextRef {
        SemanticContextRef {
            context_id: "ctx.test".to_string(),
            kind: SemanticContextKind::Accepted,
            artifact: SemanticArtifactRef {
                artifact_key: "artifact.test".to_string(),
                kir_schema_version: "0.1".to_string(),
                source_authority: Some("test_fixture".to_string()),
                source_revision: None,
            },
        }
    }
}
